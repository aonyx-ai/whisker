/// Where a function's error type had to be read from
///
/// An `async fn`'s written return type is the opaque future, not the type
/// `?` in its body converts into. Recording which of the two was read means
/// a test can assert that the provider went through the future, rather than
/// asserting a result a provider ignoring `async` could also produce.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum ReturnMode {
    /// `?` sees the declared return type
    Direct,
    /// The function is `async`; `?` sees the future's output
    Awaited,
    /// The function is `async` and the future's output could not be
    /// projected, so nothing is known about what `?` sees
    Opaque,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_return_mode() {
        fn assert_send<T: Send>() {}
        assert_send::<ReturnMode>();
    }

    #[test]
    fn trait_sync_return_mode() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ReturnMode>();
    }

    #[test]
    fn trait_unpin_return_mode() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ReturnMode>();
    }
}
