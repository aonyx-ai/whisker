use std::collections::BTreeSet;
use std::mem::offset_of;

use crate::RuleId;

mod rule_option;

pub use rule_option::RuleOption;

/// Every option a project set, for every rule it runs
///
/// A rule cannot always decide a case from the source alone. Whether an
/// attribute marks a boundary depends on which framework wrote
/// the attribute, and no rule knows every framework. The project does, so
/// the project says, and the rule reads what it said.
///
/// The whole table crosses the plugin boundary, not the part one pass
/// would read. Whisker knows which rules a plugin declares but not which
/// pass reports which, so it cannot cut the table down before it hands it
/// over. A pass therefore names its own rule in the lookup, which it
/// already holds as a constant.
///
/// A table that names a rule no loaded plugin declares is refused by
/// [`RuleOptions::validate`]. An option name is not checked, because
/// nothing declares the options a rule reads; a misspelled option name
/// reads as an absent one.
///
/// # Examples
///
/// ```
/// use whisker_types::{RuleId, RuleOption, RuleOptions};
///
/// let options = RuleOptions::new(vec![RuleOption::new(
///     "lint.bool-param".to_owned(),
///     "boundary-attributes".to_owned(),
///     vec!["shard".to_owned()],
/// )]);
///
/// let names = options.names(RuleId::new("lint.bool-param"), "boundary-attributes");
///
/// assert_eq!(names, Some(&["shard".to_owned()][..]));
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct RuleOptions {
    options: Vec<RuleOption>,
}

impl RuleOptions {
    /// Creates the table `options` describes
    ///
    /// Each pair of a rule and an option name must appear once. A
    /// configuration file cannot repeat a key, so the host that reads one
    /// cannot build a table that does; [`RuleOptions::names`] returns the
    /// first match for a caller that builds one by hand.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleOptions;
    ///
    /// let options = RuleOptions::new(Vec::new());
    ///
    /// assert!(options.options().is_empty());
    /// ```
    pub fn new(options: Vec<RuleOption>) -> Self {
        Self { options }
    }

    /// Returns the names `rule` was given for the option `option`
    ///
    /// Returns [`None`] when the project set no such option, which a rule
    /// reads as "use the default". An option set to an empty list is
    /// [`Some`] holding nothing, which a rule reads as "allow none". The
    /// two differ, so a rule with a non-empty default can tell an empty
    /// list from silence.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::{RuleId, RuleOptions};
    ///
    /// let options = RuleOptions::new(Vec::new());
    ///
    /// assert_eq!(options.names(RuleId::new("lint.bool-param"), "unset"), None);
    /// ```
    pub fn names(&self, rule: RuleId, option: &str) -> Option<&[String]> {
        self.options
            .iter()
            .find(|candidate| candidate.rule() == rule.as_str() && candidate.name() == option)
            .map(RuleOption::values)
    }

    /// Returns every option in the table
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::RuleOptions;
    ///
    /// let options = RuleOptions::new(Vec::new());
    ///
    /// assert_eq!(options.options().len(), 0);
    /// ```
    pub fn options(&self) -> &[RuleOption] {
        &self.options
    }

    /// Refuses a table that configures a rule no loaded plugin declares
    ///
    /// # Errors
    ///
    /// Returns an error naming every such rule. An option set on a
    /// misspelled rule configures nothing and reads exactly like a rule
    /// that took the option and found no fault, so whisker refuses the
    /// file rather than the reader having to notice.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    ///
    /// use whisker_types::RuleOptions;
    ///
    /// let options = RuleOptions::new(Vec::new());
    ///
    /// assert!(options.validate(&BTreeSet::new()).is_ok());
    /// ```
    pub fn validate(&self, declared: &BTreeSet<String>) -> anyhow::Result<()> {
        let unknown: BTreeSet<&str> = self
            .options
            .iter()
            .map(RuleOption::rule)
            .filter(|rule| !declared.contains(*rule))
            .collect();

        anyhow::ensure!(
            unknown.is_empty(),
            "[rules.options] configures {}, which no configured lint reports; the lints loaded \
             here report {}",
            unknown.into_iter().collect::<Vec<_>>().join(", "),
            match declared.is_empty() {
                true => "nothing".to_owned(),
                false => declared
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
            }
        );

        Ok(())
    }
}

