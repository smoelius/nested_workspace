use anyhow::{Result, ensure};
use assert_cmd::{cargo::CommandCargoExt, output::OutputError};
use std::{env::remove_var, ffi::OsStr, path::Path, process::Command};
use walkdir::WalkDir;

mod util;
use util::NowContents;

#[ctor::ctor]
fn initialize() {
    unsafe {
        remove_var("CARGO_TERM_COLOR");
    }
}

#[test]
fn build_script_always_runs() {
    let mut failures = Vec::new();
    for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR"))) {
        let entry = result.unwrap();
        let path = entry.path();
        if path.file_name() != Some(OsStr::new("build.rs")) {
            continue;
        }
        let manifest_path = path.with_file_name("Cargo.toml");
        eprintln!("{}", manifest_path.display());
        fetch(&manifest_path);
        if let Err(error) = check_then_build(&manifest_path) {
            failures.push((manifest_path, error));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

// smoelius: `fetch` is for the `git_dependency` fixture. Note that we must use `cargo nw fetch` and
// not just `cargo fetch` because the command is run in the containing package's directory.
fn fetch(manifest_path: &Path) {
    let manifest_dir = manifest_path.parent().unwrap();
    let mut command = Command::cargo_bin("cargo-nw").unwrap();
    command.args(["nw", "fetch"]);
    command.current_dir(manifest_dir);
    let status = command.status().unwrap();
    assert!(status.success());
}

fn check_then_build(manifest_path: &Path) -> Result<()> {
    let mut now_contents_before = None;
    for build in [false, true] {
        let mut command = Command::new("cargo");
        command.args([
            if build { "build" } else { "check" },
            "-vv",
            "--offline",
            "--manifest-path",
        ]);
        command.arg(manifest_path);
        let output = command.output()?;
        if build {
            ensure!(output.status.success());
        } else {
            if !output.status.success() {
                eprintln!("command failed: {command:?}: {}", OutputError::new(output));
                return Ok(());
            }
            let stdout = String::from_utf8(output.stdout)?;
            let now_contents = NowContents::new(&stdout)?;
            eprintln!("`now_contents` before build: {:#?}", now_contents.get());
            ensure!(now_contents.get().is_empty() == false);
            now_contents_before = Some(now_contents);
        }
    }
    let now_contents_before = now_contents_before.unwrap();
    let now_contents_after = now_contents_before.rescan()?;
    eprintln!(
        "`now_contents` after build: {:#?}",
        now_contents_after.get()
    );
    now_contents_before.compare(&now_contents_after)?;
    Ok(())
}
