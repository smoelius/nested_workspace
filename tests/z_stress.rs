//! Performs repeated calls to `cargo check` and `cargo +nighty check` in attempt to cause a
//! deadlock.
//!
//! Note that Nested Workspace always passes `--offline` in commands run on nested workspaces. Thus,
//! a deadlock should not occur.

use anyhow::{Result, ensure};
use elaborate::std::process::CommandContext;
use std::{process::Command, thread};

const N_ATTEMPTS: usize = 10;

#[test]
fn stress() {
    for i_attempt in 0..N_ATTEMPTS {
        dbg!(i_attempt);
        let handles = [false, true].map(|nightly| {
            thread::spawn(move || {
                check(nightly).unwrap();
            })
        });
        for handle in handles {
            handle.join().unwrap();
        }
    }
}

fn check(nightly: bool) -> Result<()> {
    let mut command = Command::new("cargo");
    if nightly {
        command.arg("+nightly");
    }
    command.args(["check", "-vv"]);
    command.current_dir("example");
    let status = command.status_wc().unwrap();
    ensure!(status.success());
    Ok(())
}
