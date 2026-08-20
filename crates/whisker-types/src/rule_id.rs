/// Identifies a lint rule
///
/// Each rule has a unique static string identifier following the convention
/// `category.rule-name` (e.g. `lint.wildcard-match-arm`).
///
/// The string is private so that the only way to mint an identifier is
/// [`RuleId::new`], which takes a `&'static str` and is `const`. A rule
/// therefore still declares its identifier as an associated constant, while
/// nothing can build one out of a string assembled at runtime.
///
/// # Examples
///
/// ```
/// use whisker_types::RuleId;
///
/// const RULE_ID: RuleId = RuleId::new("lint.wildcard-match-arm");
///
/// assert_eq!(RULE_ID.as_str(), "lint.wildcard-match-arm");
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct RuleId(&'static str);

impl RuleId {
    /// Returns the identifier for a static rule name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleId;
    ///
    /// let id = RuleId::new("lint.bool-param");
    ///
    /// assert_eq!(id.to_string(), "lint.bool-param");
    /// ```
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Returns the string representation of this rule identifier
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleId;
    ///
    /// assert_eq!(RuleId::new("lint.derive-order").as_str(), "lint.derive-order");
    /// ```
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
    fn as_str_returns_inner() {
        let id = RuleId::new("lint.test");

        assert_eq!(id.as_str(), "lint.test");
    }

    #[test]
    fn display_matches_inner() {
        let id = RuleId::new("lint.test");

        assert_eq!(id.to_string(), "lint.test");
    }

    #[test]
    fn new_in_const_context_returns_the_name() {
        const RULE_ID: RuleId = RuleId::new("lint.const");

        assert_eq!(RULE_ID.as_str(), "lint.const");
    }

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
}
