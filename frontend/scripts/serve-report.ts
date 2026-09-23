import { lstat, readFile, realpath } from "node:fs/promises";
import { join, resolve } from "node:path";
import { artifactPath } from "../src/data/report";
import { getAssetVersion } from "./version-assets";

const arguments_ = process.argv.slice(2).filter((argument) => argument !== "--");
const root = await realpath(
  resolve(arguments_[0] || resolve(import.meta.dir, "../.fixtures/workspace/.regressify")),
);
const index = await lstat(join(root, "index.html")).catch(() => null);
if (!index?.isFile() || index.isSymbolicLink())
  throw new Error(
    "Generate a Rust snapshot first, or pass its output directory to bun run preview.",
  );
const config = JSON.parse(
  await readFile(resolve(import.meta.dir, "../hosting/staticwebapp.config.json"), "utf8"),
) as { globalHeaders: Record<string, string> };

const server = Bun.serve({
  hostname: "127.0.0.1",
  port: Number(arguments_[1] || 4174),
  async fetch(request) {
    const url = new URL(request.url);
    const pathname = url.pathname;
    const relative =
      pathname === "/" || pathname === "/index.html" || pathname.startsWith("/suites/")
        ? "index.html"
        : /^\/_snapshot\/assets\/[\w.-]+$/.test(pathname) &&
            /^[A-Za-z0-9_-]{20}$/.test(url.searchParams.get("v") || "")
          ? pathname.slice(1)
          : artifactPath(
              `${pathname.slice(1)}${url.search}`,
              pathname.startsWith("/_snapshot/data/") ? "data" : "images",
            );
    if (!relative) return new Response("Not found", { status: 404 });
    let path = root;
    for (const part of relative.split("/")) {
      if (!part || part === "." || part === "..")
        return new Response("Invalid path", { status: 400 });
      path = join(path, part);
      const metadata = await lstat(path).catch(() => null);
      if (!metadata || metadata.isSymbolicLink()) return new Response("Not found", { status: 404 });
    }
    const file = Bun.file(path);
    if (!(await lstat(path)).isFile()) return new Response("Not found", { status: 404 });
    const bytes = await readFile(path);
    if (relative !== "index.html" && url.searchParams.get("v") !== getAssetVersion(bytes))
      return new Response("Snapshot changed; reload the report.", {
        status: 409,
        headers: { "Cache-Control": "no-store" },
      });
    return new Response(bytes, {
      headers: {
        ...config.globalHeaders,
        "Content-Type": file.type,
        "Cache-Control":
          relative === "index.html" ? "no-cache" : "public, max-age=31536000, immutable",
      },
    });
  },
});
console.log(`Snapshot preview: ${server.url}`);
