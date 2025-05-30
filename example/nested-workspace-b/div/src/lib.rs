pub fn div(left: u64, right: u64) -> u64 {
    left / right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div_works() {
        let result = div(8, 2);
        assert_eq!(result, 4);
    }
}
