#![warn(clippy::eq_op)]

pub fn eq(value: u64) -> bool {
    value == value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_works() {
        assert!(eq(1));
    }
}
