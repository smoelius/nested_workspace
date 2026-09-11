use crate::{
    NestedWorkspaceRoot, Source,
    cargo_nested::CARGO_NESTED_ENV,
    reentrancy_guard::{dependent_from_package_name, reentrancy_guard_from_package_name},
};
use anyhow::{Result, bail, ensure};
use elaborate::std::{ffi::OsStrContext, path::PathContext, process::CommandContext};
use log::debug;
use std::{
    ffi::{OsStr, OsString},
    fmt::Debug,
    io::{Write, stderr},
    path::Path,
    process::{Command, id},
    sync::LazyLock,
};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System, UpdateKind};

/// A Cargo subcommand, with variants for recognized commands and a fallback for other commands.
#[doc(hidden)]
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

/// Parses a Cargo command, returning its subcommand and remaining arguments.
#[doc(hidden)]
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

/// Parses a Cargo subcommand and returns its remaining arguments.
#[doc(hidden)]
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

/// A Cargo command whose subcommand and arguments have been prepared for reuse across nested
/// workspace roots.
#[doc(hidden)]
pub struct NestedWorkspaceCommand<'a> {
    source: Source,
    package_name: Option<&'a str>,
    subcommand: &'a OsStr,
    args: Vec<OsString>,
}

impl<'a> NestedWorkspaceCommand<'a> {
    /// Prepares the subcommand and arguments once, emitting any filtering or duplication warnings.
    pub fn new<T: AsRef<OsStr>>(
        source: Source,
        package_name: Option<&'a str>,
        subcommand: &'a CargoSubcommand,
        args: &Args<'_, T>,
    ) -> Result<Self> {
        let (subcommand, args) = build_subcommand_and_args(source, package_name, subcommand, args)?;
        Ok(Self {
            source,
            package_name,
            subcommand,
            args,
        })
    }

    /// Builds and runs the prepared command, returning an error if it fails.
    pub fn run(&self, root: Option<&NestedWorkspaceRoot>) -> Result<()> {
        let mut command = self.build_command(root)?;
        debug!("{}: {command:?}", self.source);
        let status = command.status_wc()?;
        ensure!(status.success(), "command failed: {command:?}");
        Ok(())
    }

    /// Builds a command for the supplied nested workspace root, or for the current directory if
    /// none is given.
    fn build_command(&self, root: Option<&NestedWorkspaceRoot>) -> Result<Command> {
        let mut command = Command::new("cargo");
        command.arg(self.subcommand);
        command.args(&self.args);
        command.env_remove("CARGO");
        command.env_remove("RUSTC");
        command.env_remove("RUSTUP_TOOLCHAIN");
        match self.source {
            Source::CargoNested => {
                command.env(CARGO_NESTED_ENV, "1");
            }
            Source::BuildScript => {
                let Some(package_name) = self.package_name else {
                    bail!("failed to get package name");
                };
                let reentrancy_guard = reentrancy_guard_from_package_name(package_name);
                command.env(reentrancy_guard, "1");
                if root.is_some_and(NestedWorkspaceRoot::dependent) {
                    let dependent = dependent_from_package_name(package_name);
                    command.env(dependent, "1");
                }
            }
            Source::Test => {}
        }
        if let Some(root) = root {
            command.current_dir(root.path());
        }
        Ok(command)
    }
}

/// Explicit arguments supplied by a builder and arguments inherited from a parent Cargo invocation.
#[doc(hidden)]
pub struct Args<'a, T: AsRef<OsStr>> {
    pub explicit: &'a [T],
    pub inherited: &'a [T],
}

impl<'a, T: AsRef<OsStr>> Args<'a, T> {
    /// Creates an argument set containing only inherited arguments.
    pub fn inherited(inherited: &'a [T]) -> Self {
        Self {
            explicit: &[],
            inherited,
        }
    }
}

/// Prepares the Cargo subcommand and its arguments according to the invocation source. Emits
/// warnings for inherited arguments removed by filtering or deduplication.
fn build_subcommand_and_args<'subcommand, T: AsRef<OsStr>>(
    source: Source,
    package_name: Option<&str>,
    subcommand: &'subcommand CargoSubcommand,
    args: &Args<'_, T>,
) -> Result<(&'subcommand OsStr, Vec<OsString>)> {
    let (subcommand, args) = match (&source, &subcommand) {
        // smoelius: If `cargo check` caused the build script to be run, run `cargo check` (i.e.,
        // running `cargo build` would be too much). For all other cases, run `cargo build`.
        (Source::BuildScript, CargoSubcommand::Check) => (
            OsStr::new("check"),
            build_or_check_args(package_name, subcommand, args),
        ),
        (Source::BuildScript, _subcommand_other_than_check) => (
            OsStr::new("build"),
            build_or_check_args(package_name, &CargoSubcommand::Build, args),
        ),
        (Source::Test, CargoSubcommand::Test) => (
            OsStr::new("test"),
            test_args(package_name, subcommand, args),
        ),
        // smoelius: Do not pass `--workspace` to all Cargo subcommands, because not all subcommands
        // accept such an option. `cargo fmt` is an example.
        (Source::CargoNested, _) => {
            assert!(
                args.explicit.is_empty(),
                "`cargo-nested` should not use explicit arguments"
            );
            let args = args.inherited.iter().map(OsString::from).collect();
            (subcommand.as_os_str(), args)
        }
        (_, _) => bail!("{source} unexpectedly invoked subcommand `{subcommand}`"),
    };
    Ok((subcommand, args))
}

