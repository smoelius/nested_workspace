pub fn mul(left: u64, right: u64) -> u64 {
    left * right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_works() {
        let result = mul(8, 2);
        assert_eq!(result, 16);
    }
}
