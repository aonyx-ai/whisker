/// How file discovery reacts to a failure the walk survives
///
/// A directory that cannot be opened, or an entry that cannot be stat'd,
/// silently narrows the set of files a run inspects; an ignore file whose
/// syntax cannot be parsed silently reshapes it. For a linter both are the
/// worst available outcome: it reports success over a scan that never
/// happened. Discovery therefore surfaces every such failure, and mirrors the
/// promise `whisker check --keep-going` already makes about the files it does
/// manage to read.
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
