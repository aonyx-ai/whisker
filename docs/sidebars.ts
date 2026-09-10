import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

// Whisker has two audiences. A project configures and runs Whisker; a rule
// author writes the lints it runs. This is the first of them. The second
// lives under `authoring/`, in its own plugin instance, and `sidebarsAuthoring`
// describes it. Both are split the same way: tutorials to learn from, how-to
// guides to follow, reference to look things up in, and explanation to
// understand the design.
//
// The directories under `docs/` carry that split, so a page's path says which
// of the four it is, and a page that turns out to be two pages moves by being
// split rather than by being relabeled.
//
// Only the first category of a sidebar starts open. Docusaurus expands
// whichever category holds the current page, so the others open when the
// reader reaches them.
const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: "category",
      label: "Tutorials",
      collapsed: false,
      items: ["tutorials/quick-start"],
    },
    {
      type: "category",
      label: "How-to guides",
      collapsed: true,
      items: [
        "how-to/install",
        "how-to/github-actions",
        "how-to/pin-shared-rules",
        "how-to/adopt-rules-gradually",
      ],
    },
    {
      type: "category",
      label: "Reference",
      collapsed: true,
      items: [
        "reference/configuration",
        "reference/command-line",
        "reference/runs-and-outcomes",
        "reference/file-discovery",
        "reference/prebuilt-archives",
        "reference/environment-variables",
        "reference/cache-layout",
        "reference/platforms",
      ],
    },
    {
      type: "category",
      label: "Explanation",
      collapsed: true,
      items: [
        "explanation/how-whisker-works",
        "explanation/coverage",
        "explanation/pinning-and-trust",
      ],
    },
  ],
};

export default sidebars;
