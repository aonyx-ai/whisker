/// Identifies a lint rule
///
/// Each rule has a unique static string identifier following the convention
/// `category.rule-name` (e.g. `lint.wildcard-match-arm`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct RuleId(pub &'static str);

impl RuleId {
    /// Returns the string representation of this rule identifier
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RuleId>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RuleId>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RuleId>();
    }

    #[test]
    fn as_str_returns_inner() {
        let id = RuleId("lint.test");
        assert_eq!(id.as_str(), "lint.test");
    }

    #[test]
    fn display_matches_inner() {
        let id = RuleId("lint.test");
        assert_eq!(id.to_string(), "lint.test");
    }
}
