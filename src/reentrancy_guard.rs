use anyhow::{Result, bail};
use elaborate::std::env::var_wc;

pub fn check_reentrancy_guard() -> Result<bool> {
    let reentrancy_guard = reentrancy_guard()?;

    if enabled(&reentrancy_guard) {
        let dependent = dependent()?;
        if enabled(&dependent) {
            return Ok(true);
        }
        bail!("cycle detected: cannot run on nested workspaces");
    }

    Ok(false)
}

pub fn reentrancy_guard() -> Result<String> {
    var_wc("CARGO_PKG_NAME").map(|package_name| reentrancy_guard_from_package_name(&package_name))
}

pub fn reentrancy_guard_from_package_name(package_name: &str) -> String {
    format!("NESTED_WORKSPACE_REENTRANCY_GUARD_{package_name}")
}

fn dependent() -> Result<String> {
    var_wc("CARGO_PKG_NAME").map(|package_name| dependent_from_package_name(&package_name))
}

pub fn dependent_from_package_name(package_name: &str) -> String {
    format!("NESTED_WORKSPACE_DEPENDENT_{package_name}")
}

fn enabled(key: &str) -> bool {
    var_wc(key).is_ok_and(|value| value != "0")
}
