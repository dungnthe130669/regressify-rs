import { readFileSync } from "node:fs";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { fixturePlugin } from "./scripts/fixture-plugin.ts";
import { getAssetVersion } from "./scripts/version-assets.ts";

export default defineConfig({
  base: "/",
  publicDir: false,
  define: {
    "import.meta.env.VITE_REGRESSIFY_ICON_VERSION": JSON.stringify(
      getAssetVersion(readFileSync(new URL("../assets/icon.svg", import.meta.url))),
    ),
  },
  plugins: [react(), tailwindcss(), fixturePlugin()],
  build: {
    assetsDir: "_snapshot/assets",
    assetsInlineLimit: 0,
    sourcemap: false,
    cssCodeSplit: false,
    rolldownOptions: {
      output: {
        comments: false,
        entryFileNames: "_snapshot/assets/app.js",
        chunkFileNames: "_snapshot/assets/[name].js",
        assetFileNames: "_snapshot/assets/[name][extname]",
      },
    },
  },
});
