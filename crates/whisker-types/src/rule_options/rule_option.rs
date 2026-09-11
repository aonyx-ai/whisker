use std::hash::{Hash, Hasher};

use stabby::{string, vec};

/// One option a project set for one rule
///
/// A rule reads the values through [`RuleOptions::names`] rather than
/// reaching for an option directly, so this is what a host builds and a
/// rule rarely names. The rule it belongs to is an owned string, because
/// the name comes from a configuration file and [`RuleId`] admits only a
/// `&'static str`. A name that no loaded rule
/// declares is refused by [`RuleOptions::validate`] before any pass runs.
///
/// A value is a list of names. Every option the rules ask for today names
/// things: the attributes that mark a boundary, the crates a
/// module may import. A number and a flag have no form here, so a project
/// that writes one is told at load rather than having it read as nothing.
///
/// The option crosses the plugin boundary inside [`RuleOptions`]. Stabby
/// lays it out, and its strings and list are stabby's `String` and `Vec`.
/// The constructor copies what it is given, and the accessors hand
/// out borrowed `str`s, so nothing outside this crate names those types.
///
/// [`RuleId`]: crate::RuleId
/// [`RuleOptions`]: crate::RuleOptions
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
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct RuleOption {
    rule: string::String,
    name: string::String,
    values: vec::Vec<string::String>,
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
    /// assert_eq!(option.values().collect::<Vec<_>>(), ["shard"]);
    /// ```
    pub fn new(rule: String, name: String, values: Vec<String>) -> Self {
        Self {
            rule: string::String::from(rule.as_str()),
            name: string::String::from(name.as_str()),
            values: values
                .iter()
                .map(|value| string::String::from(value.as_str()))
                .collect(),
        }
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

    /// Returns the names this option holds, in the order they were written
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
    /// assert_eq!(option.values().count(), 1);
    /// ```
    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.values.iter().map(string::String::as_str)
    }
}

impl Hash for RuleOption {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rule.hash(state);
        self.name.hash(state);
        self.values.as_slice().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn option() -> RuleOption {
        RuleOption::new(
            "lint.repeated-primitive-params".to_owned(),
            "boundary-attributes".to_owned(),
            vec!["shard".to_owned(), "procedure".to_owned()],
        )
    }

    #[test]
    fn hash_agrees_with_eq() {
        let mut options = HashSet::new();
        options.insert(option());

        let found = options.contains(&option());

        assert!(found);
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

        let values: Vec<&str> = option.values().collect();

        assert_eq!(values, ["shard", "procedure"]);
    }
}
