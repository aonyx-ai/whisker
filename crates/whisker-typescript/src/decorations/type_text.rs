use stabby::string;
use whisker_macros::Decoration;

/// The checker's rendering of the type of the value an identifier names
///
/// The TypeScript provider attaches one to every `identifier`,
/// `property_identifier`, and `shorthand_property_identifier` node that
/// the checker can type. The text is what the TypeScript checker prints
/// for that type, such as `number` or `(a: string) => void`. It is a
/// rendering for comparison and display, not a structure to walk.
///
/// A rule reads it out of memory the host allocated, so stabby lays it
/// out and the text is stabby's `String`.
///
/// # Examples
///
/// ```
/// use whisker_typescript::decorations::TypeText;
///
/// let ty = TypeText::new("number");
///
/// assert_eq!(ty.as_str(), "number");
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub struct TypeText {
    text: string::String,
}

impl TypeText {
    /// Creates the decoration from the checker's rendering of a type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::TypeText;
    ///
    /// let ty = TypeText::new("string[]");
    ///
    /// assert_eq!(ty.as_str(), "string[]");
    /// ```
    pub fn new(text: &str) -> Self {
        Self {
            text: string::String::from(text),
        }
    }

    /// Returns the rendered type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_typescript::decorations::TypeText;
    ///
    /// let ty = TypeText::new("number");
    ///
    /// assert_eq!(ty.as_str(), "number");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_the_text() {
        let ty = TypeText::new("(a: string) => void");

        let text = ty.as_str();

        assert_eq!(text, "(a: string) => void");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TypeText>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypeText>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypeText>();
    }
}
