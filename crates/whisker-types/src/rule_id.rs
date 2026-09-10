use std::cmp::Ordering;

use stabby::str::Str;

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
/// The string is held as a [`Str`], stabby's string slice, because the
/// identifier crosses the plugin boundary. A plugin returns the rules
/// it declares and stamps one on every diagnostic. Std promises no layout
/// for a string slice that holds from one compiler to the next.
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
///
/// [`Str`]: stabby::str::Str
#[stabby::stabby]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct RuleId(Str<'static>);

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
        Self(Str::new(id))
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
        self.0.as_str()
    }
}

impl Ord for RuleId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for RuleId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    fn debug_shows_the_name() {
        let id = RuleId::new("lint.test");

        let text = format!("{id:?}");

        assert_eq!(text, "RuleId(\"lint.test\")");
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
    fn ordering_follows_the_name() {
        let first = RuleId::new("lint.a");
        let second = RuleId::new("lint.b");

        assert!(first < second);
        assert_eq!(first.partial_cmp(&second), Some(Ordering::Less));
        assert_eq!(first.cmp(&first), Ordering::Equal);
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
