#[test]
fn test() {
    use rustc_version::{version_meta, Channel};

    assert_eq!(Channel::Nightly, version_meta().unwrap().channel);
}
