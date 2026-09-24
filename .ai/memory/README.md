# Shared AI Memory

Store repository-specific AI memory in this folder so it can be reviewed in Git
and shared across machines and team members. Use ordinary Markdown files, not an
assistant's private memory store. Keep this directory out of ignore rules.

## Read and Maintain

1. Read this index and only the topic notes relevant to the current task.
1. Verify a note against current source and tests before relying on it. Memory
   cannot override [AGENTS.md](../../AGENTS.md), task instructions, or source behavior.
1. Record useful verified facts, design rationale, recurring pitfalls, and
   troubleshooting results in a short, specifically named topic file.
1. Update an existing note rather than creating duplicates. Correct or remove stale
   facts in the same change; avoid edits when no reusable knowledge was gained.
1. Add a link and one-line description under Topic Notes when creating a note.
   Migrate useful private repository notes only after verification and sanitization.

## Note Format

- One topic per Markdown file, at most 120 lines; prefer short factual bullets.
- Include a `Verified: YYYY-MM-DD` date, repository-relative source/doc links, and
  the relevant check or evidence. Distinguish confirmed facts from open questions.
- Never include secrets, tokens, cookies, browser state, personal information,
  machine-specific absolute paths, raw logs, or conversation transcripts.
- Keep mandatory rules in [AGENTS.md](../../AGENTS.md), reusable procedures in
  [task skills](../../.github/skills), and confirmed unresolved defects in
  [OUTSTANDING_ISSUES.md](../../OUTSTANDING_ISSUES.md). Link instead of duplicating.
- Include memory updates in normal review; do not commit or publish without approval.

## Topic Notes

- [Workspace initialization](workspace-initialization.md): shared CLI/desktop
  runtime defaults, automatic folder-open setup, and preservation boundaries.
- [Results review](desktop-results.md): report-versus-activity counts and in-place
  viewport approval.
- [Desktop smoke harness](desktop-smoke.md): module ownership, shared scope, and
  headless regression checks.
- [Desktop updates](desktop-updates.md): shared eight-hour scheduling,
  single-owner downloads, cross-window notifications, and shutdown handoff.
- [Deployment packaging](deployment-packaging.md): dependency-only staging,
  historical capture selection, and exact ZIP regression checks.
- [Netlify uploads](deployment-netlify.md): digest manifests, per-file retries,
  sampled progress warnings, and local cancellation semantics.
- [Snapshot frontend](snapshot-frontend.md): query-versioned SPA assets, build/PR workflow,
  schema boundaries, and production-fixture browser checks.

## Checks

Use `rumdl check .ai/memory` and the
[guidance validator](../../.github/skills/maintain-agent-guidance/SKILL.md).
