#[cfg(all(feature = "big-endian", feature = "little-endian"))]
compile_error!("Features `big-endian` and `little-endian` cannot be enabled simultaneously");

pub trait ToBytes {
    fn to_bytes(&self) -> [u8; 4];
}

#[cfg(feature = "big-endian")]
impl ToBytes for i32 {
    fn to_bytes(&self) -> [u8; 4] {
        self.to_be_bytes()
    }
}

#[cfg(feature = "little-endian")]
impl ToBytes for i32 {
    fn to_bytes(&self) -> [u8; 4] {
        self.to_le_bytes()
    }
}
