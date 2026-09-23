---
name: snapshot-command
description: 'Use when changing regressify snapshot, HTML summary generation, suite filters, shared report assets, or Markdown headings.'
---

# Snapshot Reports

## Start Here

[Snapshot selection](../../../src/snapshot/actions.rs), [report generation](../../../src/snapshot/report.rs), and [templates/assets](../../../src/snapshot/data).

## Procedure and Invariants

1. Read existing comparison data from the active output directory, including environment/custom-output overrides. Preserve suite filters and explicit misses.
1. Resolve report text per field: explicit `--title`/`--heading`/`--subheading`, then workspace `snapshotTitle`/`snapshotHeading`/`snapshotSubheading`, then built-in defaults. Preserve explicit empty strings and CLI values equal to the defaults. Automatic `test`/`ref` reports must leave CLI text options unset.
1. Desktop Snapshot/Deploy must also leave unchosen report text unset in `RunOptions` and omit its CLI flags. Verify both the argument builder and bundled-worker staging; CLI-only tests do not cover desktop-generated overrides.
1. Preserve Markdown subheading rendering and plain-text title/heading escaping. Review each HTML, JS, JSON, and URL sink separately for untrusted report content.
1. Keep copied/embedded assets and generated hosting configuration consistent. Shared files must be replaced atomically, not left partly written.
1. Existing visual failures remain report data. Malformed input or generated-file write failures are process errors; do not broadly swallow optional-file errors.

## Checks

```sh
cargo test --lib snapshot::
cargo test --lib config::tests::snapshot_report_text_loads_from_workspace_yaml
```

Generate and inspect a temporary fixture report for UI changes. Cover empty data, multiple filtered suites, hostile labels/URLs, repeated generation, and write errors.

## Maintain

Update [command docs](../../../docs/commands.md), hosting docs if changed, and this skill. Keep report templates, shared assets, and downstream consumers synchronized.
