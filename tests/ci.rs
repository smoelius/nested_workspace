use assert_cmd::assert::OutputAssertExt;
use regex::Regex;
use std::{env::remove_var, ffi::OsStr, fs::read_to_string, path::Path, process::Command};
use tempfile::tempdir;
use walkdir::WalkDir;

#[ctor::ctor]
fn initialize() {
    unsafe {
        remove_var("CARGO_TERM_COLOR");
    }
}

#[test]
fn clippy() {
    let status = Command::new("cargo")
        .args([
            "+nightly",
            "clippy",
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
fn doctests_are_disabled_for_example_and_fixtures() {
    for dir in ["example", "fixtures"] {
        for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join(dir)) {
            let entry = result.unwrap();
            if entry.file_name() != OsStr::new("Cargo.toml") {
                continue;
            }
            let path = entry.path();
            let manifest_dir = path.parent().unwrap();
            if !manifest_dir.join("src/lib.rs").try_exists().unwrap() {
                continue;
            }
            let contents = read_to_string(path).unwrap();
            let table = toml::from_str::<toml::Table>(&contents).unwrap();
            let doctest = table
                .get("lib")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("doctest"))
                .and_then(toml::Value::as_bool);
            assert_eq!(Some(false), doctest, "failed for `{}`", path.display());
        }
    }
}

#[test]
fn dylint() {
    let assert = Command::new("cargo")
        .args(["dylint", "--all", "--", "--all-targets"])
        .env("DYLINT_RUSTFLAGS", "--deny warnings")
        .assert();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(assert.try_success().is_ok(), "{}", stderr);
}

#[test]
fn fixtures_are_unpublishable() {
    for result in WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")) {
        let entry = result.unwrap();
        if entry.file_name() != OsStr::new("Cargo.toml") {
            continue;
        }
        let path = entry.path();
        let contents = read_to_string(path).unwrap();
        let table = toml::from_str::<toml::Table>(&contents).unwrap();
        let Some(package) = table.get("package").and_then(|value| value.as_table()) else {
            continue;
        };
        let publish = package.get("publish").and_then(toml::Value::as_bool);
        assert_eq!(Some(false), publish, "failed for `{}`", path.display());
    }
}

#[cfg_attr(target_os = "windows", ignore = "`markdown_link_check` not installed")]
#[test]
fn markdown_link_check() {
    let tempdir = tempdir().unwrap();

    Command::new("npm")
        .args(["install", "markdown-link-check"])
        .current_dir(&tempdir)
        .assert()
        .success();

    let readme_md = concat!(env!("CARGO_MANIFEST_DIR"), "/README.md");

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

#[cfg(all(unix, not(feature = "__disable_supply_chain_test")))]
#[test]
fn supply_chain() {
    use similar_asserts::SimpleDiff;
    use std::{fs::write, process::ExitStatus, str::FromStr};

    let mut command = Command::new("cargo");
    command.args(["supply-chain", "update", "--cache-max-age=0s"]);
    let _: ExitStatus = command.status().unwrap();

    let mut command = Command::new("cargo");
    command.args(["supply-chain", "json", "--no-dev"]);
    let assert = command.assert().success();

    let stdout_actual = std::str::from_utf8(&assert.get_output().stdout).unwrap();
    let mut value = serde_json::Value::from_str(stdout_actual).unwrap();
    remove_avatars(&mut value);
    let stdout_normalized = serde_json::to_string_pretty(&value).unwrap();

    let path_buf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/supply_chain.json");

    if enabled("BLESS") {
        write(path_buf, stdout_normalized).unwrap();
    } else {
        let stdout_expected = read_to_string(&path_buf).unwrap();

        assert!(
            stdout_expected == stdout_normalized,
            "{}",
            SimpleDiff::from_str(&stdout_expected, &stdout_normalized, "left", "right")
        );
    }
}

#[cfg(all(unix, not(feature = "__disable_supply_chain_test")))]
fn remove_avatars(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
        serde_json::Value::Array(array) => {
            for value in array {
                remove_avatars(value);
            }
        }
        serde_json::Value::Object(object) => {
            object.retain(|key, value| {
                if key == "avatar" {
                    return false;
                }
                remove_avatars(value);
                true
            });
        }
    }
}

#[cfg(all(unix, not(feature = "__disable_supply_chain_test")))]
fn enabled(key: &str) -> bool {
    use std::env::var;

    var(key).is_ok_and(|value| value != "0")
}
