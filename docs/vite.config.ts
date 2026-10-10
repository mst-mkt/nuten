import { getWorkerAssetsDir } from "@cloudflare/build-output-utils";
import { cloudflare } from "@cloudflare/vite-plugin";
import { oxContent } from "@ox-content/vite-plugin";
import { defineConfig } from "vite-plus";
import { theme } from "./theme";
import { ansi } from "./theme/ansi";

export default defineConfig({
  plugins: [
    cloudflare(),
    oxContent({
      srcDir: "content",
      outDir: getWorkerAssetsDir(import.meta.dirname),
      highlight: true,
      docs: false,
      codeAnnotations: {
        notation: "vitepress",
      },
      codeGroups: true,
      cards: true,
      icons: {
        include: ["content/**/*.md"],
      },
      containers: {
        types: {
          compare: {},
          tools: {},
        },
      },
      transformers: [ansi],
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
        theme,
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
        input: [{ auto: true }, "!.cloudflare/**"],
      },
      deploy: {
        command: "cf deploy",
        cache: false,
      },
    },
  },
});
