import { oxContent } from "@ox-content/vite-plugin";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [
    oxContent({
      srcDir: "content",
      highlight: true,
      docs: false,
      ssg: {
        siteName: "Nuten",
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
