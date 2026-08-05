/// How file discovery reacts to a failure the walk survives
///
/// An unreadable directory silently narrows the scan, and an unparsable
/// ignore file silently reshapes it. The policy decides whether the first
/// failure ends the walk, or whether the walk records it and continues.
///
/// # Examples
///
/// ```ignore
/// let policy = match keep_going {
///     true => WalkErrorPolicy::ReportAndContinue,
///     false => WalkErrorPolicy::Fail,
/// };
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum WalkErrorPolicy {
    /// Abandons the walk and returns the first failure
    #[default]
    Fail,

    /// Records the failure and keeps walking
    ReportAndContinue,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_fail() {
        let policy = WalkErrorPolicy::default();

        assert_eq!(policy, WalkErrorPolicy::Fail);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WalkErrorPolicy>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<WalkErrorPolicy>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<WalkErrorPolicy>();
    }
}
