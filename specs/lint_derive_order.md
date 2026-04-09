# Derive order

r[lint.derive-order.detect]
The lint must flag `#[derive(...)]` attributes whose derives are not in the
canonical order.

r[lint.derive-order.std-order]
Standard library derives must appear first and in the following order: Copy,
Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default. Only the derives
that are present need to appear, but their relative order must match this
sequence.

r[lint.derive-order.third-party-after-std]
Third-party derives (any derive not in the standard library list) must appear
after all standard library derives.

r[lint.derive-order.third-party-alpha]
Third-party derives must be sorted alphabetically by crate, then by macro name.

r[lint.derive-order.message]
The diagnostic must show the expected ordering of the derives.
