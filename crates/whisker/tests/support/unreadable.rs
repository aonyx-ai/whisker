use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

/// A guard that restores a directory's permissions on drop
///
/// A failed test would otherwise leave a mode `000` directory behind, which
/// `TempDir` cannot remove. The unit tests in `src/discovery.rs` carry the
/// same guard, because an integration test cannot reach into the binary it
/// drives.
pub struct Unreadable<'a> {
    directory: &'a Path,
}

impl Drop for Unreadable<'_> {
    fn drop(&mut self) {
        make_readable(self.directory);
    }
}

/// Restores default permissions so `TempDir` can remove the directory
fn make_readable(directory: &Path) {
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755))
        .expect("permissions should be restored");
}

/// Makes `directory` unreadable until the returned guard is dropped
///
/// Root, common in CI containers, can read a mode `000` directory anyway.
/// The CLI tests would then exercise nothing, so this fails loudly.
///
/// # Panics
///
/// Panics if the mode cannot be cleared, or if `directory` is still readable
/// afterwards.
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
