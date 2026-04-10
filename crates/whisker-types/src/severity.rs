/// Severity level for a diagnostic
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Severity {
    /// A help message
    Help,
    /// An informational note
    Info,
    /// A warning that should be addressed
    Warn,
    /// An error that must be addressed
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Severity>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Severity>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Severity>();
    }

    #[test]
    fn ordering_error_is_greatest() {
        assert!(Severity::Error > Severity::Warn);
        assert!(Severity::Warn > Severity::Info);
        assert!(Severity::Info > Severity::Help);
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        fn arb_severity() -> impl Strategy<Value = Severity> {
            prop_oneof![
                Just(Severity::Help),
                Just(Severity::Info),
                Just(Severity::Warn),
                Just(Severity::Error),
            ]
        }

        proptest! {
            #[test]
            fn equal_variants_are_equal(a in arb_severity()) {
                #[allow(clippy::eq_op)]
                let is_eq = a == a;
                prop_assert!(is_eq);
            }

            #[test]
            fn ordering_is_transitive(
                a in arb_severity(),
                b in arb_severity(),
                c in arb_severity(),
            ) {
                if a <= b && b <= c {
                    prop_assert!(a <= c);
                }
            }

            #[test]
            fn equality_is_reflexive(a in arb_severity()) {
                prop_assert_eq!(a, a);
            }
        }
    }
}
