use anyhow::{Result, bail, ensure};
use nested_workspace::{
    Args, CargoSubcommand, Source, all_nested_workspace_roots, build_cargo_command,
    parse_cargo_command, parse_cargo_subcommand,
    run_cargo_subcommand_on_all_nested_workspace_roots,
};
use std::env::{args, current_dir};

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
    let args = args().collect::<Vec<_>>();

    let Some((subcommand, inherited_args)) = parse_args(&args)? else {
        return Ok(());
    };

    // smoelius: Run on current package or workspace.
    let mut command = build_cargo_command(
        Source::CargoNested,
        None,
        &subcommand,
        &Args::inherited(inherited_args),
    )?;
    let status = command.status()?;
    ensure!(status.success(), "command failed: {command:?}");

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
    for root in all_nested_workspace_roots(&current_dir)? {
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
    Ok(())
}
