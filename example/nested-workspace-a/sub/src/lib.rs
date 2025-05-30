pub fn sub(left: u64, right: u64) -> u64 {
    left - right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_works() {
        let result = sub(8, 2);
        assert_eq!(result, 6);
    }
}
