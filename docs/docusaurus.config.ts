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

  // The rule author's documentation is a second docs instance rather than a
  // directory inside the first. It carries its own route, so a page's path
  // never repeats the section it sits in.
  plugins: [
    [
      "@docusaurus/plugin-content-docs",
      {
        id: "authoring",
        path: "authoring",
        routeBasePath: "authoring",
        sidebarPath: "./sidebarsAuthoring.ts",
        editUrl: "https://github.com/aonyx-ai/whisker/tree/main/docs/",
      },
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
          type: "docSidebar",
          sidebarId: "authoringSidebar",
          docsPluginId: "authoring",
          position: "left",
          label: "Writing rules",
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
              to: "/docs/tutorials/quick-start",
            },
            {
              label: "Configuration",
              to: "/docs/reference/configuration",
            },
            {
              label: "Write a rule",
              to: "/authoring/how-to/write-a-rule",
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
