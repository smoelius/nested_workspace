//! Performs repeated calls to `cargo check` and `cargo +nighty check` in attempt to cause a
//! deadlock.
//!
//! If either thread runs without `--offline`, the test is likely to deadlock.

use anyhow::{Result, ensure};
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
    command.args([
        "check",
        "-vv",
        "--features=nested_workspace/__disable_offline_check",
    ]);
    // smoelius: Commenting out the next line should cause a deadlock.
    command.arg("--offline");
    command.current_dir("example");
    let status = command.status().unwrap();
    ensure!(status.success());
    Ok(())
}
