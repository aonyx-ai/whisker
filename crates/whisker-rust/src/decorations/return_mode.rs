/// Where a function's error type comes from
///
/// For a plain function, the declared return type holds the error type. For
/// an `async fn`, the declared type is an opaque future, and the error type
/// is in the future's output.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum ReturnMode {
    /// The error type is in the declared return type
    Direct,
    /// The function is `async`; the error type is in the future's output
    Awaited,
    /// The function is `async`, but the future's output is unknown
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
