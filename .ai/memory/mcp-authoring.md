# MCP Authoring Export

Verified: 2026-10-14

- [The CLI exporter](../../src/mcp/mod.rs) embeds its guides and Worker template;
  it does not scan the current workspace or need a source checkout. Output is a
  new directory under an existing parent, with no-clobber file publication.
  Failures may leave a partial export; there is no automatic recursive cleanup.
- [The catalog](../../src/mcp/catalog.rs) derives six schemas from real Rust
  types, embeds nine guides (including `recipes`), and includes 32 synthetic
  examples. The hooks and troubleshooting topics cover script contracts, local
  evidence, and safe repair. Tests check real parsers, local schema references,
  every property having direct or referenced help, action coverage, local
  negative/boundary constraints, preflight action rules, and every guide YAML
  fence as a complete file.
- Runtime facts verified for the guides: `wait` with `url` matches the URL within
  `elementTimeout` first, then sleeps or waits for the selector; action selectors
  use the bundled Playwright engines, but hide/remove and screenshot selectors
  are plain CSS in the main document; captures and `report.json` share
  `<output>/<suite>/bitmaps_test/<run>/` (there is no `tests/<run>` folder).
- Suite preflight (`validate_action_fields`, `validate_action_files`, and
  screenshot selector prefixes) rejects schema-valid but unusable actions before
  browser launch; the actions guide lists them under "Checks Beyond the Schema".
- Element captures are pixel clips of the target's border box after centering
  it: overflowing menus, drawers, and body-level panels are cut off, while
  overlays drawn inside the box are included. `viewport` is reserved in
  `selector`, `selectors`, and `save.selector`. Verified 2026-10-06 with a
  cached-Chromium probe and the selector pixel fixture; QA reported agents
  capturing `header` after opening menus.
- Related closed/default and opened/expanded states now share viewport context
  captures in the navigation example and recipe. A header-only initial image
  can miss premature expansion outside its box; do not force-close the initial
  state. Startup/overview guidance recommends the same larger viewport/full-page
  area, matching viewport/scroll position, and supplementary element details.
  Catalog/Worker regressions cover delivery and matching recipe/example targets.
- Page-level failure markers, `engineErrorMsg`, and progress errors carry a typed
  summary (step, condition, duration, category); the console adds a sanitized
  local detail line that is never archived. Journals keep `ActionIndex`,
  `Condition`, and stage records. Identity fields are unsalted `sha256:` of the
  exact text, so local candidates can be hashed to confirm a match.
- New guide topics are discovered through the overview served by the same Worker
  version. Keep CLI-managed `AGENTS.md` tool/topic references compatible with
  older hosted Workers; the consumer memory policy adds no tools or guide topics.
- [Startup instructions](../../src/mcp/catalog.rs) and the
  [overview](../../src/mcp/guides/overview.md) teach agents to read and maintain the
  consumer project's `.ai/memory/` with client tools. Notes are at most 60 lines,
  one behavior or issue on one site per file, with a compact linked index. Record
  verified lessons, failed approaches, fixes, and evidence; split/update notes and
  exclude private data. The server never reads or stores memory. Catalog and
  Worker regressions cover startup, tool/resource, and non-overview prompt delivery.
- Startup instructions and the overview require agents to run the installed
  `regressify ref --test-suite <suite-name>` after every consumer suite edit or
  creation. CLI and desktop are installed together on the user's PATH. Catalog
  and Worker regressions cover fresh intended screenshots, not exit code zero or
  old references, plus reference-write authorization and incomplete verification
  when blocked. [Reference execution](../../src/test/test_suites.rs) preserves
  old references on capture failure and can exit zero after page failures.
- Hook examples and a companion suite use classic browser scripts at `.js` target
  paths. [Worker tests](../../src/mcp/worker/tests/worker.test.mjs.template) evaluate
  the exported scripts with DOM stubs; this is not browser coverage or a capture
  dependency. Browser-only lifecycle order and timeout sources are explained in
  [the hooks guide](../../src/mcp/guides/hooks.md).
- The overview and hooks guide explicitly prohibit CommonJS `module.exports`
  and Playwright `page` callbacks; `.cjs` does not select a module loader, and
  renaming the file does not convert its contents. Navigation belongs in suite
  actions; metadata comes from `window.regressify`. The existing
  [Chromium hook fixture](../../src/test/hooks.rs) verifies missing `module` and
  `page` globals in before, shared-ready, and scenario-ready scripts. Catalog
  regressions require the warnings and migration guidance in the exported text.
- Standalone checks permit consumer `AGENTS.md` references but reject source-only
  dependencies. Managed MCP/agent initialization and legacy text-append shortcuts
  are documented; explicit action lists suppress legacy shortcuts.
- [Viewport schema metadata](../../src/config/viewports.rs) is available in the
  default CLI feature set, not just desktop. Cookies retain runtime-validated
  prose/examples instead of a separately maintained typed schema.
- [The Worker](../../src/mcp/worker/src/index.ts.template) uses the pinned Cloudflare
  stateless MCP handler, with read-only tools/resources/prompts, Origin checks,
  bounded bodies, and no bindings. Public guidance is not authorization to read
  local files, execute captures, or approve baselines.
- [Protocol tests](../../src/mcp/worker/tests/worker.test.mjs.template) cover SDK legacy
  compatibility and current HTTP metadata. Invalid tool arguments are tool-error
  results; stateless tests provide `MCP-Protocol-Version` and `Mcp-Method` headers.
- [CI](../../.github/workflows/mcp-checks.yml) builds the default CLI, copies it
  into an empty directory, exports a package, and runs npm installation,
  typechecking, local MCP tests, and a Wrangler dry run. No publishing is implied.
- Wrangler 4.147 passes `upload_source_maps` to esbuild as `sourcemap`, and an
  unset value means `true`: maps were written to `dist` (not uploaded) and the
  bundle was unminified. The template now sets `minify: true` and
  `upload_source_maps: false`; a package test checks the dry-run output.
- See [the task skill](../../.github/skills/mcp-command/SKILL.md) for maintenance
  gates. Keep schema/runtime changes synchronized with the self-contained prose.
