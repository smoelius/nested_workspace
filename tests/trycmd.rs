use anyhow::Result;
use dir_entry_ext::DirEntryExt;
use elaborate::std::{
    env::{join_paths_wc, var_os_wc, var_wc},
    fs::{OpenOptionsContext, create_dir_wc, read_dir_wc, read_to_string_wc, write_wc},
    path::{PathContext, absolute_wc},
    process::CommandContext,
};
use regex::Regex;
use std::{
    env::{remove_var, split_paths},
    ffi::{OsStr, OsString},
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::Command,
};
use trycmd::TestCases;
use walkdir::WalkDir;

// smoelius: The following order is intentional.
const SUBDIR_ARGS: [(&str, &[&str]); 6] = [
    ("before", &[]),
    ("nested_clean", &["nested", "clean"]),
    ("check", &["check", "-vv", "--offline"]),
    ("build", &["build", "-vv", "--offline"]),
    ("test", &["test", "--workspace"]),
    ("after", &[]),
];

#[ctor::ctor]
fn initialize() {
    unsafe {
        remove_var("CARGO_TERM_COLOR");
    }
}

#[test]
fn trycmd() {
    build_runner();

    let paths = prepend_to_paths(Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug"))
        .into_string()
        .unwrap();

    for (subdir, _) in SUBDIR_ARGS {
        let test_cases = TestCases::new();

        test_cases.insert_var("[PUT]", "nested_workspace").unwrap();

        test_cases.env("PATH", &paths);

        test_cases.register_bin("cargo", Path::new(env!("CARGO")));

        test_cases.case(format!("tests/trycmd/{subdir}/*.toml"));
    }
}

fn build_runner() {
    let mut command = Command::new("cargo");
    command.args(["build", "--package", "runner"]);
    let status = command.status_wc().unwrap();
    assert!(status.success());
}

fn prepend_to_paths(path: PathBuf) -> OsString {
    let paths = var_os_wc("PATH").unwrap();
    let paths_split = split_paths(&paths);
    let paths_prepended = std::iter::once(path).chain(paths_split);
    join_paths_wc(paths_prepended).unwrap()
}

#[test]
fn completeness() {
    let mut missing = Vec::new();
    for result in read_dir_wc("fixtures").unwrap() {
        let entry = result.unwrap();
        let filename = entry.file_name();
        for (subdir, _) in SUBDIR_ARGS {
            if subdir == "before" || subdir == "after" {
                continue;
            }
            for extension in ["stderr", "stdout", "toml"] {
                let path = Path::new("tests/trycmd")
                    .join(subdir)
                    .join(&filename)
                    .with_extension(extension);
                if !path.try_exists_wc().unwrap() {
                    let path = absolute_wc(path).unwrap();
                    missing.push(path);
                }
            }
        }
    }
    if !missing.is_empty() {
        let bless = enabled("BLESS");
        eprintln!("The following files are missing:");
        for path in missing {
            eprintln!("    {}", path.display());
            if bless {
                touch(&path).unwrap();
            }
        }
        panic!();
    }
}

#[test]
fn correctness() {
    for (subdir, args_expected) in SUBDIR_ARGS {
        if subdir == "before" || subdir == "after" {
            continue;
        }
        let path = Path::new("tests/trycmd").join(subdir);
        for result in read_dir_wc(path).unwrap() {
            let entry = result.unwrap();
            if entry.extension().as_deref() != Some(OsStr::new("toml")) {
                continue;
            }
            let path = entry.path();
            let file_stem = path.file_stem_wc().unwrap();
            let contents = read_to_string_wc(&path).unwrap();
            let table = toml::from_str::<toml::Table>(&contents).unwrap();

            let args_actual = table
                .get("args")
                .and_then(|value| value.as_array())
                .and_then(|array| {
                    array
                        .iter()
                        .map(|value| value.as_str())
                        .collect::<Option<Vec<_>>>()
                });

            if file_stem == "runner" {
                assert!(
                    args_actual.unwrap().starts_with(args_expected),
                    "failed for `{}`",
                    path.display()
                );
            } else {
                assert_eq!(
                    Some(args_expected),
                    args_actual.as_deref(),
                    "failed for `{}`",
                    path.display()
                );
            }

            let bin = table
                .get("bin")
                .and_then(|value| value.as_table())
                .and_then(|table| table.get("name"))
                .and_then(|value| value.as_str())
                .unwrap();

            if subdir == "nested_clean" {
                assert_eq!("cargo-nested", bin);
            } else {
                assert_eq!("cargo", bin);
            }

            let cwd = table
                .get("fs")
                .and_then(|value| value.as_table())
                .and_then(|table| table.get("cwd"))
                .and_then(|value| value.as_str())
                .map(Path::new)
                .unwrap();

            let fixture = cwd.file_name_wc().unwrap();

            assert_eq!(file_stem, fixture);
        }
    }
}

#[test]
fn no_decimal_times() {
    let re = Regex::new(r"\b[0-9]+\.[0-9]+s").unwrap();
    for result in WalkDir::new("tests/trycmd") {
        let entry = result.unwrap();
        if entry.extension() != Some(OsStr::new("stdout")) {
            continue;
        }
        let path = entry.path();
        let contents = read_to_string_wc(path).unwrap();
        assert!(!re.is_match(&contents), "{} matches", path.display());
    }
}

#[test]
fn cargo_configs() {
    for result in read_dir_wc("fixtures").unwrap() {
        let entry = result.unwrap();
        for (i, result) in WalkDir::new(entry.path())
            .into_iter()
            .filter_entry(|entry| {
                // smoelius: `multiple_toolchains` gets special treatment because it does not use
                // the stable toolchain.
                entry.file_name() != "multiple_toolchains"
                    && entry.path().join("Cargo.toml").exists()
            })
            .enumerate()
        {
            let entry = result.unwrap();
            let path = entry.path();
            assert_cargo_config(path, &subdir(i));
        }
    }
}

fn subdir(i: usize) -> String {
    let c = u32::try_from(i)
        .ok()
        .and_then(|i| char::from_u32(97 + i))
        .unwrap();
    format!("{c}")
}

#[test]
fn cargo_configs_multiple_toolchains() {
    assert_cargo_config("fixtures/multiple_toolchains/beta", "beta");
    assert_cargo_config("fixtures/multiple_toolchains/nightly", "nightly");
}

fn assert_cargo_config(fixture_path: impl AsRef<Path>, subdir: &str) {
    let fixture_path = fixture_path.as_ref();
    let expected_contents = format!(
        r#"[build]
target-dir = "{}/target_fixtures/{}"
"#,
        path_hops(fixture_path),
        subdir
    );
    if enabled("BLESS") {
        let cargo_path = fixture_path.join(".cargo");
        create_dir_wc(&cargo_path).unwrap_or_default();
        write_wc(cargo_path.join("config.toml"), expected_contents).unwrap();
    } else {
        let actual_contents = read_to_string_wc(fixture_path.join(".cargo/config.toml")).unwrap();
        assert_eq!(expected_contents, actual_contents);
    }
}

fn path_hops(path: &Path) -> String {
    let mut components = path.components().peekable();
    let component = components.peek().unwrap();
    assert!(component.as_os_str() == "fixtures");
    let mut hops = String::new();
    for _ in components {
        if !hops.is_empty() {
            hops.push('/');
        }
        hops.push_str("..");
    }
    hops
}

fn touch(path: &Path) -> Result<()> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open_wc(path)
        .map(|_| ())
}

fn enabled(key: &str) -> bool {
    var_wc(key).is_ok_and(|value| value != "0")
}
