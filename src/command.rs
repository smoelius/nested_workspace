use crate::{
    Source,
    cargo_nested::CARGO_NESTED_ENV,
    reentrancy_guard::{dependent_from_package_name, reentrancy_guard_from_package_name},
};
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

#[derive(Clone)]
pub struct PackageContext {
    pub name: String,
    pub dependent: bool,
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

#[doc(hidden)]
pub struct Args<'a, T: AsRef<OsStr>> {
    pub explicit: &'a [T],
    pub inherited: &'a [T],
}

impl<'a, T: AsRef<OsStr>> Args<'a, T> {
    pub fn inherited(inherited: &'a [T]) -> Self {
        Self {
            explicit: &[],
            inherited,
        }
    }
}

#[doc(hidden)]
pub fn build_cargo_command<T: AsRef<OsStr>>(
    source: Source,
    package: Option<&PackageContext>,
    subcommand: &CargoSubcommand,
    args: &Args<'_, T>,
) -> Result<Command> {
    let mut command = Command::new("cargo");
    let (subcommand, args) = match (&source, &subcommand) {
        // smoelius: If `cargo check` caused the build script to be run, run `cargo check` (i.e.,
        // running `cargo build` would be too much). For all other cases, run `cargo build`.
        (Source::BuildScript, CargoSubcommand::Check) => {
            (OsStr::new("check"), build_or_check_args(args))
        }
        (Source::BuildScript, _subcommand_other_than_check) => {
            (OsStr::new("build"), build_or_check_args(args))
        }
        (Source::Test, CargoSubcommand::Test) => (
            OsStr::new("test"),
            test_args(package.map(|package| package.name.as_str()), args),
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
    command.arg(subcommand);
    command.args(args);
    command.env_remove("CARGO");
    command.env_remove("RUSTC");
    command.env_remove("RUSTUP_TOOLCHAIN");
    match source {
        Source::CargoNested => {
            command.env(CARGO_NESTED_ENV, "1");
        }
        Source::BuildScript => {
            let Some(package) = package else {
                bail!("failed to get package name");
            };
            let reentrancy_guard = reentrancy_guard_from_package_name(&package.name);
            command.env(reentrancy_guard, "1");
            if package.dependent {
                let dependent = dependent_from_package_name(&package.name);
                command.env(dependent, "1");
            }
        }
        Source::Test => {}
    }
    Ok(command)
}

fn build_or_check_args<T: AsRef<OsStr>>(args: &Args<'_, T>) -> Vec<OsString> {
    // smoelius: The following arguments are prepended to the arguments passed: `-vv`, `--offline`,
    // and `--workspace`.
    let mut args_out = ["-vv", "--offline", "--workspace"]
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    args_out.extend(args.explicit.iter().map(OsString::from));
    for arg in args.inherited {
        // smoelius: The following arguments are forwarded provided they were not already passed
        // with `Builder::arg` or `Builder::args`: `--frozen` and `--locked`.
        let arg_as_ref = arg.as_ref();
        if (arg_as_ref == OsStr::new("--frozen") || arg_as_ref == OsStr::new("--locked"))
            && !args_out
                .iter()
                .any(|arg_out| arg_out.as_os_str() == arg_as_ref)
        {
            args_out.push(arg_as_ref.to_owned());
        }
        // smoelius: All arguments besides those covered by the previous bullet are filtered out,
        // i.e., no other arguments are forwarded. Do not forward other `args` to `cargo build` or
        // `cargo check`. If `args` contains `--manifest-path ...`, for example, the command could
        // block.
    }
    args_out
}

fn test_args<T: AsRef<OsStr>>(package_name: Option<&str>, args: &Args<'_, T>) -> Vec<OsString> {
    // smoelius: The following arguments are prepended to the arguments passed: `--offline` and
    // `--workspace`. (The reason for prepending these arguments is to ensure they do not appear
    // after `--` and are thus rejected by `libtest`.)
    let mut args_out = ["--offline", "--workspace"]
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    args_out.extend(args.explicit.iter().map(OsString::from));
    let package_name = package_name.map(OsStr::new);
    let mut iter = args.inherited.iter().peekable();
    while let Some(arg) = iter.next() {
        let arg_as_ref = arg.as_ref();
        // smoelius: The following arguments are filtered out: `-p <containing-package>` and
        // `--package <containing-package>`.
        if let Some(package_name) = package_name
            && (arg_as_ref == OsStr::new("-p") || arg_as_ref == OsStr::new("--package"))
            && iter.peek().map(AsRef::as_ref) == Some(package_name)
        {
            let _: Option<&T> = iter.next();
            continue;
        }
        if arg_as_ref == OsStr::new("--offline") || arg_as_ref == OsStr::new("--workspace") {
            continue;
        }
        // smoelius: All arguments besides those covered by the previous bullet are forwarded.
        args_out.push(arg_as_ref.to_owned());
    }
    args_out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_check_forward_frozen_and_locked() {
        let package = PackageContext {
            name: "package".to_owned(),
            dependent: false,
        };

        for (subcommand, expected_subcommand) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            let args_in_and_expected: &[(&[&str], &[&str])] = &[
                (
                    &["--frozen", "--release"],
                    &[
                        expected_subcommand,
                        "-vv",
                        "--offline",
                        "--workspace",
                        "--frozen",
                    ],
                ),
                (
                    &["--locked", "--release"],
                    &[
                        expected_subcommand,
                        "-vv",
                        "--offline",
                        "--workspace",
                        "--locked",
                    ],
                ),
            ];
            for (args_in, args_expected) in args_in_and_expected {
                let command = build_cargo_command(
                    Source::BuildScript,
                    Some(&package),
                    &subcommand,
                    &Args::inherited(args_in),
                )
                .unwrap();

                let args_actual = command.get_args().collect::<Vec<_>>();
                assert_eq!(
                    args_expected.iter().map(OsStr::new).collect::<Vec<_>>(),
                    args_actual,
                );
            }
        }
    }

    #[test]
    fn build_and_check_do_not_forward_frozen_or_locked_twice() {
        let package = PackageContext {
            name: "package".to_owned(),
            dependent: false,
        };

        for (subcommand, expected_subcommand) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            for flag in ["--frozen", "--locked"] {
                let builder = crate::build().arg(flag);
                let command = builder
                    .cargo_command(Some(&package), &subcommand, &[OsString::from(flag)])
                    .unwrap();

                let args_actual = command.get_args().collect::<Vec<_>>();
                assert_eq!(
                    [
                        OsStr::new(expected_subcommand),
                        OsStr::new("-vv"),
                        OsStr::new("--offline"),
                        OsStr::new("--workspace"),
                        OsStr::new(flag),
                    ],
                    args_actual.as_slice(),
                );
            }
        }
    }

    #[test]
    fn build_and_check_forward_explicit_args_unconditionally() {
        let package = PackageContext {
            name: "package".to_owned(),
            dependent: false,
        };

        for (subcommand, expected_subcommand) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            let builder = crate::build().args(["--locked", "--release"]);
            let command = builder
                .cargo_command(Some(&package), &subcommand, &[])
                .unwrap();

            let args_actual = command.get_args().collect::<Vec<_>>();
            assert_eq!(
                [
                    OsStr::new(expected_subcommand),
                    OsStr::new("-vv"),
                    OsStr::new("--offline"),
                    OsStr::new("--workspace"),
                    OsStr::new("--locked"),
                    OsStr::new("--release"),
                ],
                args_actual.as_slice(),
            );
        }
    }

    #[test]
    fn build_and_check_prepend_explicit_args_to_inherited_args() {
        let package = PackageContext {
            name: "package".to_owned(),
            dependent: false,
        };

        for (subcommand, expected_subcommand) in [
            (CargoSubcommand::Build, "build"),
            (CargoSubcommand::Check, "check"),
        ] {
            let builder = crate::build().args(["--release"]);
            let command = builder
                .cargo_command(
                    Some(&package),
                    &subcommand,
                    &[OsString::from("--locked"), OsString::from("--release")],
                )
                .unwrap();

            let args_actual = command.get_args().collect::<Vec<_>>();
            assert_eq!(
                [
                    OsStr::new(expected_subcommand),
                    OsStr::new("-vv"),
                    OsStr::new("--offline"),
                    OsStr::new("--workspace"),
                    OsStr::new("--release"),
                    OsStr::new("--locked"),
                ],
                args_actual.as_slice(),
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
            let command = build_cargo_command(
                Source::Test,
                None,
                &CargoSubcommand::Test,
                &Args::inherited(args_in),
            )
            .unwrap();

            let args = command.get_args().collect::<Vec<_>>();
            assert_eq!(
                [
                    OsStr::new("test"),
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
        let builder = crate::test().args(["--release"]);
        let command = builder
            .cargo_command(
                None,
                &CargoSubcommand::Test,
                &[OsString::from("--"), OsString::from("--nocapture")],
            )
            .unwrap();

        assert_eq!(
            [
                OsStr::new("test"),
                OsStr::new("--offline"),
                OsStr::new("--workspace"),
                OsStr::new("--release"),
                OsStr::new("--"),
                OsStr::new("--nocapture"),
            ],
            command.get_args().collect::<Vec<_>>().as_slice(),
        );
    }
}
