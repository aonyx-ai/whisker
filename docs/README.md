# Documentation

The public website and documentation for Whisker is built using [Docusaurus].

The necessary tooling is included in the [Flox] environment for this project.
Run `flox activate` to enter the environment, and then use `just` to run the
development server:

```sh
# From the project root
just docs dev

# Or, from the docs/ directory
just dev
```

The site is built on every pull request. It is deployed to GitHub Pages when a
release is published.

[docusaurus]: https://docusaurus.io/
[flox]: https://flox.dev/
