# API documentation

The crates a lint crate depends on document themselves.

| Crate                              | Holds                                                                                               |
| ---------------------------------- | --------------------------------------------------------------------------------------------------- |
| [whisker-rust][whisker-rust]       | `RustLintPass`, one method per node kind, plus `DeclaresRules` and `export_lints!`                  |
| [whisker-types][whisker-types]     | `Diagnostic`, `Span`, `Suggestion`, `Severity`, `RuleId`, `DecoratedNode`, and the decoration types |
| [whisker-testing][whisker-testing] | `parse`, `decorate`, `execute`, `fixtures`, and `assert_diagnostic`                                 |

Pin all three to the same tag as the Whisker you run. See
[write a rule][write-a-rule].

[write-a-rule]: /authoring/how-to/write-a-rule
[whisker-rust]: https://docs.rs/whisker-rust
[whisker-testing]: https://docs.rs/whisker-testing
[whisker-types]: https://docs.rs/whisker-types
