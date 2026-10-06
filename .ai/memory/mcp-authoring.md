# MCP Authoring Export

Verified: 2026-10-06

- [The CLI exporter](../../src/mcp/mod.rs) embeds its guides and Worker template;
  it does not scan the current workspace or need a source checkout. Output is a
  new directory under an existing parent, with no-clobber file publication.
  Failures may leave a partial export; there is no automatic recursive cleanup.
- [The catalog](../../src/mcp/catalog.rs) derives six schemas from real Rust
  types, embeds six authoring guides, and includes synthetic action/shared-file
  examples. Its tests check real parsers, schema validity/local references,
  action coverage, deterministic export, and representative invalid inputs.
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
