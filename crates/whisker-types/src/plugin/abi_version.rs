use std::fmt;

/// The version of the plugin declaration protocol
///
/// A plugin carries the version it was built for at offset zero of its
/// declaration, and whisker compares it against its own [`ABI_VERSION`].
/// The two components say different things, and the rule is cargo's:
///
/// - The minor rises when the protocol gains something an older plugin
///   simply lacks, such as a field appended to [`PluginDeclaration`].
///   That struct is `#[repr(C)]`, so whisker knows where each version's
///   fields end, and such a plugin offers less rather than being refused.
/// - The major rises when a plugin built for an earlier version cannot be
///   read at all. A method added to [`LintPass`] is such a change,
///   because it reorders a vtable and no offset arithmetic reaches around
///   that.
///
/// Major 0 is the exception, and it is the era whisker is in. Nothing is
/// promised there. [`floor`] is the version itself, so a plugin loads
/// only on the whisker it was built for. Every change to the boundary
/// costs one minor.
///
/// From 1.0 the major carries the promise. The two fingerprints beside
/// this field carry the other half of it: a type on either list changes
/// with the major, and not without it.
///
/// The fields are public rather than read through getters, for the
/// reason [`PluginDeclaration`]'s are. This is the head of a wire
/// format. The exporting macro writes it in a `const` context, and the
/// loader reads it field by field.
///
/// The derived order compares the major first, which is the order
/// [`accepts`] rests on.
///
/// This type's own layout is the one thing no version can ever change.
/// Whisker reads these eight bytes before it knows anything else about a
/// library, so it reads them the same way whatever the plugin says. A
/// protocol that laid them out differently would be read as though it
/// had not, and the version whisker compared would be a fiction.
///
/// [`ABI_VERSION`]: crate::plugin::ABI_VERSION
/// [`LintPass`]: crate::LintPass
/// [`PluginDeclaration`]: crate::plugin::PluginDeclaration
/// [`accepts`]: AbiVersion::accepts
/// [`floor`]: AbiVersion::floor
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::AbiVersion;
///
/// let version = AbiVersion { major: 1, minor: 2 };
///
/// assert_eq!(version.to_string(), "1.2");
/// ```
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AbiVersion {
    /// The component a plugin is refused over
    pub major: u32,

    /// The component a compatible change raises
    pub minor: u32,
}

impl AbiVersion {
    /// Returns the oldest version a whisker at this one still reads
    ///
    /// From 1.0 that is the major. A whisker at a later minor of one
    /// major reads every earlier minor of it. Before 1.0 it is the
    /// version itself, so a whisker reads only its own version.
    ///
    /// A publisher of prebuilt lints also reads this. The tag that names
    /// an archive carries the floor, so archives built for one whisker
    /// serve every whisker that reads them.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::plugin::AbiVersion;
    ///
    /// assert_eq!(AbiVersion { major: 1, minor: 2 }.floor(), AbiVersion { major: 1, minor: 0 });
    /// assert_eq!(AbiVersion { major: 0, minor: 2 }.floor(), AbiVersion { major: 0, minor: 2 });
    /// ```
    pub const fn floor(self) -> Self {
        match self.major {
            0 => self,
            major => Self { major, minor: 0 },
        }
    }

    /// Reports whether a whisker at this version reads `plugin`
    ///
    /// The range runs from [`floor`] up to this version. A plugin below
    /// it belongs to a protocol whisker no longer reads. A plugin above
    /// it was built for a whisker that knows more than this one does.
    ///
    /// [`floor`]: AbiVersion::floor
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::plugin::AbiVersion;
    ///
    /// let host = AbiVersion { major: 1, minor: 2 };
    ///
    /// assert!(host.accepts(AbiVersion { major: 1, minor: 0 }));
    /// assert!(!host.accepts(AbiVersion { major: 1, minor: 3 }));
    /// assert!(!host.accepts(AbiVersion { major: 0, minor: 9 }));
    /// ```
    ///
    /// Before 1.0 the two must be equal:
    ///
    /// ```
    /// use whisker_types::plugin::AbiVersion;
    ///
    /// let host = AbiVersion { major: 0, minor: 2 };
    ///
    /// assert!(host.accepts(AbiVersion { major: 0, minor: 2 }));
    /// assert!(!host.accepts(AbiVersion { major: 0, minor: 1 }));
    /// ```
    pub fn accepts(self, plugin: Self) -> bool {
        plugin >= self.floor() && plugin <= self
    }
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { major, minor } = self;

        write!(f, "{major}.{minor}")
    }
}

#[cfg(test)]
mod tests {
    use std::mem::offset_of;

    use super::*;

    #[test]
    fn accepts_a_newer_major_returns_false() {
        let host = AbiVersion { major: 1, minor: 2 };

        assert!(!host.accepts(AbiVersion { major: 2, minor: 0 }));
    }

    #[test]
    fn accepts_a_newer_minor_returns_false() {
        let host = AbiVersion { major: 1, minor: 2 };

        assert!(!host.accepts(AbiVersion { major: 1, minor: 3 }));
    }

    #[test]
    fn accepts_an_older_major_returns_false() {
        let host = AbiVersion { major: 2, minor: 0 };

        assert!(!host.accepts(AbiVersion { major: 1, minor: 0 }));
    }

    /// Before 1.0 an older minor is refused, which is the whole
    /// difference between the two eras.
    #[test]
    fn accepts_an_older_minor_before_one_point_zero_returns_false() {
        let host = AbiVersion { major: 0, minor: 2 };

        assert!(!host.accepts(AbiVersion { major: 0, minor: 1 }));
    }

    #[test]
    fn accepts_an_older_minor_from_one_point_zero_onward_returns_true() {
        let host = AbiVersion { major: 1, minor: 2 };

        assert!(host.accepts(AbiVersion { major: 1, minor: 0 }));
    }

    #[test]
    fn accepts_the_same_version_returns_true() {
        let host = AbiVersion { major: 0, minor: 1 };

        assert!(host.accepts(host));
    }

    #[test]
    fn floor_before_one_point_zero_is_the_version_itself() {
        let version = AbiVersion { major: 0, minor: 3 };

        assert_eq!(version.floor(), version);
    }

    #[test]
    fn floor_from_one_point_zero_onward_drops_the_minor() {
        let version = AbiVersion { major: 2, minor: 5 };

        assert_eq!(version.floor(), AbiVersion { major: 2, minor: 0 });
    }

    /// Pins the layout every protocol is read through
    ///
    /// Whisker reads these bytes before it knows which protocol wrote
    /// them, so a field that moved would make a plugin claim a version
    /// it never named.
    #[test]
    fn major_sits_first_and_the_pair_is_eight_bytes() {
        assert_eq!(offset_of!(AbiVersion, major), 0);
        assert_eq!(offset_of!(AbiVersion, minor), 4);
        assert_eq!(size_of::<AbiVersion>(), 8);
    }

    /// [`AbiVersion::accepts`] compares versions, so the derived order
    /// has to weigh the major first.
    #[test]
    fn ord_compares_the_major_before_the_minor() {
        assert!(AbiVersion { major: 1, minor: 0 } > AbiVersion { major: 0, minor: 9 });
        assert!(AbiVersion { major: 1, minor: 1 } > AbiVersion { major: 1, minor: 0 });
    }

    #[test]
    fn to_string_joins_the_components_with_a_dot() {
        let version = AbiVersion {
            major: 12,
            minor: 34,
        };

        assert_eq!(version.to_string(), "12.34");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AbiVersion>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AbiVersion>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AbiVersion>();
    }
}
