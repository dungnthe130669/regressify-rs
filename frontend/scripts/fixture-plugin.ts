import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { Plugin } from "vite";
import { artifactPath } from "../src/data/report.ts";
import { getAssetVersion } from "./version-assets.ts";

export function fixturePlugin(): Plugin {
  return {
    name: "snapshot-development-fixture",
    apply: "serve",
    async transformIndexHtml(html) {
      const manifest = await readFile(
        resolve(import.meta.dirname, "../.fixtures/_snapshot/data/manifest.json"),
      );
      return html.replace(
        "__SNAPSHOT_MANIFEST__",
        `_snapshot/data/manifest.json?v=${getAssetVersion(manifest)}`,
      );
    },
    configureServer(server) {
      server.middlewares.use(async (request, response, next) => {
        const url = new URL(request.url || "/", "http://localhost");
        const file = artifactPath(
          `${url.pathname.slice(1)}${url.search}`,
          url.pathname.startsWith("/_snapshot/data/") ? "data" : "images",
        );
        if (!file) return next();
        try {
          const bytes = await readFile(resolve(import.meta.dirname, "../.fixtures", file));
          if (url.searchParams.get("v") !== getAssetVersion(bytes)) {
            response.statusCode = 409;
            response.end("Fixture changed; reload the report.");
            return;
          }
          response.setHeader(
            "Content-Type",
            file.endsWith("png") ? "image/png" : "application/json",
          );
          response.end(bytes);
        } catch {
          response.statusCode = 404;
          response.end("Fixture unavailable. Run bun run fixtures.");
        }
      });
    },
  };
}
