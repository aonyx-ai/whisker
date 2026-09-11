use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use stabby::vec;

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
/// already holds as a constant. Because the table crosses, stabby lays it
/// out and its list is stabby's rather than std's.
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
/// assert_eq!(names, Some(vec!["shard".to_owned()]));
/// ```
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Default)]
pub struct RuleOptions {
    options: vec::Vec<RuleOption>,
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
        Self {
            options: options.into_iter().collect(),
        }
    }

    /// Returns the names `rule` was given for the option `option`
    ///
    /// Returns [`None`] when the project set no such option, which a rule
    /// reads as "use the default". An option set to an empty list is
    /// [`Some`] holding nothing, which a rule reads as "allow none". The
    /// two differ, so a rule with a non-empty default can tell an empty
    /// list from silence.
    ///
    /// The names come back owned. The table keeps them in stabby's
    /// `String`, and a rule keeps what it reads for the whole file. A copy
    /// at this one call therefore costs less than a type every rule would
    /// have to name.
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
    pub fn names(&self, rule: RuleId, option: &str) -> Option<Vec<String>> {
        self.options
            .iter()
            .find(|candidate| candidate.rule() == rule.as_str() && candidate.name() == option)
            .map(|candidate| candidate.values().map(str::to_owned).collect())
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

impl Hash for RuleOptions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.options.as_slice().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

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
    fn hash_agrees_with_eq() {
        let mut tables = HashSet::new();
        tables.insert(options());

        let found = tables.contains(&options());

        assert!(found);
    }

    #[test]
    fn names_with_an_empty_list_differs_from_an_absent_option() {
        let options = options();

        let empty = options.names(RuleId::new("lint.bool-param"), "boundary-attributes");

        assert_eq!(empty, Some(Vec::new()));
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
            Some(vec!["shard".to_owned(), "procedure".to_owned()])
        );
    }

    #[test]
    fn names_with_another_rules_option_returns_none() {
        let options = options();

        let names = options.names(RuleId::new("lint.derive-order"), "boundary-attributes");

        assert_eq!(names, None);
    }

    #[test]
    fn options_returns_every_option_in_order() {
        let options = options();

        let rules: Vec<&str> = options.options().iter().map(RuleOption::rule).collect();

        assert_eq!(rules, ["lint.repeated-primitive-params", "lint.bool-param"]);
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
