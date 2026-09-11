use anyhow::{Result, bail};
use nested_workspace::{
    Args, CargoSubcommand, ContainingPackage, Delimiter, Source, all_containing_packages,
    build_and_run_cargo_command, build_subcommand_and_args, parse_cargo_command,
    parse_cargo_subcommand, warn_about_missing_nested_workspaces,
};
use std::{
    env::{args, current_dir},
    ffi::OsStr,
    path::Path,
};

const USAGE: &str = concat!(
    "Usage: cargo nested [OPTIONS or Cargo SUBCOMMAND]\n",
    "\n",
    "Options:\n",
    "      --list     List current nested workspaces\n",
    "  -h, --help     Print help\n",
    "  -V, --version  Print version\n",
    "\n",
    "If a Cargo SUBCOMMAND is passed, it is run on the current package and all nested \
     workspaces.\n",
    "\n",
    "For example, the following command runs `cargo clean` on the current package and all nested \
     workspaces:\n",
    "\n",
    "    cargo nested clean"
);

enum Action {
    List,
    Help,
    Version,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = args().collect::<Vec<_>>();

    let Some((subcommand, inherited_args)) = parse_args(&args)? else {
        return Ok(());
    };

    // smoelius: Run on current package or workspace.
    let (subcommand_osstr, args) = build_subcommand_and_args(
        Source::CargoNested,
        None,
        &subcommand,
        &Args::inherited(inherited_args),
    )?;
    build_and_run_cargo_command(Source::CargoNested, None, subcommand_osstr, &args, None)?;

    // smoelius: Run on all nested workspaces.
    let current_dir = current_dir()?;
    run_cargo_subcommand_on_all_nested_workspace_roots(
        &subcommand,
        inherited_args,
        &current_dir,
        false,
    )?;

    Ok(())
}

fn parse_args(args: &[String]) -> Result<Option<(CargoSubcommand, &[String])>> {
    let Some((subcommand, args)) = parse_cargo_command(args)? else {
        bail!("failed to parse `cargo nested` arguments: {args:?}")
    };

    if !matches!(&subcommand, CargoSubcommand::Other(other) if other == "nested") {
        bail!("failed to parse `cargo nested` arguments: {subcommand} {args:?}")
    }

    if parse_cargo_nested_args(args)? {
        return Ok(None);
    }

    let (subcommand, args) = parse_cargo_subcommand(args)?;

    Ok(Some((subcommand, args)))
}

fn parse_cargo_nested_args(args: &[String]) -> Result<bool> {
    let mut args = args.iter();

    let Some(arg) = args.next() else { bail!(USAGE) };

    if !arg.starts_with('-') {
        return Ok(false);
    }

    let action = match arg.as_str() {
        "--list" => Action::List,
        "-h" | "--help" => Action::Help,
        "-V" | "--version" => Action::Version,
        _ => bail!("unrecognized argument: {arg}\n\n{USAGE}"),
    };

    if let Some(arg) = args.next() {
        bail!("unexpected argument: {arg}\n\n{USAGE}")
    }

    match action {
        Action::List => list_nested_workspaces()?,
        Action::Help => println!("{USAGE}"),
        Action::Version => println!("cargo-nested {}", env!("CARGO_PKG_VERSION")),
    }

    Ok(true)
}

fn list_nested_workspaces() -> Result<()> {
    let current_dir = current_dir()?;
    for containing_package in all_containing_packages(&current_dir)? {
        for root in containing_package.roots {
            let path = root
                .path()
                .strip_prefix(&current_dir)
                .unwrap_or(root.path());
            println!(
                "{}{}",
                path.display(),
                if root.dependent() { " (dependent)" } else { "" }
            );
        }
    }
    Ok(())
}

/// Runs a Cargo subcommand recursively on every nested workspace under `dir`.
// smoelius: `cargo nested` has no need to worry about containing packages and effectively ignores
// them.
fn run_cargo_subcommand_on_all_nested_workspace_roots<T: AsRef<OsStr>>(
    subcommand: &CargoSubcommand,
    inherited_args: &[T],
    dir: &Path,
    is_recursive_call: bool,
) -> Result<()> {
    let containing_packages = all_containing_packages(dir)?;
    if !containing_packages
        .iter()
        .any(ContainingPackage::has_nested_workspace_roots)
    {
        warn_about_missing_nested_workspaces(Some(dir), is_recursive_call)?;
        return Ok(());
    }
    for containing_package in &containing_packages {
        let (subcommand_osstr, args) = build_subcommand_and_args(
            Source::CargoNested,
            Some(&containing_package.name),
            subcommand,
            &Args::inherited(inherited_args),
        )?;
        for root in &containing_package.roots {
            let _delimiter = Delimiter::new(root.path());
            build_and_run_cargo_command(
                Source::CargoNested,
                Some(&containing_package.name),
                subcommand_osstr,
                &args,
                Some(root),
            )?;
            // smoelius: `cargo nested` is a special case. It must be run manually on each nested
            // workspace root to ensure that _nested_-nested workspaces are handled.
            run_cargo_subcommand_on_all_nested_workspace_roots(
                subcommand,
                inherited_args,
                root.path(),
                true,
            )?;
        }
    }
    Ok(())
}
