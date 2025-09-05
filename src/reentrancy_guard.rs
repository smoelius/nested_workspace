use anyhow::{Result, bail};
use std::env::var;

pub fn check_reentrancy_guard() -> Result<()> {
    let reentrancy_guard = reentrancy_guard()?;

    if enabled(&reentrancy_guard) {
        bail!("cycle detected: cannot run on nested workspaces");
    }

    Ok(())
}

pub fn reentrancy_guard() -> Result<String> {
    var("CARGO_PKG_NAME")
        .map(|package_name| reentrancy_guard_from_package_name(&package_name))
        .map_err(Into::into)
}

pub fn reentrancy_guard_from_package_name(package_name: &str) -> String {
    format!("NESTED_WORKSPACE_REENTRANCY_GUARD_{package_name}")
}

fn enabled(key: &str) -> bool {
    var(key).is_ok_and(|value| value != "0")
}
