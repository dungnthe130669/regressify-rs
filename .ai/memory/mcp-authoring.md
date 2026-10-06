# MCP Authoring Export

Verified: 2026-10-06

- [The CLI exporter](../../src/mcp/mod.rs) embeds its guides and Worker template;
  it does not scan the current workspace or need a source checkout. Output is a
  new directory under an existing parent, with no-clobber file publication.
  Failures may leave a partial export; there is no automatic recursive cleanup.
- [The catalog](../../src/mcp/catalog.rs) derives six schemas from real Rust
  types, embeds eight guides, and includes 31 synthetic examples. The hooks and
  troubleshooting topics cover script contracts, local evidence, and safe repair.
  Tests check real parsers, local schema references, every property having direct
  or referenced help, action coverage, and local negative/boundary constraints.
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
