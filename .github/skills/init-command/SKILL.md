---
name: init-command
description: 'Use when changing regressify init, starter files, legacy migration, editor settings/schema setup, managed MCP or AGENTS.md guidance, or non-destructive workspace initialization.'
---

# Initialize a Workspace

## Start Here

[Initialization](../../../src/init.rs) owns starter generation, migration, workspace-directory validation, and write-if-missing behavior. Desktop [session connection](../../../src/desktop/session.rs) invokes shared runtime and agent setup.

## Procedure and Invariants

1. Create only missing starter files, except for the managed MCP endpoint and agent section below. Preserve project suites, config, scripts, and unrelated editor settings; a second invocation must be non-destructive.
1. Both CLI and desktop setup create `regressify.yaml` with inherited defaults. Respect `.yml` alternatives for workspace config and replacement profiles; do not generate a higher-priority default that shadows existing settings.
1. Automatic desktop setup creates runtime defaults, the suite folder, MCP configuration, and managed agent instructions. Keep examples, exported schemas, other VS Code settings/extensions, and legacy migration in explicit CLI init. Do not generate Node helpers, report artifacts, or persisted states.
1. CLI init and desktop folder setup create or merge `.vscode/mcp.json`. Add a missing HTTP
   `regressify-mcp` server; refresh only an existing server's `url` from
   `REGRESSIFY_MCP_URL`. Preserve other fields, servers, inputs, comments, and
   trailing commas with JSONC CST edits. A matching URL must not rewrite the file.
   Reject malformed JSONC and non-object entries without exposing file contents
   in diagnostics. Read-only files needing an update must fail unchanged.
1. CLI init and desktop folder setup manage the root `AGENTS.md` section titled
   `## Regressify CMP`, creating or appending it when absent. Use `pulldown-cmark`
   top-level H2 offsets for ownership through the next H2 or EOF and parsed events
   for freshness. Preserve outside bytes and current Markdown formatting; replace
   outdated content, including subsections, and consolidate duplicate managed
   sections. Ignore code/quote/list/comment examples when locating headings.
   Refuse writes when an unfinished block would swallow an appended heading.
1. Migrate legacy files only when the new destination is absent. Do not replace project-specific content with defaults or erase old data on a failed write.
1. Validate each workspace path and reject symlinked directories/unsafe entries before creating files, including dangling links at the destination. Existence is not proof an entry is a regular file. Publish complete starter files using same-directory temporary files and no-clobber persistence.
   For MCP and agent-guidance updates, recheck the path and original content before
   atomic replacement; this is not a compare-and-swap guarantee against concurrent
   external writers.
1. Treat permission, conflict, and write failures as command failures with context.

## Checks

```sh
cargo test --lib init::tests
cargo test --features desktop --lib init::tests
cargo test --features desktop --lib desktop::session::tests
```

Cover first/repeated init, deleted defaults, existing custom settings and `.yml` files, legacy migration, concurrent creation, read-only files, symlink rejection, and failed opens in temporary directories. Do not run init in the user's working project as a test. Unix symlink cases require a Unix test host.
Include MCP URL refresh, matching-URL no-ops, missing maps/URLs, JSONC comments and
line endings, invalid drafts, retained server settings, and concurrent creation.
Cover agent-section creation/append/refresh, parsed freshness, H2 versus other
heading levels, ATX/Setext headings, EOF boundaries, misleading examples,
duplicate sections, CRLF/UTF-8 preservation, read-only files, and blocked drafts.
Exercise both managed files through desktop `Session::connect`, including
reopening after deletion, refresh/no-op behavior, and preserving the current
workspace and source files when setup fails.

## Maintain

Update [getting started](../../../docs/get-started.md), [command docs](../../../docs/commands.md), and this skill when generated content or migration behavior changes. Explain preservation rules in code comments.
