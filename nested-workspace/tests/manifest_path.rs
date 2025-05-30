use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, ExitStatus, Stdio},
};
use walkdir::WalkDir;

/// Verify that `cargo check` does not block when run on any Cargo.toml in this repository.
#[test]
fn manifest_path() {
    for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("..")) {
        let entry = result.unwrap();
        let path = entry.path();
        if path.file_name() != Some(OsStr::new("Cargo.toml")) {
            continue;
        }
        let mut command = Command::new("cargo");
        command.args(["check", "--manifest-path", path.to_str().unwrap()]);
        command.stderr(Stdio::null());
        // smoelius: `cargo check` may fail but it should not block.
        let _: ExitStatus = command.status().unwrap();
    }
}
