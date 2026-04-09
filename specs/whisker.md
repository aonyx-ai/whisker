# whisker

The CLI binary. Owns the command-line contract and configuration. Wires together
the pipeline (from `whisker-core`), language SDKs, and reporting (from
`whisker-reporting`).

## CLI

r[cli.check]
The `whisker check` command must run all enabled rules against the target
project by invoking the `whisker-core` pipeline.

r[cli.check.path]
The `whisker check` command must accept a path argument to specify the target
project directory.

r[cli.check.keep-going]
The `whisker check` command must accept a `--keep-going` flag that continues
checking after encountering errors rather than stopping at the first failure.

r[cli.check.extra-args]
The `whisker check` command must forward trailing arguments to the underlying
analysis pipeline.

r[cli.version]
The `whisker --version` command must print the whisker version.

## Configuration

These requirements are planned for the configuration layer, which will be
implemented after the first lint rules are ported to the tree-sitter platform.

r[cli.config.file]
The CLI must read a configuration file that maps rules to severities and
enables or disables individual rules.

r[cli.config.default-severity]
When a rule is enabled but has no configured severity, the CLI must apply a
default severity.

r[cli.config.rule-severity]
Configuration must allow mapping rules to severities (error, warn, etc.)
independently of the rule's own implementation.

r[cli.config.rule-enable]
Configuration must allow enabling and disabling individual rules.

r[cli.config.rule-independence]
The same rule must be usable with different severity levels in different
contexts (e.g. error in CI, warning in an IDE).

r[cli.diagnostics.exit-code]
The CLI must exit with a non-zero status if any diagnostic has error severity.

## whisker-reporting

Diagnostic formatting and output. Consumes severity-tagged diagnostics from the
pipeline and renders them for the target consumer.

## Output

r[reporting.output]
The reporter must render diagnostics with the primary location, message, and
severity.

r[reporting.origins]
When a diagnostic has origin locations, the reporter must display them as
supplementary annotations.

r[reporting.related]
When a diagnostic has related locations, the reporter must display them as
supplementary annotations.

r[reporting.suggestions]
When a diagnostic has suggested fixes, the reporter must display them.
