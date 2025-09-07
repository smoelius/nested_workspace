use anyhow::{Result, bail, ensure};
use cargo_metadata::{MetadataCommand, Package};
use log::debug;
use serde::Deserialize;
use std::{
    env::var,
    ffi::{OsStr, OsString},
    fmt::Debug,
    fs::{OpenOptions, write},
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};

mod command;
use command::parent_command;
pub use command::{
    CargoSubcommand, build_cargo_command, parse_cargo_command, parse_cargo_subcommand,
};

mod util;
use util::Delimiter;

#[derive(Deserialize)]
struct Metadata {
    roots: Vec<String>,
}

#[derive(Clone, Copy)]
pub enum Source {
    BuildScript,
    Test,
    CargoNested,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::BuildScript => f.write_str("build script"),
            Source::Test => f.write_str("test"),
            Source::CargoNested => f.write_str("cargo nested"),
        }
    }
}

#[must_use]
pub fn build() -> Builder {
    Builder {
        source: Source::BuildScript,
        args: Vec::new(),
    }
}

#[must_use]
pub fn test() -> Builder {
    Builder {
        source: Source::Test,
        args: Vec::new(),
    }
}

pub struct Builder {
    source: Source,
    args: Vec<OsString>,
}

impl Builder {
    /// Pass `arg` to subcommand
    #[must_use]
    pub fn arg<S>(mut self, arg: S) -> Builder
    where
        S: AsRef<OsStr>,
    {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    /// Pass `args` to subcommand
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Builder
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    pub fn unwrap(self) {
        // smoelius: Suppose a user runs `cargo check` followed by `cargo build`. Cargo's default
        // behavior is to run the build script for the first command (`cargo check`), but not again
        // for the second. However, we need to the build script to be rerun so that we can
        // call `cargo build` for the nested workspaces. `force_rerun` is a hack to achieve
        // this.
        if matches!(self.source, Source::BuildScript) {
            force_rerun().unwrap();
        }

        self.run_parent_cargo_command_on_current_package_nested_workspace_roots()
            .unwrap();
    }

    fn run_parent_cargo_command_on_current_package_nested_workspace_roots(self) -> Result<()> {
        let command = parent_command()?;
        let args = command.split_ascii_whitespace().collect::<Vec<_>>();
        let (subcommand, subcommand_args) = parse_cargo_command(&args)?;

        #[cfg(not(feature = "__disable_offline_check"))]
        if matches!(subcommand, CargoSubcommand::Build | CargoSubcommand::Check)
            && !subcommand_args
                .iter()
                .any(|&arg| arg == "--frozen" || arg == "--offline")
        {
            println!(
                "cargo::warning=Refusing to {subcommand} nested workspaces as `--offline` was not \
                 passed to parent command"
            );
            return Ok(());
        }

        let mut args = self.args;
        args.extend(subcommand_args.iter().map(OsString::from));

        let roots = current_package_nested_workspace_roots()?;

        run_cargo_subcommand_on_nested_workspace_roots(
            self.source,
            &subcommand,
            &args,
            None,
            &roots,
            false,
        )?;
        Ok(())
    }
}

const TIMESTAMP_CONTENTS: &str =
    "This file has an mtime of when a Nested Workspace build script was started.

https://github.com/smoelius/nested_workspace";

// smoelius: Variant of @juggle-tux's idea here:
// https://users.rust-lang.org/t/how-can-i-make-build-rs-rerun-every-time-that-cargo-run-or-cargo-build-is-run/51852/5
fn force_rerun() -> Result<()> {
    let out_dir = var("OUT_DIR")?;
    let path = PathBuf::from(out_dir).join("nested_workspace.timestamp");
    println!("cargo::rerun-if-changed={}", path.to_string_lossy());
    write(&path, TIMESTAMP_CONTENTS)?;
    // smoelius: Manually set the file's mtime. Simply creating/writing the file doesn't seem to
    // work on Windows. I'm not sure why.
    touch(&path)?;
    Ok(())
}

fn touch(path: &Path) -> Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_modified(SystemTime::now())?;
    Ok(())
}

