use crate::{Source, reentrancy_guard::reentrancy_guard_from_package_name};
use anyhow::{Result, bail};
use elaborate::std::{ffi::OsStrContext, path::PathContext};
use std::{
    ffi::{OsStr, OsString},
    fmt::Debug,
    path::Path,
    process::Command,
};

pub enum CargoSubcommand {
    Build,
    Check,
    Run,
    Test,
    Other(OsString),
}

impl CargoSubcommand {
    fn as_os_str(&self) -> &OsStr {
        match self {
            CargoSubcommand::Build => OsStr::new("build"),
            CargoSubcommand::Check => OsStr::new("check"),
            CargoSubcommand::Run => OsStr::new("run"),
            CargoSubcommand::Test => OsStr::new("test"),
            CargoSubcommand::Other(other) => other,
        }
    }
}

impl std::fmt::Display for CargoSubcommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_os_str().display())
    }
}

pub use os_specific::parent_command;

#[cfg(unix)]
mod os_specific {
    use anyhow::{Result, bail, ensure};
    use elaborate::std::process::CommandContext;
    use std::os::unix::process::parent_id;
    use std::{
        io::{self, BufRead},
        process::Command,
    };

    pub fn parent_command() -> Result<String> {
        let ppid = parent_id();
        let mut command = Command::new("ps");
        command.args(["-o", "pid,args"]);
        // smoelius: `-A` is required to find the parent process on macOS.
        #[cfg(target_os = "macos")]
        command.arg("-A");
        let output = command.output_wc()?;
        ensure!(output.status.success(), "command failed: {command:?}");
        // smoelius: Skip the header.
        let lines = output
            .stdout
            .lines()
            .skip(1)
            .collect::<io::Result<Vec<_>>>()?;
        for line in lines {
            let Some((pid_str, args)) = line.trim_ascii_start().split_once(' ') else {
                bail!("line does not contain whitespace: {line:?}");
            };
            if ppid == pid_str.parse::<u32>()? {
                return Ok(args.to_owned());
            }
        }
        bail!("failed to find {ppid} args");
    }
}

#[cfg(windows)]
mod os_specific {
    use anyhow::{Context, Result, bail, ensure};
    use elaborate::std::process::CommandContext;
    use std::{
        process::{Command, id},
        str::FromStr,
    };

    pub fn parent_command() -> Result<String> {
        let ppid = parent_id()?;
        get_cim_instance::<String>(ppid, "CommandLine")
    }

    // smoelius: Based on:
    // https://stackoverflow.com/questions/7486717/finding-parent-process-id-on-windows
    fn parent_id() -> Result<u32> {
        get_cim_instance::<u32>(id(), "ParentProcessId")
    }

    fn get_cim_instance<T>(pid: u32, property: &str) -> Result<T>
    where
        T: FromStr,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
    {
        let mut command = Command::new("powershell");
        command.arg(format!(
            r#"(Get-CimInstance -ClassName Win32_Process -Filter "ProcessId = {pid}").{property}"#
        ));
        let output = command.output_wc()?;
        ensure!(output.status.success(), "command failed: {command:?}");
        let stdout = String::from_utf8(output.stdout)?;
        let mut lines = get_cim_instance_lines(&stdout);
        let (Some(line), None) = (lines.next(), lines.next()) else {
            bail!("unexpected output format: {stdout:?}");
        };
        str::parse::<T>(line)
            .with_context(|| format!("failed to parse line as {property}: {line:?}"))
    }

    fn get_cim_instance_lines(output: &str) -> impl Iterator<Item = &str> {
        output.lines().map(str::trim_end)
    }
}

#[expect(clippy::similar_names)]
pub fn parse_cargo_command<T: AsRef<OsStr> + Debug>(args: &[T]) -> Result<(CargoSubcommand, &[T])> {
    if args.is_empty()
        || !{
            let arg0 = args[0].as_ref();
            let path = Path::new(&arg0);
            path.file_stem_wc()
                .and_then(OsStr::to_str_wc)
                .is_ok_and(|file_stem| file_stem == "cargo" || file_stem.starts_with("cargo-"))
        }
    {
        bail!("failed to parse Cargo command: {args:?}")
    }
    parse_cargo_subcommand(&args[1..])
}

