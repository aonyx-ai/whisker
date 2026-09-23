# Shared rules

Whisker allows sharing rules between different projects through shared rules: a
single Git repository that Whisker can pull rules from. These repositories can
[publish precompiled archives][prebuilt-archives] that Whisker can use, or if
that's not available Whisker will build the rules itself.

## Adding shared rules

To use rules published to a separate repository, you can provide a Git repo
link. This requires [a specific commit hash](#find-hash). Whisker does not work
with branches or other refs, to avoid the footgun of having a moving linting
target. We recommend [automating the updates](#move-pin) though. Configure
the lints like so:

```toml title=".config/whisker.toml"
[[lints]]
git = "https://github.com/organisation/whisker-organisation-rules"
rev = "0123456789abcdef0123456789abcdef01234567"
```

## Finding the commit hash {#find-hash}

You need the full 40 character hash. There's a few ways of getting it, easiest
from the command line is `git ls-remote`:

```bash
git ls-remote https://github.com/organisation/whisker-organisation-rules HEAD
```

Alternatively you can find it in the Github UI. There is a "Copy full SHA"
button on the commit history page, as well as on the page for a single commit.
You can also get the commit SHA for the latest stable release using this `gh`
oneliner:

```bash
REPO=organisation/whisker-organisation-rules \
    gh api \
        "repos/$REPO/commits/$(gh api "repos/$REPO/releases/latest" --jq .tag_name)" \
        --jq .sha
```

## Updating the commit hash {#move-pin}

Be sure to update the commit hash of your rules regularly! Find the new hash
using [the method above](#find-hash), or even better: have a bot do it for you.

:::warning

When automatically updating your rules, new or updated versions may break on the
same codebase. This might mean that a rules update PR can become very large with
a flood of fixes.

Considering [adopting rules gradually][adopt-rules-gradually] to avoid this.

:::

We recommend setting up Renovate to track Github Releases for your rules
repository. To do that, modify the your Whisker config to add a version comment
that Renovate can read:

```toml title=".config/whisker.toml"
[[lints]]
git = "https://github.com/organisation/whisker-organisation-rules"
rev = "0123456789abcdef0123456789abcdef01234567" # v0.1.0
```

And then [set up Renovate in your repository][set-up-renovate] and configure it
to read that comment and track it:

```json5 title=".github/renovate.json5"
{
  $schema: "https://docs.renovatebot.com/renovate-schema.json",
  customManagers: [
    {
      customType: "regex",
      fileMatch: ["^\\.config/whisker\\.toml$"],
      matchStrings: [
        'rev = "(?<currentDigest>[0-9a-f]{40})" # (?<currentValue>v\\S+)',
      ],
      depNameTemplate: "organisation/whisker-organisation-rules",
      datasourceTemplate: "github-releases",
    },
  ],
  packageRules: [
    {
      matchDepNames: ["organisation/whisker-organisation-rules"],
      ignoreUnstable: false,
    },
  ],
}
```

[set-up-renovate]: https://github.com/apps/renovate
[prebuilt-archives]: /authoring/reference/prebuilt-archives
[adopt-rules-gradually]: /docs/how-to/adopt-rules-gradually
