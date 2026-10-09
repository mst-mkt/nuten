import { defaultTheme, defineTheme } from "@ox-content/vite-plugin";

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
  header: {
    logo: "logo.svg",
  },
  embed: {
    head: '<link rel="icon" href="/logo.svg" type="image/svg+xml">',
  },
  socialLinks: {
    github: "https://github.com/mst-mkt/nuten",
  },
});
