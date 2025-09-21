use crate::{Source, reentrancy_guard::reentrancy_guard_from_package_name};
use anyhow::{Result, bail};
use elaborate::std::{ffi::OsStrContext, path::PathContext};
use std::{
    ffi::{OsStr, OsString},
    fmt::Debug,
    path::Path,
    process::{Command, id},
    sync::LazyLock,
};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System, UpdateKind};

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

static SYSTEM: LazyLock<System> = LazyLock::new(|| {
    System::new_with_specifics(
        RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always)),
    )
});

pub fn parent_cargo_command() -> Result<(CargoSubcommand, &'static [OsString])> {
    let mut id = id();
    loop {
        let (parent_id, command) = parent_command(id)?;
        match parse_cargo_command(command)? {
            Some((subcommand, args)) => {
                return Ok((subcommand, args));
            }
            None => {
                id = parent_id;
            }
        }
    }
}

fn parent_command(id: u32) -> Result<(u32, &'static [OsString])> {
    let Some(process) = SYSTEM.process(Pid::from_u32(id)) else {
        bail!("failed to get process with id {id}");
    };
    let Some(parent_id) = process.parent() else {
        bail!("failed to get {id}'s parent process id");
    };
    let Some(parent_process) = SYSTEM.process(parent_id) else {
        bail!("failed to get process with id {parent_id}");
    };
    let cmd = parent_process.cmd();
    Ok((parent_id.as_u32(), cmd))
}

#[expect(clippy::similar_names)]
pub fn parse_cargo_command<T: AsRef<OsStr> + Debug>(
    args: &[T],
) -> Result<Option<(CargoSubcommand, &[T])>> {
    if args.is_empty()
        || !{
            let arg0 = args[0].as_ref();
            let path = Path::new(&arg0);
            path.file_stem_wc()
                .and_then(OsStr::to_str_wc)
                .is_ok_and(|file_stem| file_stem == "cargo" || file_stem.starts_with("cargo-"))
        }
    {
        return Ok(None);
    }
    parse_cargo_subcommand(&args[1..]).map(Some)
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
