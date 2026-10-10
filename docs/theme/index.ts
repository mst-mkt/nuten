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
  codeBackground: "var(--nuten-bg-alt)",
  codeBackgroundTop: "var(--nuten-bg-alt)",
  codeText: "var(--nuten-text)",
};

const tokens = {
  "surface-glass": "var(--octc-color-bg-alt)",
  "color-code-frame-border": "var(--octc-color-border)",
  "color-code-title-bg": "var(--octc-color-bg-alt)",
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
  },
  socialLinks: {
    github: "https://github.com/mst-mkt/nuten",
  },
  footer: {
    message: "© 2026 mst-mkt. Released under the MIT License.",
  },
  css: readFileSync(new URL("style.css", import.meta.url), "utf8"),
});