pub fn run_cargo_subcommand_on_all_nested_workspace_roots<T: AsRef<OsStr> + Debug>(
    subcommand: &CargoSubcommand,
    args: &[T],
    dir: &Path,
    is_recursive_call: bool,
) -> Result<()> {
    let roots = all_nested_workspace_roots(dir)?;
    run_cargo_subcommand_on_nested_workspace_roots(
        Source::CargoNested,
        subcommand,
        args,
        Some(dir),
        &roots,
        is_recursive_call,
    )?;
    Ok(())
}

fn run_cargo_subcommand_on_nested_workspace_roots<T: AsRef<OsStr> + Debug>(
    source: Source,
    subcommand: &CargoSubcommand,
    args: &[T],
    dir: Option<&Path>,
    roots: &[PathBuf],
    is_recursive_call: bool,
) -> Result<()> {
    env_logger::try_init().unwrap_or_default();
    if roots.is_empty() {
        if !is_recursive_call {
            let in_dir = dir.map_or_else(String::new, |dir| format!(" in `{}`", dir.display()));
            writeln!(
                std::io::stderr(),
                "Warning: found no nested workspaces{in_dir}",
            )?;
        }
        return Ok(());
    }
    let package_name = var("CARGO_PKG_NAME").ok();
    for root in roots {
        let _delimiter = Delimiter::new(root);
        let mut command = build_cargo_command(source, package_name.as_deref(), subcommand, args)?;
        command.current_dir(root);
        debug!("{source}: {:?}", &command);
        let status = command.status()?;
        ensure!(status.success(), "command failed: {command:?}");
        // smoelius: `cargo nested` is a special case. It must be run manually on each nested
        // workspace root to ensure that _nested_-nested workspaces are handled.
        if matches!(source, Source::CargoNested) {
            run_cargo_subcommand_on_all_nested_workspace_roots(subcommand, args, root, true)?;
        }
    }
    Ok(())
}

fn current_package_nested_workspace_roots() -> Result<Vec<PathBuf>> {
    let cargo_manifest_path = var("CARGO_MANIFEST_PATH")?;
    let cargo_metadata = MetadataCommand::new().no_deps().exec()?;
    let Some(package) = cargo_metadata
        .packages
        .iter()
        .find(|package| package.manifest_path == cargo_manifest_path)
    else {
        bail!("failed to find package with manifest at `{cargo_manifest_path}`");
    };
    let Some(roots) = nested_workspace_roots_for_package(package)? else {
        bail!("package at `{cargo_manifest_path}` has no `nested_workspace` metadata");
    };
    Ok(roots)
}

fn all_nested_workspace_roots(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    let cargo_metadata = MetadataCommand::new().current_dir(dir).no_deps().exec()?;
    for package in &cargo_metadata.packages {
        if let Some(current_roots) = nested_workspace_roots_for_package(package)? {
            roots.extend(current_roots);
        }
    }
    Ok(roots)
}

fn nested_workspace_roots_for_package(package: &Package) -> Result<Option<Vec<PathBuf>>> {
    let Some(nested_workspace_value) = package
        .metadata
        .as_object()
        .and_then(|object| object.get("nested_workspace"))
    else {
        return Ok(None);
    };
    let Some(cargo_manifest_dir) = package.manifest_path.parent() else {
        bail!(
            "failed to get manifest dir from `{}`",
            package.manifest_path
        );
    };
    let nested_workspace_metadata =
        serde_json::from_value::<Metadata>(nested_workspace_value.clone())?;
    let roots = nested_workspace_metadata
        .roots
        .into_iter()
        .map(|path| Path::new(&cargo_manifest_dir).join(path))
        .collect();
    Ok(Some(roots))
}
