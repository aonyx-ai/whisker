# API

<!--
goal: send a rule author to the right crate's own documentation.
non-goal: duplicating any of it here.
-->

## the table

<!--
which crate holds what. three rows, whisker-rust, whisker-types and
whisker-testing, each naming its main items.
which version should I read. the same tag as the whisker you run.
-->

| Crate                              | Holds                                                                                               |
| ---------------------------------- | --------------------------------------------------------------------------------------------------- |
| [whisker-rust][whisker-rust]       | `RustLintPass`, one method per node kind, plus `DeclaresRules` and `export_lints!`                  |
| [whisker-types][whisker-types]     | `Diagnostic`, `Span`, `Suggestion`, `Severity`, `RuleId`, `DecoratedNode`, and the decoration types |
| [whisker-testing][whisker-testing] | `parse`, `decorate`, `execute`, `fixtures`, and `assert_diagnostic`                                 |

<!--
every link points at docs.rs and every crate sets publish = false, so nothing
resolves today. decide whether a page that cannot work yet ships or waits.
-->

[whisker-rust]: https://docs.rs/whisker-rust
[whisker-testing]: https://docs.rs/whisker-testing
[whisker-types]: https://docs.rs/whisker-types
