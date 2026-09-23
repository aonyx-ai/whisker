# Adopt rules gradually

<!--
goal: let a project that fails many rules start using whisker today.
non-goal: deciding which rules are worth turning on. that is each rule's own
documentation.
-->

Adopting Whisker rules can be an arduous process, requiring significant changes
to your codebase. To avoid this, we have multiple ways to adopt only specific
rules. These are explained on this page.

## Enable only specific rules

You can choose to run only particular rules. This disables all other rules. To
do so, add a `[rules]` section to your configuration with an `enable` property
like so:

```toml title=".config/whisker.toml"
[rules]
enable = ["lint.doc-summary-break"]
```

## Disable rules

When a new rule comes in, or a particular rule doesn't fit your style, you can
choose to disable only that rule. To do so, add a `disable` property to your
configuration:

```toml title=".config/whisker.toml"
[rules]
disable = ["lint.no-inline-comments"]
```

## Find a rule's name

To enable and disable rules, you need to know the rule's ID. Currently, we
recommend rule authors to list the IDs in their documentation. There is no way
to list rules through the Whisker CLI yet.

Alternatively, you can look through the rule's source code for a line like
`RuleId::new("lint.doc-summary-break")`.

## Fine-grained control

Rules may define more fine-grained controls than just enable/disable. These are
called rule options, and configured in the [config file][rule-options].

[rule-options]: /docs/reference/configuration#rule-options
