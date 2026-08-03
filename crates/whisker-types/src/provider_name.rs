/// Identifies a decoration provider in user-facing diagnostics
///
/// Providers are linked in at compile time rather than discovered at
/// runtime, so the name is always static — the same shape [`RuleId`] uses
/// for the same reason.
///
/// # Examples
///
/// ```
/// use whisker_types::ProviderName;
///
/// assert_eq!(ProviderName("rust").to_string(), "rust");
/// ```
///
/// [`RuleId`]: crate::RuleId
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ProviderName(pub &'static str);

impl ProviderName {
    /// Returns the string representation of this provider name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::ProviderName;
    ///
    /// assert_eq!(ProviderName("rust").as_str(), "rust");
    /// ```
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for ProviderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_inner_str() {
        let name = ProviderName("rust");

        let result = name.as_str();

        assert_eq!(result, "rust");
    }

    #[test]
    fn display_writes_name() {
        let name = ProviderName("rust");

        let result = name.to_string();

        assert_eq!(result, "rust");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ProviderName>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ProviderName>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ProviderName>();
    }
}
