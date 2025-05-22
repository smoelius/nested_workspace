use anyhow::{Result, bail, ensure};
use nested_workspace::{
    CargoSubcommand, Source, build_cargo_command, parse_cargo_command, parse_cargo_subcommand,
    run_cargo_subcommand_on_all_nested_workspace_roots,
};
use std::env::{args, current_dir};

fn main() -> Result<()> {
    let args = args().collect::<Vec<_>>();

    let (subcommand, args) = parse_args(&args)?;

    // smoelius: Run on current package or workspace.
    let mut command = build_cargo_command(Source::CargoNw, None, &subcommand, args)?;
    let status = command.status()?;
    ensure!(status.success(), "command failed: {command:?}");

    // smoelius: Run on all nested workspaces.
    let current_dir = current_dir()?;
    run_cargo_subcommand_on_all_nested_workspace_roots(&subcommand, args, &current_dir, false)?;

    Ok(())
}

fn parse_args(args: &[String]) -> Result<(CargoSubcommand, &[String])> {
    let (subcommand, args) = parse_cargo_command(args)?;
    if !matches!(subcommand, CargoSubcommand::Other(other) if other == "nw") {
        bail!("failed to parse `cargo nw` arguments: {args:?}")
    }
    parse_cargo_subcommand(args)
}
