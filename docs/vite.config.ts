import { defaultTheme, defineTheme, oxContent } from "@ox-content/vite-plugin";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [
    oxContent({
      srcDir: "content",
      highlight: true,
      docs: false,
      i18n: {
        enabled: true,
        defaultLocale: "en",
        locales: [
          { code: "en", name: "English" },
          { code: "ja", name: "日本語" },
        ],
        check: false,
      },
      ssg: {
        siteName: "Nuten",
        siteUrl: "https://nuten.keito.dev",
        localeSwitcher: true,
        theme: defineTheme({
          extends: defaultTheme,
          header: {
            logo: "logo.svg",
          },
          embed: {
            head: '<link rel="icon" href="/logo.svg" type="image/svg+xml">',
          },
          socialLinks: {
            github: "https://github.com/mst-mkt/nuten",
          },
        }),
      },
    }),
  ],
  run: {
    tasks: {
      dev: {
        command: "vp dev",
        cache: false,
      },
      build: {
        command: "vp build",
      },
    },
  },
});
