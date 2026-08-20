/// The two inline link forms Markdown allows
///
/// The rule keeps them apart so the advice names the form the author wrote.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub(crate) enum LinkKind {
    /// An image link, written `![alt](url)`
    Image,
    /// A text link, written `[text](url)`
    Text,
}

impl LinkKind {
    /// Returns the diagnostic message for this form
    pub(crate) fn message(self) -> &'static str {
        match self {
            LinkKind::Image => "replace this inline image link with a reference-style image link",
            LinkKind::Text => "replace this inline link with a reference-style link",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_for_image_names_an_image_link() {
        let kind = LinkKind::Image;

        let message = kind.message();

        assert!(message.contains("inline image link"));
    }

    #[test]
    fn message_for_text_names_a_link() {
        let kind = LinkKind::Text;

        let message = kind.message();

        assert!(message.contains("inline link"));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LinkKind>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LinkKind>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LinkKind>();
    }
}
