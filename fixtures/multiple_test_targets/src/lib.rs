#[test]
fn nested_workspace() {
    nested_workspace::test().arg("--lib").unwrap();
}