/// The offsets of every field, in declaration order
///
/// The plugin handshake hashes these so a plugin that places a field
/// somewhere else is refused rather than trusted. They live beside the
/// struct, because a field added there has to be added here too.
pub(crate) const FIELD_OFFSETS: &[usize] = &[offset_of!(RuleOptions, options)];

/// The offsets of every field of [`RuleOption`], in declaration order
///
/// [`RuleOptions`] holds them behind a [`Vec`], so the handshake has to
/// hash the element's layout as well as the table's own.
pub(crate) const OPTION_FIELD_OFFSETS: &[usize] = rule_option::FIELD_OFFSETS;

#[cfg(test)]
mod tests {
    use super::*;

    fn declared(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn options() -> RuleOptions {
        RuleOptions::new(vec![
            RuleOption::new(
                "lint.repeated-primitive-params".to_owned(),
                "boundary-attributes".to_owned(),
                vec!["shard".to_owned(), "procedure".to_owned()],
            ),
            RuleOption::new(
                "lint.bool-param".to_owned(),
                "boundary-attributes".to_owned(),
                Vec::new(),
            ),
        ])
    }

    #[test]
    fn field_offsets_covers_every_field() {
        let offsets = FIELD_OFFSETS;

        assert_eq!(offsets.len(), 1);
    }

    #[test]
    fn names_with_an_empty_list_differs_from_an_absent_option() {
        let options = options();

        let empty = options.names(RuleId::new("lint.bool-param"), "boundary-attributes");

        assert_eq!(empty, Some(&[][..]));
        assert_eq!(
            options.names(RuleId::new("lint.bool-param"), "absent"),
            None
        );
    }

    #[test]
    fn names_with_a_matching_rule_and_option_returns_the_values() {
        let options = options();

        let names = options.names(
            RuleId::new("lint.repeated-primitive-params"),
            "boundary-attributes",
        );

        assert_eq!(
            names,
            Some(&["shard".to_owned(), "procedure".to_owned()][..])
        );
    }

    #[test]
    fn names_with_another_rules_option_returns_none() {
        let options = options();

        let names = options.names(RuleId::new("lint.derive-order"), "boundary-attributes");

        assert_eq!(names, None);
    }

    #[test]
    fn option_field_offsets_covers_every_field() {
        let offsets = OPTION_FIELD_OFFSETS;

        assert_eq!(offsets.len(), 3);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RuleOptions>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RuleOptions>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RuleOptions>();
    }

    #[test]
    fn validate_names_every_unknown_rule_once() {
        let options = options();

        let error = options
            .validate(&declared(&["lint.derive-order"]))
            .expect_err("an unknown rule should be refused");

        let message = format!("{error:#}");
        assert!(message.contains("lint.bool-param"), "{message}");
        assert!(
            message.contains("lint.repeated-primitive-params"),
            "{message}"
        );
        assert!(message.contains("lint.derive-order"), "{message}");
    }

    #[test]
    fn validate_with_every_rule_declared_succeeds() {
        let options = options();

        let result = options.validate(&declared(&[
            "lint.bool-param",
            "lint.repeated-primitive-params",
        ]));

        assert!(result.is_ok());
    }

    #[test]
    fn validate_without_any_declared_rule_says_so() {
        let options = options();

        let error = options
            .validate(&BTreeSet::new())
            .expect_err("an unknown rule should be refused");

        assert!(format!("{error:#}").contains("report nothing"), "{error:#}");
    }
}
