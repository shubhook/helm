//! Text storage: a rope behind a Helm trait, with transactions, undo, snapshots and UTF-16 offsets.

pub fn answer() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    #[test]
    fn deliberately_fails() {
        assert_eq!(super::answer(), 41);
    }
}
