# GitHub Actions

<!--
goal: get a whisker check running in a workflow without paying a build on every
run.
non-goal: teaching Actions. every snippet assumes the reader writes workflows.
-->

This page explains how to get Whisker running automatically in Github Actions.

## Install step

We publish a Github Action to install Whisker directly from the Whisker
repository:

```yaml
- uses: aonyx-ai/whisker@v0.1.0-rc.4
  with:
    version: v0.1.0-rc.4
- run: whisker check .
```

## Caching rules

Currently, the action does not cache anything. When you are not using [shared
rules][shared-rules], the action will rebuild rules every time. This will be
very slow.

## Private rules

If you're using [shared rules][shared-rules] from a private repository, your
action will not be able to access them by default. You'll need to configure a
Github token and add it as a secret to a job.

The default token that is configured with Github (`GITHUB_TOKEN`) is only able
to grant access to the active repository. [Github
recommends][gh-additional-permissions] using a Github App or a personal access
token. For a PAT, add it to your Github Secrets and pass it to Whisker like so:

```yaml
- run: whisker check .
  env:
    GH_TOKEN: ${{ secrets.RULES_TOKEN }}
```

[shared-rules]: /docs/how-to/shared-rules
[gh-additional-permissions]: https://docs.github.com/en/actions/tutorials/authenticate-with-github_token#granting-additional-permissions