#[expect(clippy::similar_names)]
pub fn parse_cargo_subcommand<T: AsRef<OsStr> + Debug>(
    args: &[T],
) -> Result<(CargoSubcommand, &[T])> {
    if args.is_empty() {
        bail!("failed to parse Cargo subcommand: {args:?}")
    }
    let arg0 = args[0].as_ref();
    #[allow(clippy::allow_attributes, clippy::disallowed_methods)]
    let subcommand = match arg0.to_str() {
        Some("build") => CargoSubcommand::Build,
        Some("check") => CargoSubcommand::Check,
        Some("run") => CargoSubcommand::Run,
        Some("test") => CargoSubcommand::Test,
        _ => CargoSubcommand::Other(arg0.to_owned()),
    };
    Ok((subcommand, &args[1..]))
}

pub fn build_cargo_command<T: AsRef<OsStr> + Debug>(
    source: Source,
    package_name: Option<&str>,
    subcommand: &CargoSubcommand,
    args: &[T],
) -> Result<Command> {
    let mut command = Command::new("cargo");
    let (subcommand, args) = match (&source, &subcommand) {
        // smoelius: If `cargo check` caused the build script to be run, run `cargo check` (i.e.,
        // running `cargo build` would be too much). For all other cases, run `cargo build`.
        // smoelius: Do not forward `args` to `cargo build` or `cargo check`. If `args` contains
        // `--manifest-path ...`, for example, the command could block. Do, however, pass `-vv` and
        // `--workspace`. The former aids in debugging.
        (Source::BuildScript, CargoSubcommand::Check) => {
            (OsStr::new("check"), build_or_check_args())
        }
        (Source::BuildScript, _subcommand_other_than_check) => {
            (OsStr::new("build"), build_or_check_args())
        }
        (Source::Test, CargoSubcommand::Test) => {
            let args = std::iter::once(OsString::from("--workspace"))
                .chain(filter_package_and_workspace(package_name, args))
                .collect();
            (OsStr::new("test"), args)
        }
        // smoelius: Do not pass `--workspace` to all Cargo subcommands, because not all subcommands
        // accept such an option. `cargo fmt` is an example.
        (Source::CargoNested, _) => {
            let args = args.iter().map(OsString::from).collect();
            (subcommand.as_os_str(), args)
        }
        (_, _) => bail!("{source} unexpectedly invoked subcommand `{subcommand}`"),
    };
    command.arg(subcommand);
    command.args(args);
    command.env_remove("CARGO");
    command.env_remove("RUSTC");
    command.env_remove("RUSTUP_TOOLCHAIN");
    if matches!(source, Source::BuildScript) {
        let Some(package_name) = package_name else {
            bail!("failed to get package name");
        };
        let reentrancy_guard = reentrancy_guard_from_package_name(package_name);
        command.env(reentrancy_guard, "1");
    }
    Ok(command)
}

fn build_or_check_args() -> Vec<OsString> {
    ["-vv", "--offline", "--workspace"]
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>()
}

fn filter_package_and_workspace<T: AsRef<OsStr> + Debug>(
    package_name: Option<&str>,
    args_in: &[T],
) -> Vec<OsString> {
    let Some(package_name) = package_name.map(OsStr::new) else {
        return args_in.iter().map(OsString::from).collect();
    };
    let mut args_out = Vec::new();
    let mut iter = args_in.iter().peekable();
    while let Some(arg) = iter.next() {
        let arg_as_ref = arg.as_ref();
        if (arg_as_ref == OsStr::new("-p") || arg_as_ref == OsStr::new("--package"))
            && iter.peek().map(AsRef::as_ref) == Some(package_name)
        {
            let _: Option<&T> = iter.next();
            continue;
        }
        if arg_as_ref == OsStr::new("--workspace") {
            continue;
        }
        args_out.push(arg_as_ref.to_owned());
    }
    args_out
}
