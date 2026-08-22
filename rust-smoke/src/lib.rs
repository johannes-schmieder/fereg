pub fn add(left: i64, right: i64) -> i64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_correctly() {
        assert_eq!(add(2, 3), 5);
    }
}
