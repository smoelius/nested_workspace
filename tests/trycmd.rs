use anyhow::Result;
use std::{
    env::{remove_var, var},
    ffi::OsStr,
    fs::{OpenOptions, read_dir, read_to_string},
    path::{Path, absolute},
};
use trycmd::TestCases;

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
    for (subdir, _) in SUBDIR_ARGS {
        let test_cases = TestCases::new();

        test_cases.register_bin("cargo", Path::new(env!("CARGO")));

        test_cases.case(format!("tests/trycmd/{subdir}/*.toml"));
    }
}

#[test]
fn completeness() {
    let mut missing = Vec::new();
    for result in read_dir("fixtures").unwrap() {
        let entry = result.unwrap();
        let path = entry.path();
        let filename = path.file_name().unwrap();
        for (subdir, _) in SUBDIR_ARGS {
            if subdir == "before" || subdir == "after" {
                continue;
            }
            for extension in ["stderr", "stdout", "toml"] {
                let path = Path::new("tests/trycmd")
                    .join(subdir)
                    .join(filename)
                    .with_extension(extension);
                if !path.try_exists().unwrap() {
                    let path = absolute(path).unwrap();
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
        for result in read_dir(path).unwrap() {
            let entry = result.unwrap();
            let path = entry.path();
            if path.extension() != Some(OsStr::new("toml")) {
                continue;
            }
            let file_stem = path.file_stem().unwrap();
            let contents = read_to_string(&path).unwrap();
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

            assert_eq!(
                Some(args_expected),
                args_actual.as_deref(),
                "failed for `{}`",
                path.display()
            );

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

            let fixture = cwd.file_name().unwrap();

            assert_eq!(file_stem, fixture);
        }
    }
}

fn touch(path: &Path) -> Result<()> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(|_| ())
        .map_err(Into::into)
}

fn enabled(key: &str) -> bool {
    var(key).is_ok_and(|value| value != "0")
}
