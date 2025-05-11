#[test]
fn to_bytes() {
    use to_bytes::ToBytes;

    assert_eq!(Some(1), 1_i32.to_bytes().first().copied());
}
