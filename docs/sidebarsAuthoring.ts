import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

// The rule author's half of the documentation, under its own route so that a
// path never says "rule" twice. `sidebars.ts` describes the other half and
// explains the split.
//
// Tutorials has no page yet. The quick start writes a rule, but a tutorial
// that builds a type-aware rule belongs here once someone writes it.
const sidebars: SidebarsConfig = {
  authoringSidebar: [
    {
      type: "category",
      label: "How-to guides",
      collapsed: false,
      items: ["how-to/write-a-rule", "how-to/match-the-toolchain"],
    },
    {
      type: "category",
      label: "Reference",
      collapsed: true,
      items: ["reference/api"],
    },
    {
      type: "category",
      label: "Explanation",
      collapsed: true,
      items: ["explanation/plugin-boundary"],
    },
  ],
};

export default sidebars;
