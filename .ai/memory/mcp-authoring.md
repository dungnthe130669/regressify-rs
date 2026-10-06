# MCP Authoring Export

Verified: 2026-10-06

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
- Page-level failure markers, `engineErrorMsg`, and progress errors carry a typed
  summary (step, condition, duration, category); the console adds a sanitized
  local detail line that is never archived. Journals keep `ActionIndex`,
  `Condition`, and stage records. Identity fields are unsalted `sha256:` of the
  exact text, so local candidates can be hashed to confirm a match.
- New guide topics are discovered through the overview served by the same Worker
  version. The CLI-managed `AGENTS.md` section was left unchanged so a newer CLI
  cannot name a topic that an older hosted Worker rejects.
- Node examples and a companion suite use six-argument ESM hooks at `.mjs` target
  paths. [Worker tests](../../src/mcp/worker/tests/worker.test.mjs.template) import
  the exported modules and exercise them with stub pages; this is not browser
  coverage. Document/Node lifecycle order and timeout sources are explained in
  [the hooks guide](../../src/mcp/guides/hooks.md).
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
- See [the task skill](../../.github/skills/mcp-command/SKILL.md) for maintenance
  gates. Keep schema/runtime changes synchronized with the self-contained prose.
