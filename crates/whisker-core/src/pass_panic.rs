use std::fmt;

use whisker_types::{Panic, Span};

/// A lint pass panicked while it checked a node
///
/// The pass caught the panic at the plugin boundary and handed it back as
/// a [`Panic`]. The walker adds where it happened: the kind of node under
/// check and the node's place in the file. The walk stops there. A pass
/// that panicked is broken, and whisker would check the rest of the file
/// with a pass in a state its author never meant.
///
/// The message carries everything, so the error can travel inside an
/// [`anyhow::Error`] without a source chain to unfold.
///
/// [`anyhow::Error`]: anyhow::Error
///
/// # Examples
///
/// ```
/// use whisker_core::PassPanic;
/// use whisker_types::{Panic, Span};
///
/// let panic = Panic::catch(|| -> () { panic!("no functions") }).expect_err("should catch");
///
/// let error = PassPanic::new("function_item", Span::new("src/lib.rs", 3, 9), panic);
///
/// assert_eq!(
///     error.to_string(),
///     "a lint pass panicked while checking a function_item at byte 3 of src/lib.rs: no functions"
/// );
/// ```
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct PassPanic {
    kind: String,
    span: Span,
    panic: Panic,
}

impl PassPanic {
    /// Records that a pass panicked while checking a node of `kind` at `span`
    pub fn new(kind: &str, span: Span, panic: Panic) -> Self {
        Self {
            kind: kind.to_owned(),
            span,
            panic,
        }
    }

    /// Returns the kind of the node the pass was checking
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns where the node sits in its file
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the panic the pass handed back
    pub fn panic(&self) -> &Panic {
        &self.panic
    }
}

impl fmt::Display for PassPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a lint pass panicked while checking a {} at byte {} of {}: {}",
            self.kind,
            self.span.start(),
            self.span.file().display(),
            self.panic
        )
    }
}

impl std::error::Error for PassPanic {}

#[cfg(test)]
mod tests {
    use super::*;

    fn panic() -> Panic {
        Panic::catch(|| -> () { panic!("no functions") }).expect_err("should catch")
    }

    #[test]
    fn accessors_return_each_part() {
        let error = PassPanic::new("function_item", Span::new("src/lib.rs", 3, 9), panic());

        assert_eq!(error.kind(), "function_item");
        assert_eq!(error.span().start(), 3);
        assert_eq!(error.panic().message(), "no functions");
    }

    #[test]
    fn display_names_the_node_the_file_and_the_message() {
        let error = PassPanic::new("function_item", Span::new("src/lib.rs", 3, 9), panic());

        let text = error.to_string();

        assert_eq!(
            text,
            "a lint pass panicked while checking a function_item at byte 3 of src/lib.rs: no functions"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PassPanic>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PassPanic>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PassPanic>();
    }
}
