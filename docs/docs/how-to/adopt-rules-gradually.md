# Adopt rules one at a time

A `[[lints]]` entry brings every rule its source provides. A project that is
not ready for all of them names the ones it is ready for:

```toml title=".config/whisker.toml"
[rules]
enable = ["lint.doc-summary-break"]
```

Only those rules run. Add a name each time you fix what the last one found.

## The other direction

A project that wants the whole set minus a few names those few instead:

```toml title=".config/whisker.toml"
[rules]
disable = ["lint.no-inline-comments"]
```

Naming both is refused. A project that says which rules run has already said
which do not.

## Finding the names

A rule's id comes from the rule's own documentation.

A name that no configured lint reports is an error, and the error lists the
names the configured lints do report. A misspelled name would otherwise
disable nothing and read exactly like a rule that found no fault.

## Turning a rule down rather than off

Some rules take options, which is often the better answer to a rule that fires
too often. See [configuration][configuration].

[configuration]: /docs/reference/configuration#rulesoptionsrule
