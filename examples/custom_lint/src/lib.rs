//! An example whisker lint plugin
//!
//! The crate is a `cdylib` holding two rules and one `export_lints!`
//! invocation, which is everything a plugin needs. Whisker compiles this
//! package at check time, opens the library it produces, and runs the
//! lints registered here beside its own.
//!
//! The two rules are here to show both halves of what a lint can read.
//! [`NoTodo`] works from the parsed tree alone. [`AnyhowError`] reads a
//! decoration the host resolved, which is the part a plugin cannot
//! compute for itself, and the part that only works because the plugin
//! and the whisker running it agree on how those types are laid out.

mod anyhow_error;
mod no_todo;

pub use anyhow_error::AnyhowError;
pub use no_todo::NoTodo;

whisker_rust::export_lints![NoTodo, AnyhowError];
