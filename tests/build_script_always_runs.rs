use assert_cmd::{cargo::CommandCargoExt, output::OutputError};
use std::{
    collections::HashMap, env::remove_var, ffi::OsStr, fs::read_to_string, path::Path,
    process::Command,
};
use walkdir::WalkDir;

#[ctor::ctor]
fn initialize() {
    unsafe {
        remove_var("CARGO_TERM_COLOR");
    }
}

#[test]
fn build_script_always_runs() {
    for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR"))) {
        let entry = result.unwrap();
        let path = entry.path();
        if path.file_name() != Some(OsStr::new("build.rs")) {
            continue;
        }
        let manifest_path = path.with_file_name("Cargo.toml");
        eprintln!("{}", manifest_path.display());
        fetch(&manifest_path);
        check_then_build(&manifest_path);
    }
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

fn check_then_build(manifest_path: &Path) {
    let mut path_contents_map = HashMap::<String, String>::new();
    for build in [false, true] {
        if build {
            eprintln!("{path_contents_map:#?}");
            assert!(!path_contents_map.is_empty());
        }
        let mut command = Command::new("cargo");
        command.args([
            if build { "build" } else { "check" },
            "-vv",
            "--offline",
            "--manifest-path",
        ]);
        command.arg(manifest_path);
        let output = command.output().unwrap();
        if build {
            assert!(output.status.success());
        } else if !output.status.success() {
            eprintln!("command failed: {}", OutputError::new(output));
            return;
        }
        let stdout = String::from_utf8(output.stdout).unwrap();
        for line in stdout.lines().filter(|line| line.ends_with("/now.txt")) {
            let index = line.rfind('=').unwrap();
            let path = &line[index + 1..];
            let contents_curr = read_to_string(path).unwrap();
            if build {
                let contents_prev = path_contents_map.remove(path).unwrap();
                assert_ne!(*contents_prev, contents_curr);
            } else {
                path_contents_map.insert(path.to_owned(), contents_curr);
            }
        }
    }
    assert!(path_contents_map.is_empty());
}