fn build_or_check_args<T: AsRef<OsStr>>(
    package_name: Option<&str>,
    subcommand: &CargoSubcommand,
    args: &Args<'_, T>,
) -> Vec<OsString> {
    // smoelius: The following arguments are prepended to the arguments passed: `-vv`, `--offline`,
    // and `--workspace`.
    let mut args_out = ["-vv", "--offline", "--workspace"]
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    args_out.extend(args.explicit.iter().map(OsString::from));
    let mut args_duplicated = Vec::new();
    let mut args_filtered = Vec::new();
    for arg in args.inherited {
        // smoelius: The following arguments are forwarded provided they were not already passed
        // with `Builder::arg` or `Builder::args`: `--frozen` and `--locked`. (Cargo rejects
        // repeated occurrences of either option.)
        let arg_as_ref = arg.as_ref();
        if arg_as_ref == OsStr::new("--frozen") || arg_as_ref == OsStr::new("--locked") {
            if args_out
                .iter()
                .any(|arg_out| arg_out.as_os_str() == arg_as_ref)
            {
                args_duplicated.push(arg_as_ref.to_owned());
            } else {
                args_out.push(arg_as_ref.to_owned());
            }
        } else {
            // smoelius: All arguments besides those covered by the previous bullet are filtered
            // out, i.e., no other arguments are forwarded. Do not forward other `args`
            // to `cargo build` or `cargo check`. If `args` contains `--manifest-path
            // ...`, for example, the command could block.
            args_filtered.push(arg_as_ref.to_owned());
        }
    }
    if !args_filtered.is_empty() {
        println!(
            "cargo::warning={}",
            filtered_message(package_name, subcommand, &args_filtered, false)
        );
    }
    if !args_duplicated.is_empty() {
        println!(
            "cargo::warning={}",
            filtered_message(package_name, subcommand, &args_duplicated, true)
        );
    }
    args_out
}

fn test_args<T: AsRef<OsStr>>(
    package_name: Option<&str>,
    subcommand: &CargoSubcommand,
    args: &Args<'_, T>,
) -> Vec<OsString> {
    const ARGS_INIT: [&str; 2] = ["--offline", "--workspace"];
    // smoelius: The following arguments are prepended to the arguments passed: `--offline` and
    // `--workspace`. (The reason for prepending these arguments is to ensure they do not appear
    // after `--` and are thus rejected by `libtest`.)
    let mut args_out = ARGS_INIT.iter().map(OsString::from).collect::<Vec<_>>();
    args_out.extend(args.explicit.iter().map(OsString::from));
    let mut args_filtered = Vec::new();
    let mut args_duplicated = Vec::new();
    let package_name_os = package_name.map(OsStr::new);
    let mut iter = args.inherited.iter().peekable();
    while let Some(arg) = iter.next() {
        let arg_as_ref = arg.as_ref();
        // smoelius: The following arguments are filtered out: `-p <containing-package>` and
        // `--package <containing-package>`.
        if let Some(package_name_os) = package_name_os
            && (arg_as_ref == OsStr::new("-p") || arg_as_ref == OsStr::new("--package"))
            && iter.peek().map(AsRef::as_ref) == Some(package_name_os)
        {
            let _: Option<&T> = iter.next();
            args_filtered.extend_from_slice(&[arg_as_ref.to_owned(), package_name_os.to_owned()]);
            continue;
        }
        if ARGS_INIT.iter().any(|arg| arg_as_ref == OsStr::new(arg)) {
            args_duplicated.push(arg_as_ref.to_owned());
            continue;
        }
        // smoelius: All arguments besides those covered by the previous bullet are forwarded.
        args_out.push(arg_as_ref.to_owned());
    }
    if !args_filtered.is_empty() {
        #[allow(clippy::explicit_write)]
        writeln!(
            stderr(),
            "Warning: {}",
            filtered_message(package_name, subcommand, &args_filtered, false)
        )
        .unwrap();
    }
    if !args_duplicated.is_empty() {
        #[allow(clippy::explicit_write)]
        writeln!(
            stderr(),
            "Warning: {}",
            filtered_message(package_name, subcommand, &args_duplicated, true)
        )
        .unwrap();
    }
    args_out
}

