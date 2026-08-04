use anyhow::{Result, bail, ensure};
use nested_workspace::{
    CargoSubcommand, Source, build_cargo_command, parse_cargo_command, parse_cargo_subcommand,
    run_cargo_subcommand_on_all_nested_workspace_roots,
};
use std::env::{args, current_dir};

const USAGE: &str = "Usage: cargo-nested [OPTIONS]";

fn main() -> Result<()> {
    let args = args().collect::<Vec<_>>();

    let Some((subcommand, args)) = parse_args(&args)? else {
        return Ok(());
    };

    // smoelius: Run on current package or workspace.
    let mut command = build_cargo_command(Source::CargoNested, None, &subcommand, args)?;
    let status = command.status()?;
    ensure!(status.success(), "command failed: {command:?}");

    // smoelius: Run on all nested workspaces.
    let current_dir = current_dir()?;
    run_cargo_subcommand_on_all_nested_workspace_roots(&subcommand, args, &current_dir, false)?;

    Ok(())
}

fn parse_args(args: &[String]) -> Result<Option<(CargoSubcommand, &[String])>> {
    let Some((subcommand, args)) = parse_cargo_command(args)? else {
        bail!("failed to parse `cargo nested` arguments: {args:?}")
    };

    if !matches!(&subcommand, CargoSubcommand::Other(other) if other == "nested") {
        bail!("failed to parse `cargo nested` arguments: {subcommand} {args:?}")
    }

    match args.first().map(String::as_str) {
        None => bail!(USAGE),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            return Ok(None);
        }
        Some("-V" | "--version") => {
            println!("cargo-nested {}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        }
        Some(arg) if arg.starts_with('-') => {
            bail!("unrecognized argument: {arg}\n\n{USAGE}")
        }
        Some(_) => {}
    }

    let (subcommand, args) = parse_cargo_subcommand(args)?;

    Ok(Some((subcommand, args)))
}
