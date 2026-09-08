use std::mem::offset_of;

/// One option a project set for one rule
///
/// A rule reads the values through [`RuleOptions::names`] rather than
/// reaching for an option directly, so this is what a host builds and a
/// rule rarely names. The rule it belongs to is a [`String`] and not a
/// [`RuleId`], because the name comes from a configuration file and
/// [`RuleId`] admits only a `&'static str`. A name that no loaded rule
/// declares is refused by [`RuleOptions::validate`] before any pass runs.
///
/// A value is a list of names. Every option the rules ask for today names
/// things: the attributes that mark a boundary, the crates a
/// module may import. A number and a flag have no form here, so a project
/// that writes one is told at load rather than having it read as nothing.
///
/// [`RuleId`]: crate::RuleId
/// [`RuleOptions::names`]: crate::RuleOptions::names
/// [`RuleOptions::validate`]: crate::RuleOptions::validate
///
/// # Examples
///
/// ```
/// use whisker_types::RuleOption;
///
/// let option = RuleOption::new(
///     "lint.repeated-primitive-params".to_owned(),
///     "boundary-attributes".to_owned(),
///     vec!["shard".to_owned(), "procedure".to_owned()],
/// );
///
/// assert_eq!(option.name(), "boundary-attributes");
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct RuleOption {
    rule: String,
    name: String,
    values: Vec<String>,
}

impl RuleOption {
    /// Creates the option `name` that `rule` reads, holding `values`
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleOption;
    ///
    /// let option = RuleOption::new(
    ///     "lint.bool-param".to_owned(),
    ///     "boundary-attributes".to_owned(),
    ///     vec!["shard".to_owned()],
    /// );
    ///
    /// assert_eq!(option.values(), ["shard"]);
    /// ```
    pub fn new(rule: String, name: String, values: Vec<String>) -> Self {
        Self { rule, name, values }
    }

    /// Returns the rule that reads this option
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleOption;
    ///
    /// let option = RuleOption::new(
    ///     "lint.bool-param".to_owned(),
    ///     "boundary-attributes".to_owned(),
    ///     Vec::new(),
    /// );
    ///
    /// assert_eq!(option.rule(), "lint.bool-param");
    /// ```
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Returns the name of this option
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleOption;
    ///
    /// let option = RuleOption::new(
    ///     "lint.bool-param".to_owned(),
    ///     "boundary-attributes".to_owned(),
    ///     Vec::new(),
    /// );
    ///
    /// assert_eq!(option.name(), "boundary-attributes");
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the names this option holds
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleOption;
    ///
    /// let option = RuleOption::new(
    ///     "lint.bool-param".to_owned(),
    ///     "boundary-attributes".to_owned(),
    ///     vec!["shard".to_owned()],
    /// );
    ///
    /// assert_eq!(option.values().len(), 1);
    /// ```
    pub fn values(&self) -> &[String] {
        &self.values
    }
}

/// The offsets of every field, in declaration order
///
/// The plugin handshake hashes these so a plugin that places a field
/// somewhere else is refused rather than trusted. They live beside the
/// struct, because a field added there has to be added here too.
pub(crate) const FIELD_OFFSETS: &[usize] = &[
    offset_of!(RuleOption, rule),
    offset_of!(RuleOption, name),
    offset_of!(RuleOption, values),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn option() -> RuleOption {
        RuleOption::new(
            "lint.repeated-primitive-params".to_owned(),
            "boundary-attributes".to_owned(),
            vec!["shard".to_owned(), "procedure".to_owned()],
        )
    }

    #[test]
    fn field_offsets_covers_every_field() {
        let offsets = FIELD_OFFSETS;

        assert_eq!(offsets.len(), 3);
    }

    #[test]
    fn name_returns_the_option_name() {
        let option = option();

        let name = option.name();

        assert_eq!(name, "boundary-attributes");
    }

    #[test]
    fn rule_returns_the_rule_that_reads_it() {
        let option = option();

        let rule = option.rule();

        assert_eq!(rule, "lint.repeated-primitive-params");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RuleOption>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RuleOption>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RuleOption>();
    }

    #[test]
    fn values_returns_every_name() {
        let option = option();

        let values = option.values();

        assert_eq!(values, ["shard", "procedure"]);
    }
}
