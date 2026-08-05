use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

/// A directory whose permissions are restored when this value is dropped
///
/// The walk-error tests have to survive a panic between clearing a
/// directory's mode and putting it back: a temporary directory left at mode
/// `000` cannot be removed, so an assertion failure would be followed by a
/// second, unrelated failure from `TempDir`'s own cleanup, and the leftover
/// directory would outlive the test run.
///
/// The unit tests in `src/discovery.rs` carry the same guard. An integration
/// test cannot reach into the binary crate it drives, so the duplication buys
/// the only thing that makes the CLI's own handling of a walk error testable
/// at all.
pub struct Unreadable<'a> {
    directory: &'a Path,
}

impl Drop for Unreadable<'_> {
    fn drop(&mut self) {
        make_readable(self.directory);
    }
}

/// Restores default permissions so the temporary directory can be removed
fn make_readable(directory: &Path) {
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755))
        .expect("permissions should be restored");
}

/// Makes `directory` impossible to read until the returned value is dropped
///
/// A process with enough privilege - root in a CI container, most often - can
/// still read a mode `000` directory. A caller that quietly bowed out there
/// would report a passing test having asserted nothing at all about the
/// walk-error path, which is the very failure mode this crate exists to
/// prevent, so this stops loudly instead.
///
/// # Panics
///
/// Panics if `directory` cannot have its mode cleared, or if it is still
/// readable afterwards.
pub fn make_unreadable(directory: &Path) -> Unreadable<'_> {
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o000))
        .expect("permissions should be set");

    let unreadable = Unreadable { directory };

    assert!(
        std::fs::read_dir(directory).is_err(),
        "{} is still readable at mode 000, so the walk-error tests cannot exercise anything; run \
         them without privileges that bypass file permissions",
        directory.display()
    );

    unreadable
}
