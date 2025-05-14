// smoelius: `rustc_private` is just for testing. This package will fail to build if `RUSTC` is not
// cleared.
#![feature(rustc_private)]

#[test]
fn test() {
    use rustc_version::{version_meta, Channel};

    assert_eq!(Channel::Nightly, version_meta().unwrap().channel);
}
