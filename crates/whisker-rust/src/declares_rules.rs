use whisker_types::RuleId;

/// Declares the rules a lint pass can report
///
/// Whisker refuses a configured rule name that no loaded plugin declares,
/// and this is where a plugin says which names are its own.
///
/// The trait never crosses the plugin boundary. [`export_lints!`] calls it
/// inside the plugin's own compilation and puts the result behind a
/// function pointer in the declaration, so nothing here depends on the
/// layout of a vtable. That is what lets whisker read the rules of a
/// plugin built against a newer protocol than an older one without
/// refusing either.
///
/// [`export_lints!`]: crate::export_lints
///
/// # Examples
///
/// ```ignore
/// impl DeclaresRules for NoTodo {
///     fn rules(&self) -> Vec<RuleId> {
///         vec![RuleId::new("custom.no-todo")]
///     }
/// }
/// ```
pub trait DeclaresRules {
    /// Returns every rule this pass can report
    fn rules(&self) -> Vec<RuleId>;
}
