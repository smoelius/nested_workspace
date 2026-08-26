use anyhow::{Result, bail, ensure};
use cargo_metadata::{MetadataCommand, Package};
use elaborate::std::{
    env::var_wc,
    fs::{FileContext, OpenOptionsContext, write_wc},
    process::CommandContext,
};
use glob::glob;
use log::debug;
use serde::Deserialize;
use std::{
    ffi::{OsStr, OsString},
    fmt::Debug,
    fs::OpenOptions,
    io::{ErrorKind, Write, stderr},
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

mod cargo_nested;

mod command;
use command::parent_cargo_command;
pub use command::{
    CargoSubcommand, PackageContext, build_cargo_command, parse_cargo_command,
    parse_cargo_subcommand,
};

mod reentrancy_guard;
use reentrancy_guard::check_reentrancy_guard;

mod util;
use util::Delimiter;

#[derive(Deserialize)]
struct Metadata {
    roots: Vec<MetadataRoot>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MetadataRoot {
    Path(String),
    PathWithDependent {
        path: String,
        #[serde(default)]
        dependent: bool,
    },
}

impl MetadataRoot {
    fn path(&self) -> &str {
        match self {
            Self::Path(path) | Self::PathWithDependent { path, .. } => path,
        }
    }

    fn dependent(&self) -> bool {
        match self {
            Self::Path(_) => false,
            Self::PathWithDependent { dependent, .. } => *dependent,
        }
    }
}

pub struct NestedWorkspaceRoot {
    path: PathBuf,
    package: PackageContext,
}

impl NestedWorkspaceRoot {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn dependent(&self) -> bool {
        self.package.dependent
    }
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
        if matches!(self.source, Source::BuildScript) {
            if check_reentrancy_guard().unwrap() {
                return;
            }

            // smoelius: Suppose a user runs `cargo check` followed by `cargo build`. Cargo's
            // default behavior is to run the build script for the first command (`cargo check`),
            // but not again for the second. However, we need the build script to be rerun so that
            // we can call `cargo build` for the nested workspaces. `force_rerun` is a hack to
            // achieve this.
            force_rerun().unwrap();
        }

        // `cargo nested` traverses nested workspaces itself. Do not also traverse them through a
        // containing package's build script or test, as that would run commands more than once.
        if cargo_nested::enabled() {
            return;
        }

        self.run_parent_cargo_command_on_current_package_nested_workspace_roots()
            .unwrap();
    }

    fn run_parent_cargo_command_on_current_package_nested_workspace_roots(mut self) -> Result<()> {
        let (subcommand, subcommand_args) = parent_cargo_command()?;

        self.args.extend(subcommand_args.iter().map(OsString::from));

        let roots = current_package_nested_workspace_roots()?;
        env_logger::try_init().unwrap_or_default();
        if warn_if_no_nested_workspaces(&roots, None, false)? {
            return Ok(());
        }
        for root in &roots {
            if matches!(self.source, Source::BuildScript)
                && nested_workspace_uses_current_build_directory(&root.path)?
            {
                // A dependent nested workspace builds the containing package as a dependency. Do
                // not launch Cargo on that nested workspace if its build directory contains the
                // running build script's `OUT_DIR`: the parent Cargo process holds the build
                // directory lock, so the child Cargo process would wait for the parent while the
                // parent waits for this build script.
                //
                // `nested_workspace_uses_current_build_directory` considers only the Cargo
                // invocation running this build script. In a multilayer invocation, an inner
                // nested workspace could still reuse a build directory locked by an earlier
                // ancestor Cargo invocation if the intermediate layers use different build
                // directories. For example, Cargo A could lock build directory `a`, invoke Cargo
                // B using build directory `b`, and Cargo B's build script could invoke Cargo C
                // using build directory `a`. Cargo B's `OUT_DIR` is under `b`, so
                // `nested_workspace_uses_current_build_directory` would not detect the lock held
                // by Cargo A.
                //
                // Sharing a build directory does not prove that the parent Cargo invocation is
                // already processing this root. Cargo does not expose enough information to
                // distinguish that case from unrelated workspaces sharing a build directory, so
                // skip dependent roots to avoid deadlock and emit a warning because the requested
                // operation may not otherwise be performed.
                if root.dependent() {
                    println!(
                        "cargo::warning=skipping nested workspace `{}` because its build \
                         directory contains the running build script's `OUT_DIR` and it is marked \
                         as dependent",
                        root.path.display(),
                    );
                    continue;
                }
                bail!(
                    "cannot run on nested workspace `{}` because its build directory contains the \
                     running build script's `OUT_DIR`.\nInvoking Cargo would deadlock waiting for \
                     the current Cargo invocation's build directory lock.\nConfigure the nested \
                     workspace to use a distinct build directory (check `CARGO_TARGET_DIR`, \
                     `build.target-dir`, and `build.build-dir`).",
                    root.path.display(),
                );
            }
            let _delimiter = Delimiter::new(&root.path);
            let command = self.cargo_command(Some(&root.package), &subcommand)?;
            run_cargo_command(self.source, root, command)?;
        }
        Ok(())
    }

    fn cargo_command(
        &self,
        package: Option<&PackageContext>,
        subcommand: &CargoSubcommand,
    ) -> Result<Command> {
        build_cargo_command(self.source, package, subcommand, &self.args)
    }
}

fn nested_workspace_uses_current_build_directory(root: &Path) -> Result<bool> {
    let out_dir = dunce::canonicalize(var_wc("OUT_DIR")?)?;
    let cargo_metadata = MetadataCommand::new().current_dir(root).no_deps().exec()?;
    let build_directory = cargo_metadata
        .build_directory
        .unwrap_or(cargo_metadata.target_directory);
    let build_directory = match dunce::canonicalize(build_directory) {
        Ok(build_directory) => build_directory,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    Ok(out_dir.starts_with(build_directory))
}

const TIMESTAMP_CONTENTS: &str =
    "This file has an mtime of when a Nested Workspace build script was started.

https://github.com/smoelius/nested_workspace\
     ";

// smoelius: Variant of @juggle-tux's idea here:
// https://users.rust-lang.org/t/how-can-i-make-build-rs-rerun-every-time-that-cargo-run-or-cargo-build-is-run/51852/5
fn force_rerun() -> Result<()> {
    let out_dir = var_wc("OUT_DIR")?;
    let path = PathBuf::from(out_dir).join("nested_workspace.timestamp");
    println!("cargo::rerun-if-changed={}", path.to_string_lossy());
    write_wc(&path, TIMESTAMP_CONTENTS)?;
    // smoelius: Manually set the file's mtime. Simply creating/writing the file doesn't seem to
    // work on Windows. I'm not sure why.
    touch(&path)?;
    Ok(())
}

fn touch(path: &Path) -> Result<()> {
    let file = OpenOptions::new().write(true).open_wc(path)?;
    file.set_modified_wc(SystemTime::now())?;
    Ok(())
}

pub fn run_cargo_subcommand_on_all_nested_workspace_roots<T: AsRef<OsStr> + Debug>(
    subcommand: &CargoSubcommand,
    args: &[T],
    dir: &Path,
    is_recursive_call: bool,
) -> Result<()> {
    let roots = all_nested_workspace_roots(dir)?;
    env_logger::try_init().unwrap_or_default();
    if warn_if_no_nested_workspaces(&roots, Some(dir), is_recursive_call)? {
        return Ok(());
    }
    for root in &roots {
        let _delimiter = Delimiter::new(&root.path);
        let command =
            build_cargo_command(Source::CargoNested, Some(&root.package), subcommand, args)?;
        run_cargo_command(Source::CargoNested, root, command)?;
        // smoelius: `cargo nested` is a special case. It must be run manually on each nested
        // workspace root to ensure that _nested_-nested workspaces are handled.
        run_cargo_subcommand_on_all_nested_workspace_roots(subcommand, args, &root.path, true)?;
    }
    Ok(())
}

fn current_package_nested_workspace_roots() -> Result<Vec<NestedWorkspaceRoot>> {
    let cargo_manifest_path = var_wc("CARGO_MANIFEST_PATH")?;
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

pub fn all_nested_workspace_roots(dir: &Path) -> Result<Vec<NestedWorkspaceRoot>> {
    let mut roots = Vec::new();
    let cargo_metadata = MetadataCommand::new().current_dir(dir).no_deps().exec()?;
    for package in &cargo_metadata.packages {
        if let Some(current_roots) = nested_workspace_roots_for_package(package)? {
            roots.extend(current_roots);
        }
    }
    Ok(roots)
}

fn warn_if_no_nested_workspaces(
    roots: &[NestedWorkspaceRoot],
    dir: Option<&Path>,
    is_recursive_call: bool,
) -> Result<bool> {
    if roots.is_empty() && !is_recursive_call {
        let in_dir = dir.map_or_else(String::new, |dir| format!(" in `{}`", dir.display()));
        writeln!(stderr(), "Warning: found no nested workspaces{in_dir}")?;
    }
    Ok(roots.is_empty())
}

fn run_cargo_command(
    source: Source,
    root: &NestedWorkspaceRoot,
    mut command: Command,
) -> Result<()> {
    command.current_dir(&root.path);
    debug!("{source}: {command:?}");
    let status = command.status_wc()?;
    ensure!(status.success(), "command failed: {command:?}");
    Ok(())
}

fn nested_workspace_roots_for_package(
    package: &Package,
) -> Result<Option<Vec<NestedWorkspaceRoot>>> {
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
    let mut roots = Vec::new();
    for root in nested_workspace_metadata.roots {
        for result in glob(&format!("{cargo_manifest_dir}/{}", root.path()))? {
            let path = result?;
            if !validate_root(&path)? {
                writeln!(
                    stderr(),
                    "Warning: skipping `{}` as it does not contain a workspace",
                    path.display(),
                )?;
                continue;
            }
            roots.push(NestedWorkspaceRoot {
                path,
                package: PackageContext {
                    name: package.name.to_string(),
                    dependent: root.dependent(),
                },
            });
        }
    }
    Ok(Some(roots))
}

/// Run `cargo metadata` in `root` and verify there is a workspace rooted there.
fn validate_root(root: &Path) -> Result<bool> {
    let cargo_metadata = MetadataCommand::new().current_dir(root).no_deps().exec()?;
    let root_canonical = dunce::canonicalize(root)?;
    Ok(root_canonical == cargo_metadata.workspace_root)
}
