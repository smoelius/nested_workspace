use assert_cmd::assert::OutputAssertExt;
use cargo_metadata::MetadataCommand;
use elaborate::std::process::CommandContext;
use std::{path::PathBuf, process::Command, sync::LazyLock};
use tempfile::tempdir;

static CARGO_NESTED: LazyLock<PathBuf> = LazyLock::new(|| {
    Command::new("cargo")
        .args(["build", "--package", "cargo-nested", "--offline"])
        .assert()
        .success();
    let metadata = MetadataCommand::new().no_deps().exec().unwrap();
    metadata
        .target_directory
        .join("debug/cargo-nested")
        .into_std_path_buf()
});

#[test]
fn check_does_not_warn_about_offline() {
    let target_dir = tempdir().unwrap();
    let output = Command::new(&*CARGO_NESTED)
        .args(["nested", "check", "-vv"])
        .env("CARGO_TARGET_DIR", target_dir.path())
        .current_dir("example")
        .output_wc()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("nested_workspace.timestamp"));
    assert!(!stderr.contains("Since `--offline` was not passed"));
}

#[test]
fn list_cycle() {
    let output = Command::new(&*CARGO_NESTED)
        .args(["nested", "--list"])
        .current_dir("fixtures/cycle")
        .output_wc()
        .unwrap();

    output.assert().success().stdout("dependent\n").stderr("");
}

#[test]
fn list_cycle_with_dependent() {
    let output = Command::new(&*CARGO_NESTED)
        .args(["nested", "--list"])
        .current_dir("fixtures/cycle_with_dependent")
        .output_wc()
        .unwrap();

    output
        .assert()
        .success()
        .stdout("dependent (dependent)\n")
        .stderr("");
}

#[test]
fn list_no_roots() {
    let output = Command::new(&*CARGO_NESTED)
        .args(["nested", "--list"])
        .current_dir("fixtures/no_roots")
        .output_wc()
        .unwrap();

    output.assert().success().stdout("").stderr("");
}
