import { join } from "node:path";
import { getWorkerAssetsDir } from "@cloudflare/build-output-utils";
import { cloudflare } from "@cloudflare/vite-plugin";
import { oxContent } from "@ox-content/vite-plugin";
import { defineConfig } from "vite-plus";
import { theme } from "./theme";
import { ansi } from "./theme/ansi";
import { table } from "./theme/table";

const themeCss = join(import.meta.dirname, "theme/style.css");

export default defineConfig({
  plugins: [
    {
      name: "restart-on-theme-css-change",
      configureServer: (server) => {
        server.watcher.on("all", (event, file) => {
          if (file !== themeCss || (event !== "change" && event !== "add")) return;
          void server.restart().then(() => server.ws.send({ type: "full-reload", path: "*" }));
        });
      },
    },
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
      transformers: [ansi, table],
      siteMaps: true,
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
        readerChrome: { copy: true, externalLinks: false, backToTop: false },
        pagination: true,
        notFound: true,
        markdownSource: true,
        lastUpdated: true,
        pageChrome: true,
        siteName: "Nuten",
        siteUrl: "https://nuten.keito.dev",
        ogImage: "https://nuten.keito.dev/og-image.png",
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
