---
name: snapshot-command
description: 'Use when changing regressify snapshot, HTML summary generation, suite filters, shared report assets, or Markdown headings.'
---

# Snapshot Reports

## Start Here

[Snapshot selection](../../../src/snapshot/actions.rs),
[report generation](../../../src/snapshot/report.rs), and
[templates/assets](../../../src/snapshot/data).

## Procedure and Invariants

1. Read existing comparison data from the active output directory, including
   environment/custom-output overrides. Preserve suite filters and explicit misses.
1. Preserve title/heading/subheading and intended Markdown rendering, but review
   each HTML, JS, JSON, and URL sink separately for untrusted report content.
1. Keep copied/embedded assets and generated hosting configuration consistent.
   Shared files must be replaced atomically, not left partly written.
1. Existing visual failures remain report data. Malformed input or generated-file
   write failures are process errors; do not broadly swallow optional-file errors.

## Checks

```sh
cargo test --lib snapshot::
```

Generate and inspect a temporary fixture report for UI changes. Cover empty data,
multiple filtered suites, hostile labels/URLs, repeated generation, and write errors.

## Maintain

Update [command docs](../../../docs/commands.md), hosting docs if changed, and this
skill. Keep report templates, shared assets, and downstream consumers synchronized.
