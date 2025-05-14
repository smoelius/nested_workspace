use super::Source;
use anyhow::{Result, bail, ensure};
use std::{
    ffi::OsStr,
    fmt::Debug,
    io::{self, BufRead},
    path::Path,
    process::Command,
};

pub enum CargoSubcommand {
    Build,
    Check,
    Run,
    Test,
    Other(String),
}

impl CargoSubcommand {
    fn as_str(&self) -> &str {
        match self {
            CargoSubcommand::Build => "build",
            CargoSubcommand::Check => "check",
            CargoSubcommand::Run => "run",
            CargoSubcommand::Test => "test",
            CargoSubcommand::Other(other) => other,
        }
    }
}

impl std::fmt::Display for CargoSubcommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn parent_command() -> Result<String> {
    let ppid = parent_id();
    let mut command = Command::new("ps");
    command.args(["-p", &ppid.to_string(), "-o", "args="]);
    let output = command.output()?;
    ensure!(output.status.success(), "command failed: {command:?}");
    let lines = output.stdout.lines().collect::<io::Result<Vec<_>>>()?;
    let [line] = &lines[..] else {
        bail!(
            "expected one line but found {} in command output: {:?}",
            lines.len(),
            command
        );
    };
    Ok(line.clone())
}

#[cfg(unix)]
fn parent_id() -> u32 {
    std::os::unix::process::parent_id()
}

#[expect(clippy::similar_names)]
pub fn parse_cargo_command<T: AsRef<str> + Debug>(args: &[T]) -> Result<(CargoSubcommand, &[T])> {
    if args.is_empty()
        || !{
            let arg0 = args[0].as_ref();
            let path = Path::new(&arg0);
            path.file_stem()
                .and_then(OsStr::to_str)
                .is_some_and(|file_stem| file_stem == "cargo" || file_stem.starts_with("cargo-"))
        }
    {
        bail!("failed to parse Cargo command: {args:?}")
    }
    parse_cargo_subcommand(&args[1..])
}

pub fn parse_cargo_subcommand<T: AsRef<str> + Debug>(
    args: &[T],
) -> Result<(CargoSubcommand, &[T])> {
    if args.is_empty() {
        bail!("failed to parse Cargo subcommand: {args:?}")
    }
    let subcommand = match args[0].as_ref() {
        "build" => CargoSubcommand::Build,
        "check" => CargoSubcommand::Check,
        "run" => CargoSubcommand::Run,
        "test" => CargoSubcommand::Test,
        other => CargoSubcommand::Other(other.to_owned()),
    };
    Ok((subcommand, &args[1..]))
}

pub fn build_cargo_command<T: AsRef<str> + Debug>(
    source: Source,
    subcommand: &CargoSubcommand,
    args: &[T],
) -> Result<Command> {
    let mut command = Command::new("cargo");
    let subcommand = match (&source, &subcommand) {
        // smoelius: If `cargo check` caused the build script to be run, run `cargo check` (i.e.,
        // running `cargo build` would be too much). For all other cases, run `cargo build`.
        (Source::BuildScript, CargoSubcommand::Check) => "check",
        (Source::BuildScript, _) => "build",
        (Source::Test, CargoSubcommand::Test) => "test",
        (Source::CargoNw, _) => subcommand.as_str(),
        (_, _) => bail!("{source} unexpectedly invoked subcommand `{subcommand}`"),
    };
    command.arg(subcommand);
    // smoelius: Do not forward `args` to `cargo build` or `cargo check`. If `args` contains
    // `--manifest-path ...`, for example, the command could block. Do, however, pass `--workspace`
    // and `-vv`. The latter aids in debugging.
    // smoelius: Do not pass `--workspace` to all Cargo subcommands, because not all subcommands
    // accept such an option. `cargo fmt` is an example.
    if matches!(source, Source::BuildScript) {
        command.args(["--workspace", "-vv"]);
    } else {
        for arg in args {
            command.arg(arg.as_ref());
        }
    }
    command.env_remove("CARGO");
    command.env_remove("RUSTC");
    command.env_remove("RUSTUP_TOOLCHAIN");
    Ok(command)
}
