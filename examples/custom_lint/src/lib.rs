//! An example whisker lint plugin
//!
//! The crate is a `cdylib` holding one rule and one `export_lints!`
//! invocation, which is everything a plugin needs. Whisker compiles this
//! package at check time, opens the library it produces, and runs the
//! lints registered here beside its own.

mod no_todo;

pub use no_todo::NoTodo;

whisker_rust::export_lints![NoTodo];
