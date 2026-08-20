use std::mem::{align_of, size_of};

/// One type's contribution to a boundary fingerprint
///
/// A plugin and its host agree on a type when they place it in memory
/// identically. Size and alignment catch a type that grew, shrank, or
/// changed its alignment; the offsets a caller supplies catch fields that
/// swapped places without changing either. Nothing here reads a field's
/// name or its type, so renaming a private field costs nothing and moving
/// one is refused.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Shape {
    size: usize,
    align: usize,
    offsets: &'static [usize],
}

impl Shape {
    /// Returns the shape of `T`, with no field offsets recorded
    ///
    /// Use this for a type that crosses the boundary whole, such as an
    /// enum the host only ever matches on, and [`Shape::of_fields`] for one
    /// whose fields both images address.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::plugin::Shape;
    ///
    /// const SHAPE: Shape = Shape::of::<u64>();
    ///
    /// assert_eq!(SHAPE.size(), 8);
    /// ```
    pub const fn of<T>() -> Self {
        Self {
            size: size_of::<T>(),
            align: align_of::<T>(),
            offsets: &[],
        }
    }

    /// Returns the shape of `T` together with the offsets of its fields
    ///
    /// Pass every offset the two images depend on, in a fixed order. The
    /// caller supplies them because [`offset_of`] only reaches fields its
    /// own crate can name.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::mem::offset_of;
    ///
    /// use whisker_types::plugin::Shape;
    ///
    /// struct Pair {
    ///     first: u32,
    ///     second: u32,
    /// }
    ///
    /// const SHAPE: Shape = Shape::of_fields::<Pair>(&[offset_of!(Pair, first)]);
    ///
    /// assert_eq!(SHAPE.size(), 8);
    /// ```
    ///
    /// [`offset_of`]: std::mem::offset_of
    pub const fn of_fields<T>(offsets: &'static [usize]) -> Self {
        Self {
            size: size_of::<T>(),
            align: align_of::<T>(),
            offsets,
        }
    }

    /// Returns the size this type occupies
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::plugin::Shape;
    ///
    /// assert_eq!(Shape::of::<u32>().size(), 4);
    /// ```
    pub const fn size(&self) -> usize {
        self.size
    }
}

/// Combines the shapes of every type that crosses the boundary into one hash
///
/// The result stands in for "these two images lay the boundary out the
/// same way". It is FNV-1a, chosen because it is a handful of const
/// operations rather than because it resists anything: the fingerprint
/// detects drift between two builds of the same project, and a plugin that
/// wants to lie about its layout can simply report the host's number.
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::{Shape, fingerprint};
///
/// const A: u64 = fingerprint(&[Shape::of::<u32>()]);
/// const B: u64 = fingerprint(&[Shape::of::<u64>()]);
///
/// assert_ne!(A, B);
/// ```
pub const fn fingerprint(shapes: &[Shape]) -> u64 {
    seeded_fingerprint(0xcbf2_9ce4_8422_2325, shapes)
}

/// Combines shapes with a value that is already a hash of something else
///
/// A language crate has more than layouts to answer for: whisker-rust
/// generates its lint pass trait from a grammar, and the generated source
/// is hashed at build time. This folds that hash in alongside the layouts
/// rather than leaving the two to be combined by hand.
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::{Shape, seeded_fingerprint};
///
/// const A: u64 = seeded_fingerprint(1, &[Shape::of::<u32>()]);
/// const B: u64 = seeded_fingerprint(2, &[Shape::of::<u32>()]);
///
/// assert_ne!(A, B);
/// ```
pub const fn seeded_fingerprint(seed: u64, shapes: &[Shape]) -> u64 {
    let mut hash = mix(0xcbf2_9ce4_8422_2325, seed);

    let mut index = 0;
    while index < shapes.len() {
        let shape = shapes[index];
        hash = mix(hash, shape.size as u64);
        hash = mix(hash, shape.align as u64);

        let mut field = 0;
        while field < shape.offsets.len() {
            hash = mix(hash, shape.offsets[field] as u64);
            field += 1;
        }

        hash = mix(hash, shape.offsets.len() as u64);
        index += 1;
    }

    mix(hash, shapes.len() as u64)
}

/// Folds one value into a running FNV-1a hash, byte by byte
const fn mix(hash: u64, value: u64) -> u64 {
    let bytes = value.to_le_bytes();

    let mut hash = hash;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_distinguishes_reordered_fields() {
        let before = fingerprint(&[Shape::of_fields::<u64>(&[0, 4])]);

        let after = fingerprint(&[Shape::of_fields::<u64>(&[4, 0])]);

        assert_ne!(before, after);
    }

    #[test]
    fn fingerprint_distinguishes_shapes_of_equal_size() {
        let one = fingerprint(&[Shape::of_fields::<u64>(&[0])]);

        let other = fingerprint(&[Shape::of_fields::<u64>(&[0, 0])]);

        assert_ne!(one, other);
    }

    #[test]
    fn fingerprint_of_the_same_shapes_is_stable() {
        let shapes = [Shape::of::<u32>(), Shape::of_fields::<u64>(&[0, 4])];

        let repeated = fingerprint(&shapes);

        assert_eq!(fingerprint(&shapes), repeated);
    }

    #[test]
    fn fingerprint_matches_its_default_seed() {
        let seeded = seeded_fingerprint(0xcbf2_9ce4_8422_2325, &[Shape::of::<u32>()]);

        let plain = fingerprint(&[Shape::of::<u32>()]);

        assert_eq!(plain, seeded);
    }

    #[test]
    fn fingerprint_of_the_empty_boundary_is_not_zero() {
        let empty = fingerprint(&[]);

        assert_ne!(empty, 0);
    }

    #[test]
    fn fingerprint_orders_its_shapes() {
        let one = fingerprint(&[Shape::of::<u32>(), Shape::of::<u64>()]);

        let other = fingerprint(&[Shape::of::<u64>(), Shape::of::<u32>()]);

        assert_ne!(one, other);
    }

    #[test]
    fn of_fields_records_the_offsets_it_is_given() {
        let shape = Shape::of_fields::<u64>(&[0, 4]);

        assert_eq!(shape.offsets, &[0, 4]);
    }

    #[test]
    fn of_records_no_offsets() {
        let shape = Shape::of::<u64>();

        assert!(shape.offsets.is_empty());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Shape>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Shape>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Shape>();
    }
}
