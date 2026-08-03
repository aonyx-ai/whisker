//! A file inside the package directory that no crate's module tree reaches
//!
//! Cargo never compiles this file, because `src/lib.rs` declares no
//! `mod orphan`. rust-analyzer's VFS still interns it, because it sits
//! inside the package's source root. That combination — known to the
//! toolchain, reachable by no crate — is the case whisker used to report
//! clean while resolving nothing about it.

pub fn orphan() -> i32 {
    42
}
