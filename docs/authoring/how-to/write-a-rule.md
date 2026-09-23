# Write a rule

<!--
goal: take a reader from nothing to their own rule running in their project.
non-goal: explaining the boundary or the pipeline. both have their own pages.
-->

## start from an example

<!--
where do I begin. doc_summary_break, or custom_lint for the same shape with more
comments.
-->

## what a crate holds

<!--
what does the manifest look like. a cdylib plus the two whisker deps at a tag,
and why default-features = false.
how do I match the whisker I run. pin both crates to the release you installed,
and the compiler is not compared.
-->

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

## hook a node kind

<!--
how does my code get called. one method per node kind, returning diagnostics,
every other kind defaulting to nothing.
what bites me next. passes hold state so each file gets a fresh one, and a
supertype beats hooking every kind under it.
-->

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

## declare and export the rule

<!--
what makes a rule nameable in a config. a RuleId, DeclaresRules, and
export_lints! taking a list.
-->

```rust
const RULE_ID: RuleId = RuleId::new("lint.doc-summary-break");

impl whisker_rust::DeclaresRules for DocSummaryBreak {
    fn rules(&self) -> Vec<RuleId> {
        vec![RULE_ID]
    }
}

whisker_rust::export_lints![DocSummaryBreak];
```

## test it

<!--
how do I know it works. whisker-testing parse, execute and assert_diagnostic,
with no project and no toolchain.
how do I test a type-aware rule. build the decorations yourself rather than
running a toolchain.
-->

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

## run it

<!--
how do I use it for real. a [[lints]] path entry and whisker check.
what if it will not load. match the pin, and the boundary page is the reason.
-->

```toml title=".config/whisker.toml"
[[lints]]
path = "lints/doc_summary_break"
```

```bash
whisker check .
```

## give the rule options

<!--
how does a rule learn what it cannot read off the source. a rules.options table,
read in configure.
why does None versus empty matter. a rule with a default worth keeping can tell
them apart.
-->

```toml title=".config/whisker.toml"
[rules.options."lint.repeated-primitive-params"]
boundary-attributes = ["shard", "procedure"]
```

```rust
impl RustLintPass for RepeatedPrimitiveParams {
    fn configure(&mut self, options: &RuleOptions) {
        self.boundary_attributes = options
            .names(RULE_ID, "boundary-attributes")
            .unwrap_or_default();
    }
}
```

## next

<!--
where now. api, configuration, shared-rules, prebuilt archives.
-->
