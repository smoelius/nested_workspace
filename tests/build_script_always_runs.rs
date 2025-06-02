use std::{collections::HashMap, ffi::OsStr, fs::read_to_string, path::Path, process::Command};
use walkdir::WalkDir;

#[test]
fn build_script_always_runs() {
    for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR"))) {
        let entry = result.unwrap();
        let path = entry.path();
        if path.file_name() != Some(OsStr::new("build.rs")) {
            continue;
        }
        let manifest_path = path.with_file_name("Cargo.toml");
        check_then_build(&manifest_path);
    }
}

fn check_then_build(manifest_path: &Path) {
    let mut path_contents_map = HashMap::<String, String>::new();
    for build in [false, true] {
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
            return;
        }
        let stdout = String::from_utf8(output.stdout).unwrap();
        for line in stdout.lines().filter(|line| line.ends_with("/now.txt")) {
            let index = line.rfind('=').unwrap();
            let path = &line[index + 1..];
            let contents_curr = read_to_string(path).unwrap();
            if build {
                let contents_prev = path_contents_map.get(path).unwrap();
                assert_ne!(*contents_prev, contents_curr);
            } else {
                path_contents_map.insert(path.to_owned(), contents_curr);
            }
        }
    }
}
