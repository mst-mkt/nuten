import { readFileSync } from "node:fs";
import { defaultTheme, defineTheme } from "@ox-content/vite-plugin";

const colors = {
  primary: "var(--nuten-primary)",
  primaryHover: "var(--nuten-primary-hover)",
  background: "var(--nuten-bg)",
  backgroundAlt: "var(--nuten-bg-alt)",
  text: "var(--nuten-text)",
  textMuted: "var(--nuten-text-muted)",
  border: "var(--nuten-border)",
  codeBackground: "var(--nuten-bg-alt-glass)",
  codeBackgroundTop: "var(--nuten-bg-alt-glass)",
  codeText: "var(--nuten-text)",
};

const tokens = {
  "color-code-frame-border": "var(--octc-color-border)",
  "color-code-title-bg": "var(--nuten-bg-alt-glass)",
  "color-code-title-text": "var(--octc-color-text-muted)",
  "color-code-title-border": "var(--octc-color-border)",
  "syntax-foreground": "var(--octc-color-code-text)",
  "syntax-background": "var(--octc-color-code-bg)",
  "syntax-token-comment": "var(--octc-color-text-muted)",
  "syntax-token-punctuation": "var(--nuten-syntax-punctuation)",
  "syntax-token-keyword": "var(--nuten-syntax-keyword)",
  "syntax-token-function": "var(--nuten-syntax-function)",
  "syntax-token-constant": "var(--nuten-syntax-constant)",
  "syntax-token-parameter": "var(--nuten-syntax-parameter)",
  "syntax-token-string": "var(--octc-color-primary)",
  "syntax-token-string-expression": "var(--octc-color-primary)",
  "syntax-token-link": "var(--octc-color-primary)",
};

const sidebar = [
  {
    text: { en: "Introduction", ja: "はじめに" },
    items: [
      { text: { en: "Overview", ja: "概要" }, link: "/introduction/overview" },
      { text: { en: "Correctness", ja: "正しさ" }, link: "/introduction/correctness" },
      { text: { en: "Performance", ja: "性能" }, link: "/introduction/performance" },
    ],
  },
  {
    text: { en: "Guide", ja: "ガイド" },
    items: [
      { text: { en: "Installation", ja: "インストール" }, link: "/guide/installation" },
      { text: { en: "Getting started", ja: "はじめる" }, link: "/guide/getting-started" },
      { text: { en: "Formatting", ja: "整形" }, link: "/guide/formatting" },
      { text: { en: "Configuration", ja: "設定" }, link: "/guide/configuration" },
      { text: { en: "Ignoring code", ja: "無効化" }, link: "/guide/ignoring-code" },
      {
        text: { en: "Integrations", ja: "連携" },
        link: "/guide/integrations",
        collapsed: true,
        items: [
          { text: "VS Code", link: "/guide/integrations/vscode" },
          { text: "Zed", link: "/guide/integrations/zed" },
          { text: "Vim", link: "/guide/integrations/vim" },
          { text: "Neovim", link: "/guide/integrations/neovim" },
          { text: "Helix", link: "/guide/integrations/helix" },
          { text: "Emacs", link: "/guide/integrations/emacs" },
          { text: "dprint", link: "/guide/integrations/dprint" },
          { text: "treefmt", link: "/guide/integrations/treefmt" },
          { text: "CI", link: "/guide/integrations/ci" },
        ],
      },
    ],
  },
  {
    text: { en: "Reference", ja: "リファレンス" },
    items: [
      { text: "CLI", link: "/reference/cli" },
      { text: { en: "Settings", ja: "設定項目" }, link: "/reference/settings" },
      { text: { en: "Style", ja: "スタイル" }, link: "/reference/style" },
    ],
  },
];

export const theme = defineTheme({
  extends: defaultTheme,
  fonts: {
    sans: {
      family: "Onest",
      path: "@fontsource-variable/onest/files/onest-latin-wght-normal.woff2",
      weights: [400, 500, 600],
      selfHost: true,
      preload: [400],
    },
  },
  colors,
  darkColors: colors,
  tokens,
  darkTokens: tokens,
  header: {
    logo: "logo.svg",
  },
  embed: {
    head: '<link rel="icon" href="/logo.svg" type="image/svg+xml">',
    footer: `
      <footer class="site-footer">
        <div class="footer-row">
          <span class="footer-brand"><img src="/logo.svg" alt="">Nuten</span>
          <nav class="footer-links">
            <a href="https://github.com/mst-mkt/nuten">Repository</a>
            <a href="https://github.com/mst-mkt/nuten/releases">Releases</a>
          </nav>
        </div>
        <p class="footer-copyright">© 2026 <a href="https://github.com/mst-mkt">mst-mkt</a> · MIT License</p>
      </footer>
    `,
  },
  socialLinks: {
    github: "https://github.com/mst-mkt/nuten",
  },
  sidebar,
  aside: true,
  css: readFileSync(new URL("style.css", import.meta.url), "utf8"),
  js: `
    const lang = document.documentElement.lang;
    if (lang !== "en") document.querySelector(".header-title").href = "/" + lang + "/index.html";
  `,
});
