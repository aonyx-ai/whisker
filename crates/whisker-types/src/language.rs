/// Supported source languages
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Language {
    /// The Rust programming language
    Rust,
}

impl Language {
    /// Detects the language from a file extension
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::Language;
    ///
    /// assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    /// assert_eq!(Language::from_extension("py"), None);
    /// ```
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            _ => None,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rust => f.write_str("Rust"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Language>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Language>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Language>();
    }

    #[test]
    fn from_extension_with_rs_returns_rust() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    }

    #[test]
    fn from_extension_with_unknown_returns_none() {
        assert_eq!(Language::from_extension("py"), None);
        assert_eq!(Language::from_extension("js"), None);
        assert_eq!(Language::from_extension(""), None);
    }

    #[test]
    fn display_shows_language_name() {
        assert_eq!(Language::Rust.to_string(), "Rust");
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn non_rs_extension_returns_none(ext in "[a-z]{1,10}") {
                prop_assume!(ext != "rs");
                prop_assert_eq!(Language::from_extension(&ext), None);
            }
        }
    }
}
