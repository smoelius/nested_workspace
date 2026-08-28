use anyhow::Result;
use dir_entry_ext::DirEntryExt;
use elaborate::std::{
    env::{join_paths_wc, var_os_wc, var_wc},
    ffi::OsStrContext,
    fs::{OpenOptionsContext, read_dir_wc, read_to_string_wc},
    path::{PathContext, absolute_wc},
    process::CommandContext,
};
use regex::Regex;
use std::{
    collections::BTreeSet,
    env::split_paths,
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
    ("test", &["test"]),
    ("after", &[]),
];

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
fn test_completeness() {
    const EXTENSIONS: [&str; 3] = ["stderr", "stdout", "toml"];

    // Seed from fixtures so a fixture with no cases is reported, and supplement with case files so
    // dependent-entry cases such as `cycle__dependent`, which do not have corresponding top-level
    // fixtures, are also checked.
    let mut file_stems = read_dir_wc("fixtures")
        .unwrap()
        .map(|result| result.unwrap().file_name())
        .collect::<BTreeSet<_>>();
    for (subdir, _) in SUBDIR_ARGS {
        if subdir == "before" || subdir == "after" {
            continue;
        }
        let path = Path::new("tests/trycmd").join(subdir);
        for result in read_dir_wc(path).unwrap() {
            let entry = result.unwrap();
            if EXTENSIONS
                .iter()
                .any(|extension| entry.extension().as_deref() == Some(OsStr::new(extension)))
            {
                file_stems.insert(entry.path().file_stem_wc().unwrap().to_owned());
            }
        }
    }

    let mut missing = Vec::new();
    for file_stem in file_stems {
        for (subdir, _) in SUBDIR_ARGS {
            if subdir == "before" || subdir == "after" {
                continue;
            }
            // Dependent-entry cases exercise build-script behavior and do not have a meaningful
            // `cargo nested clean` counterpart. Running both entry points would also race while
            // cleaning the same build directory.
            if subdir == "nested_clean" && containing_and_dependent_file_stems(&file_stem).is_some()
            {
                continue;
            }
            for extension in EXTENSIONS {
                let path = Path::new("tests/trycmd")
                    .join(subdir)
                    .join(&file_stem)
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
fn test_correctness() {
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

            let fixture_suffix = containing_and_dependent_file_stems(file_stem).map_or_else(
                || PathBuf::from(file_stem),
                |(containing_file_stem, dependent_file_stem)| {
                    Path::new(containing_file_stem).join(dependent_file_stem)
                },
            );

            assert!(
                cwd.ends_with(&fixture_suffix),
                "`{}` does not end with `{}`",
                cwd.display(),
                fixture_suffix.display()
            );

            // A successful `build` should compile the fixture's root package. Assert that the
            // `.stderr` file pins down the `Compiling` line for it, rather than letting it be
            // swallowed by a `...` wildcard.
            let status_failed =
                table.get("status").and_then(|value| value.as_str()) == Some("failed");
            if subdir == "build" && file_stem != "runner" && !status_failed {
                let expected_prefix = containing_and_dependent_file_stems(file_stem).map_or_else(
                    || {
                        format!(
                            r"...
   Compiling {} v0.1.0 ([CWD])
",
                            file_stem.to_str_wc().unwrap()
                        )
                    },
                    |(containing, dependent)| {
                        format!(
                            r"...
   Compiling {containing} v0.1.0 ([..]/[PUT]/fixtures/{containing})
...
   Compiling {dependent} v0.1.0 ([CWD])
"
                        )
                    },
                );
                let stderr_path = path.with_extension("stderr");
                let stderr_contents = read_to_string_wc(&stderr_path).unwrap();
                assert!(
                    stderr_contents.starts_with(&expected_prefix),
                    "`{}` does not start with `{}`",
                    stderr_path.display(),
                    expected_prefix
                );
            }
        }
    }
}

#[test]
fn fixture_correctness() {
    for result in WalkDir::new("fixtures") {
        let entry = result.unwrap();
        if entry.file_name() != OsStr::new("Cargo.toml") {
            continue;
        }
        let path = entry.path();
        let contents = read_to_string_wc(path).unwrap();
        let table = toml::from_str::<toml::Table>(&contents).unwrap();

        let has_package = table.get("package").and_then(|value| value.as_table());

        if let Some(package) = has_package {
            assert_eq!(
                Some("2024"),
                package.get("edition").and_then(|value| value.as_str()),
                "`{}` does not have `edition = \"2024\"`",
                path.display()
            );
        }

        // A package's `edition` determines its default resolver version, so a manifest with a
        // `[package]` table should not set `resolver`. But a virtual manifest (no `[package]`
        // table) does not derive a default resolver from its members' editions. So it must set
        // `resolver` explicitly to avoid a warning from Cargo.
        if let Some(workspace) = table.get("workspace").and_then(|value| value.as_table()) {
            let resolver = workspace.get("resolver").and_then(|value| value.as_str());
            if has_package.is_some() {
                assert_eq!(
                    None,
                    resolver,
                    "`{}` has a redundant `resolver` key",
                    path.display()
                );
            } else {
                assert_eq!(
                    Some("3"),
                    resolver,
                    "`{}` does not have `resolver = \"3\"`",
                    path.display()
                );
            }
        }
    }
}

fn containing_and_dependent_file_stems(file_stem: &OsStr) -> Option<(&str, &str)> {
    file_stem.to_str_wc().unwrap().rsplit_once("__")
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

fn enabled(key: &str) -> bool {
    var_wc(key).is_ok_and(|value| value != "0")
}

fn touch(path: &Path) -> Result<()> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open_wc(path)
        .map(|_| ())
}
