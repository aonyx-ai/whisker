# Write a rule

A rule is a Rust crate that Whisker compiles and loads.

Start from [`examples/doc_summary_break`][example], or from
[`examples/custom_lint`][template], which is the same shape with more comments.
Copy either into `lints/` in your project.

## What a crate holds

A lint crate is a `cdylib` that depends on whisker-rust and whisker-types:

```toml title="lints/doc_summary_break/Cargo.toml"
[package]
name = "doc_summary_break"
version = "0.1.0"
edition = "2024"

[dependencies]
whisker-rust = { git = "https://github.com/aonyx-ai/whisker.git", tag = "v0.1.0-rc.4", default-features = false }
whisker-types = { git = "https://github.com/aonyx-ai/whisker.git", tag = "v0.1.0-rc.4" }

[lib]
crate-type = ["cdylib"]
```

`default-features = false` on whisker-rust leaves out rust-analyzer, which a
rule does not need. The tag has to match the Whisker you run; see
[matching the toolchain][toolchain].

## Hook a node kind

`RustLintPass` has one method per named node kind in the Rust grammar. Take the
kind you care about and return the diagnostics you found. Every other kind
keeps its default, which returns nothing:

```rust
impl RustLintPass for DocSummaryBreak {
    fn check_line_comment(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        if !is_doc_line(node) {
            return Vec::new();
        }

        vec![Diagnostic::new(
            RULE_ID,
            Severity::Warn,
            "the summary line needs a blank `///` line after it".into(),
            node.span(),
        )]
    }
}
```

Whisker builds a fresh pass for each file, because passes hold state.

Grammars group kinds into supertypes, so a rule that cares about every
expression hooks the supertype rather than every kind under it.

## Declare and export the rule

The id is what a project names in `[rules]`, and what a diagnostic carries.
Declaring it is what lets Whisker refuse a misspelled name in a configuration:

```rust
const RULE_ID: RuleId = RuleId::new("lint.doc-summary-break");

impl whisker_rust::DeclaresRules for DocSummaryBreak {
    fn rules(&self) -> Vec<RuleId> {
        vec![RULE_ID]
    }
}

whisker_rust::export_lints![DocSummaryBreak];
```

`export_lints!` takes a list, so one crate can carry several rules.

## Test it

whisker-testing runs a rule against source text, with no project and no
toolchain around it:

```rust
use whisker_rust::RustLintPassAdapter;
use whisker_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
use whisker_types::{Language, LintPass};

fn passes() -> Vec<Box<dyn LintPass>> {
    vec![Box::new(RustLintPassAdapter::new(DocSummaryBreak))]
}

#[test]
fn a_second_line_against_the_summary_is_flagged() {
    let tree = parse("/// Adds\n/// The rest.\nfn f() {}", Language::Rust);

    let diagnostics = execute(&tree, &mut passes());

    assert_eq!(diagnostics.len(), 1);
    assert_diagnostic(&diagnostics[0]).has_rule_id("lint.doc-summary-break");
}
```

`assert_diagnostic` also checks severity, message, span, and the counts of
origins, related spans, and suggestions. `fixtures` reads a directory of source
files when a rule needs more than a string.

A rule that reads type information takes its decorations from `decorate`, so a
test builds those facts itself rather than running a toolchain. See
[how Whisker works][how-it-works].

## Run it

Name the directory in your configuration, and check the project:

```toml title=".config/whisker.toml"
[[lints]]
path = "lints/doc_summary_break"
```

```bash
whisker check .
```

Whisker compiles the crate with your cargo, loads the library, and runs the
rule. The first build takes as long as any Rust build.

A library built by a different compiler than the Whisker running it is refused:
[matching the toolchain][toolchain] is the fix, and
[the plugin boundary][plugin-boundary] is the reason.

## Give the rule options

A rule cannot always decide a case from the source alone. Whether an attribute
marks a system boundary depends on which framework wrote the attribute, and no
rule knows every framework. A project names what the rule cannot know:

```toml title=".config/whisker.toml"
[rules.options."lint.repeated-primitive-params"]
boundary-attributes = ["shard", "procedure"]
```

The rule reads it in `configure`, which Whisker calls once on each pass before
that pass sees a node:

```rust
impl RustLintPass for RepeatedPrimitiveParams {
    fn configure(&mut self, options: &RuleOptions) {
        self.boundary_attributes = options
            .names(RULE_ID, "boundary-attributes")
            .unwrap_or_default()
            .to_vec();
    }
}
```

`names` returns `None` when the project set no such option, and `Some` holding
nothing when it set an empty list. A rule with a default worth keeping can tell
the two apart. Which options a rule reads is the rule's own documentation:
Whisker never checks an option name.

## Next

- [API documentation][api]: the crates a rule is written against.
- [Configuration][configuration]: every key of `.config/whisker.toml`.
- [Pin a shared set of rules][pinning-rules]: move the crate into its own
  repository.
- [Publish prebuilt archives][prebuilt-lints]: spare every consumer the build.

[api]: /authoring/reference/api
[configuration]: /docs/reference/configuration
[example]: https://github.com/aonyx-ai/whisker/tree/main/examples/doc_summary_break
[how-it-works]: /docs/explanation/how-whisker-works
[pinning-rules]: /docs/how-to/pin-shared-rules
[plugin-boundary]: /authoring/explanation/plugin-boundary
[prebuilt-lints]: /docs/reference/prebuilt-archives
[template]: https://github.com/aonyx-ai/whisker/tree/main/examples/custom_lint
[toolchain]: /authoring/how-to/match-the-toolchain
