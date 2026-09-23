import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { fixturePlugin } from "./scripts/fixture-plugin.ts";

export default defineConfig({
  base: "/",
  publicDir: false,
  plugins: [react(), tailwindcss(), fixturePlugin()],
  build: {
    assetsDir: "_snapshot/assets",
    assetsInlineLimit: 0,
    sourcemap: false,
    cssCodeSplit: false,
    rolldownOptions: {
      output: {
        entryFileNames: "_snapshot/assets/app.js",
        chunkFileNames: "_snapshot/assets/[name].js",
        assetFileNames: "_snapshot/assets/[name][extname]",
      },
    },
  },
});
