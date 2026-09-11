use std::any::Any;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};

use stabby::string;

/// What a plugin hands back when a call into it panicked
///
/// A panic must not unwind across the plugin boundary. The two sides may
/// have been built by different compilers, and each carries its own panic
/// runtime, so an unwind that crosses is undefined behavior. A panic that
/// reaches an `extern "C"` boundary aborts the process. A plugin therefore
/// catches every panic at its edge and hands the message back as one of
/// these. The host turns it into an error that names the file and the
/// node, and the run is over: a pass that panicked is broken.
///
/// Stabby lays the type out, because it crosses the boundary inside every
/// result a pass returns.
///
/// # Examples
///
/// ```
/// use whisker_types::Panic;
///
/// let caught = Panic::catch(|| panic!("boom"));
///
/// assert_eq!(caught.expect_err("should catch").message(), "boom");
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Panic {
    message: string::String,
}

impl Panic {
    /// Runs `work` and turns a panic inside it into a value
    ///
    /// This is the catch at the plugin's edge. The closure is treated as
    /// unwind safe, because nothing reads the state a panic interrupted.
    /// The host stops the run once it sees the panic, and the pass is
    /// dropped with it. The panic hook still runs, so the panic's location
    /// reaches stderr.
    ///
    /// The result is std's, so a caller can shape the value before it
    /// converts it into the result that crosses the boundary.
    ///
    /// # Errors
    ///
    /// Returns the panic, with its message, if `work` panicked.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::Panic;
    ///
    /// let counted = Panic::catch(|| "abc".len());
    ///
    /// assert_eq!(counted.expect("should not panic"), 3);
    /// ```
    pub fn catch<T>(work: impl FnOnce() -> T) -> Result<T, Panic> {
        panic::catch_unwind(AssertUnwindSafe(work))
            .map_err(|payload| Self::from_payload(payload.as_ref()))
    }

    /// Returns what the panic said
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::Panic;
    ///
    /// let caught = Panic::catch(|| panic!("at {}", 12));
    ///
    /// assert_eq!(caught.expect_err("should catch").message(), "at 12");
    /// ```
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Reads the message out of a panic payload
    ///
    /// `panic!` with a literal carries a `&str`, and one with a format
    /// string carries a `String`. Anything else is a payload some code
    /// chose on purpose, and nothing general can be read out of it.
    fn from_payload(payload: &(dyn Any + Send)) -> Self {
        let message = match payload.downcast_ref::<&str>() {
            Some(text) => *text,
            None => match payload.downcast_ref::<String>() {
                Some(text) => text.as_str(),
                None => "a panic whose payload is not a string",
            },
        };

        Self {
            message: string::String::from(message),
        }
    }
}

impl fmt::Display for Panic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Panic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catch_with_a_formatted_panic_returns_its_message() {
        let index = 7;

        let error = Panic::catch(|| -> u32 { panic!("index {index} is out of range") })
            .expect_err("should catch");

        assert_eq!(error.message(), "index 7 is out of range");
    }

    #[test]
    fn catch_with_a_literal_panic_returns_its_message() {
        let error = Panic::catch(|| -> u32 { panic!("boom") }).expect_err("should catch");

        assert_eq!(error.message(), "boom");
    }

    #[test]
    fn catch_with_another_payload_says_so() {
        let error = Panic::catch(|| -> u32 { panic::panic_any(42_u8) }).expect_err("should catch");

        assert_eq!(error.message(), "a panic whose payload is not a string");
    }

    #[test]
    fn catch_without_a_panic_returns_the_value() {
        let value = Panic::catch(|| 3).expect("should not panic");

        assert_eq!(value, 3);
    }

    #[test]
    fn display_shows_the_message() {
        let error = Panic::catch(|| -> u32 { panic!("boom") }).expect_err("should catch");

        let text = error.to_string();

        assert_eq!(text, "boom");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Panic>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Panic>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Panic>();
    }
}