fn filtered_message(
    package_name: Option<&str>,
    subcommand: &CargoSubcommand,
    args: &[OsString],
    duplicated: bool,
) -> String {
    let of_package_name = package_name.map_or(String::new(), |package_name| {
        format!(" of `{package_name}`")
    });
    let maybe_why = if duplicated {
        " because they would be duplicated"
    } else {
        ""
    };
    format!(
        "The following arguments were removed from the `cargo {subcommand}` command run on nested \
         workspaces{of_package_name}{maybe_why}: {}",
        args.join(OsStr::new(" ")).display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_check_forward_frozen_and_locked() {
        for (subcommand, subcommand_expected) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            let args_in_and_expected: &[(&[&str], &[&str])] = &[
                (
                    &["--frozen", "--release"],
                    &["-vv", "--offline", "--workspace", "--frozen"],
                ),
                (
                    &["--locked", "--release"],
                    &["-vv", "--offline", "--workspace", "--locked"],
                ),
            ];
            for (args_in, args_expected) in args_in_and_expected {
                let NestedWorkspaceCommand {
                    subcommand: subcommand_actual,
                    args: args_actual,
                    ..
                } = NestedWorkspaceCommand::new(
                    Source::BuildScript,
                    Some("package"),
                    &subcommand,
                    &Args::inherited(args_in),
                )
                .unwrap();

                assert_eq!(OsStr::new(subcommand_expected), subcommand_actual);
                assert_eq!(
                    args_expected.iter().map(OsStr::new).collect::<Vec<_>>(),
                    args_actual,
                );
            }
        }
    }

    #[test]
    fn build_and_check_do_not_forward_frozen_or_locked_twice() {
        for (subcommand, subcommand_expected) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            for flag in ["--frozen", "--locked"] {
                let NestedWorkspaceCommand {
                    subcommand: subcommand_actual,
                    args,
                    ..
                } = NestedWorkspaceCommand::new(
                    Source::BuildScript,
                    Some("package"),
                    &subcommand,
                    &Args {
                        explicit: &[flag],
                        inherited: &[flag],
                    },
                )
                .unwrap();

                assert_eq!(OsStr::new(subcommand_expected), subcommand_actual);
                assert_eq!(
                    [
                        OsStr::new("-vv"),
                        OsStr::new("--offline"),
                        OsStr::new("--workspace"),
                        OsStr::new(flag),
                    ],
                    args.as_slice(),
                );
            }
        }
    }

    #[test]
    fn build_and_check_forward_explicit_args_unconditionally() {
        for (subcommand, subcommand_expected) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            let NestedWorkspaceCommand {
                subcommand: subcommand_actual,
                args,
                ..
            } = NestedWorkspaceCommand::new(
                Source::BuildScript,
                Some("package"),
                &subcommand,
                &Args {
                    explicit: &["--locked", "--release"],
                    inherited: &[],
                },
            )
            .unwrap();

            assert_eq!(OsStr::new(subcommand_expected), subcommand_actual);
            assert_eq!(
                [
                    OsStr::new("-vv"),
                    OsStr::new("--offline"),
                    OsStr::new("--workspace"),
                    OsStr::new("--locked"),
                    OsStr::new("--release"),
                ],
                args.as_slice(),
            );
        }
    }

    #[test]
    fn build_and_check_prepend_explicit_args_to_inherited_args() {
        for (subcommand, subcommand_expected) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            let NestedWorkspaceCommand {
                subcommand: subcommand_actual,
                args,
                ..
            } = NestedWorkspaceCommand::new(
                Source::BuildScript,
                Some("package"),
                &subcommand,
                &Args {
                    explicit: &["--release"],
                    inherited: &["--locked", "--release"],
                },
            )
            .unwrap();

            assert_eq!(OsStr::new(subcommand_expected), subcommand_actual);
            assert_eq!(
                [
                    OsStr::new("-vv"),
                    OsStr::new("--offline"),
                    OsStr::new("--workspace"),
                    OsStr::new("--release"),
                    OsStr::new("--locked"),
                ],
                args.as_slice(),
            );
        }
    }

    #[test]
    fn test_without_package_prepends_offline_and_workspace() {
        const ARGS_IN: &[&[&str]] = &[
            &["--", "--nocapture"],
            &["--offline", "--", "--nocapture"],
            &["--workspace", "--", "--nocapture"],
        ];
        for args_in in ARGS_IN {
            let NestedWorkspaceCommand {
                subcommand, args, ..
            } = NestedWorkspaceCommand::new(
                Source::Test,
                None,
                &CargoSubcommand::Test,
                &Args::inherited(args_in),
            )
            .unwrap();

            assert_eq!(OsStr::new("test"), subcommand);
            assert_eq!(
                [
                    OsStr::new("--offline"),
                    OsStr::new("--workspace"),
                    OsStr::new("--"),
                    OsStr::new("--nocapture"),
                ],
                args.as_slice(),
            );
        }
    }

    #[test]
    fn test_prepends_explicit_args_to_inherited_args() {
        let NestedWorkspaceCommand {
            subcommand, args, ..
        } = NestedWorkspaceCommand::new(
            Source::Test,
            None,
            &CargoSubcommand::Test,
            &Args {
                explicit: &["--release"],
                inherited: &["--", "--nocapture"],
            },
        )
        .unwrap();

        assert_eq!(OsStr::new("test"), subcommand);

        assert_eq!(
            [
                OsStr::new("--offline"),
                OsStr::new("--workspace"),
                OsStr::new("--release"),
                OsStr::new("--"),
                OsStr::new("--nocapture"),
            ],
            args.as_slice(),
        );
    }
}
