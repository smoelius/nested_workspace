use assert_cmd::assert::OutputAssertExt;
use regex::Regex;
use std::{
    env::{remove_var, set_current_dir},
    ffi::OsStr,
    fs::read_to_string,
    path::Path,
    process::Command,
};
use tempfile::tempdir;
use toml::{Table, Value};
use walkdir::WalkDir;

#[ctor::ctor]
fn initialize() {
    unsafe {
        remove_var("CARGO_TERM_COLOR");
    }
    set_current_dir("..");
}

#[test]
fn clippy() {
    let status = Command::new("cargo")
        .args([
            "+nightly",
            "clippy",
            "--all-features",
            "--all-targets",
            "--offline",
            "--",
            "--deny=warnings",
        ])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn doctests_are_disabled() {
    for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("..")) {
        let entry = result.unwrap();
        let path = entry.path();
        if path.file_name() != Some(OsStr::new("Cargo.toml")) {
            continue;
        }
        let manifest_dir = path.parent().unwrap();
        if !manifest_dir.join("src/lib.rs").try_exists().unwrap() {
            continue;
        }
        let contents = read_to_string(path).unwrap();
        let table = toml::from_str::<Table>(&contents).unwrap();
        let doctest = table
            .get("lib")
            .and_then(Value::as_table)
            .and_then(|table| table.get("doctest"))
            .and_then(Value::as_bool);
        assert_eq!(Some(false), doctest, "failed for `{}`", path.display());
    }
}

#[test]
fn dylint() {
    let assert = Command::new("cargo")
        .args(["dylint", "--all", "--", "--all-features", "--all-targets"])
        .env("DYLINT_RUSTFLAGS", "--deny warnings")
        .assert();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(assert.try_success().is_ok(), "{}", stderr);
}

#[test]
fn fixtures_are_unpublishable() {
    for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")) {
        let entry = result.unwrap();
        let path = entry.path();
        if path.file_name() != Some(OsStr::new("Cargo.toml")) {
            continue;
        }
        let contents = read_to_string(path).unwrap();
        let table = toml::from_str::<Table>(&contents).unwrap();
        let Some(package) = table.get("package").and_then(|value| value.as_table()) else {
            continue;
        };
        let publish = package.get("publish").and_then(Value::as_bool);
        assert_eq!(Some(false), publish, "failed for `{}`", path.display());
    }
}

#[cfg_attr(target_os = "windows", ignore)]
#[test]
fn markdown_link_check() {
    let tempdir = tempdir().unwrap();

    Command::new("npm")
        .args(["install", "markdown-link-check"])
        .current_dir(&tempdir)
        .assert()
        .success();

    let readme_md = concat!(env!("CARGO_MANIFEST_DIR"), "/../README.md");

    Command::new("npx")
        .args(["markdown-link-check", readme_md])
        .current_dir(&tempdir)
        .assert()
        .success();
}

#[test]
fn msrv() {
    let status = Command::new("cargo")
        .args(["msrv", "verify"])
        .current_dir("nested_workspace")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn readme_reference_links_are_sorted() {
    let re = Regex::new(r"^\[[^^\]]*\]:").unwrap();
    let readme = read_to_string("README.md").unwrap();
    let links = readme
        .lines()
        .filter(|line| re.is_match(line))
        .collect::<Vec<_>>();
    let mut links_sorted = links.clone();
    links_sorted.sort_unstable();
    assert!(
        links_sorted == links,
        "contents of README.md are not what was expected:\n{}",
        links_sorted.join("\n")
    );
}
