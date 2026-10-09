import { oxContent } from "@ox-content/vite-plugin";
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
        localeSwitcher: true,
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
