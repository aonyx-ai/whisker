import { themes as prismThemes } from "prism-react-renderer";
import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

const config: Config = {
  title: "Whisker",
  tagline: "A linting platform built on tree-sitter",

  url: "https://aonyx-ai.github.io",
  baseUrl: "/whisker/",

  organizationName: "aonyx-ai",
  projectName: "whisker",

  onBrokenLinks: "throw",

  future: {
    v4: true,
  },

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  markdown: {
    format: "detect",
  },

  presets: [
    [
      "classic",
      {
        docs: {
          sidebarPath: "./sidebars.ts",
          editUrl: "https://github.com/aonyx-ai/whisker/tree/main/docs/",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: "Whisker",
      items: [
        {
          type: "docSidebar",
          sidebarId: "docsSidebar",
          position: "left",
          label: "Docs",
        },
        {
          href: "https://github.com/aonyx-ai/whisker",
          label: "GitHub",
          position: "right",
        },
      ],
    },
    footer: {
      style: "dark",
      links: [
        {
          title: "Docs",
          items: [
            {
              label: "Quick start",
              to: "/docs/quick-start",
            },
            {
              label: "Configuration",
              to: "/docs/configuration",
            },
            {
              label: "Custom lints",
              to: "/docs/custom-lints",
            },
          ],
        },
        {
          title: "Resources",
          items: [
            {
              label: "GitHub",
              href: "https://github.com/aonyx-ai/whisker",
            },
            {
              label: "Releases",
              href: "https://github.com/aonyx-ai/whisker/releases",
            },
            {
              label: "Aonyx's rules",
              href: "https://github.com/aonyx-ai/whisker-aonyx-rules",
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Aonyx AI`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ["rust", "toml"],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
