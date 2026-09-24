---
name: desktop-results-review
description: 'Use when changing desktop comparison reports, image previews, naming/filtering, diff dimensions, per-image URLs, capture inventory, or approval selection.'
---

# Desktop Results Review

## Start Here

[Report models](../../../src/desktop/results.rs), [results UI](../../../src/desktop/ui/reports.rs), and [approval intent](../../../src/desktop/ui/runs.rs).

## Procedure and Invariants

1. Treat report JSON and relative image paths as untrusted. Enforce artifact-root containment, file/byte/image-size bounds, and safe external URL schemes.
1. Suite labels prefer the source catalog's nonblank `displayName`, then captured report metadata for unmatched suites, then legacy names/IDs. Labels never control artifact paths or approval identity. Refresh catalogs/results after suite rename/delete and preserve unrelated reports.
1. Keep captured human labels separate from immutable capture keys/scenario IDs. Prefer recorded `scenarioDisplayName`; use the [shared URL naming resolver](../../../src/helpers/scenario_names.rs) for legacy reports. Resolve across logical scenarios, not individual viewports/captures, and keep desktop/editor/web names aligned. Legacy missing metrics are unavailable, not evidence of a pass or zero difference.
1. Diff dimension offsets are signed test-minus-reference pixels. Preserve raw referenceUrl versus replaced test url; do not share a display URL fallback.
1. Inventory successful baselines and actual captures, not directories or generated diffs. Select latest captures by the existing metadata/time rules, not lexical UUID.
1. Results exposes Approve This Scenario for the current viewport, never whole-run approval. Pin the output root, suite, run, scenario, and capture key. Confirm inline in the comparison and retain draft/preflight guards without switching to Runs. Dismiss pending approval on capture changes or refresh; keep whole-run approval in Runs.
1. Refresh the currently viewed report in place after scoped approval, preserving capture/filter/view state and its output root. Recompute its complete original scope, including unapproved failures, without recapturing or changing thresholds; show approval errors in Results.
1. Idle Results suite badges use the loaded report's scenario/viewport totals, not stale Activity history or raw screenshot counts. Keep live execution counts while a worker is running.
1. Keep expensive image/file loading off the UI thread and reject stale completion after selection changes. Own popup menus/subscriptions without retained cycles.
1. Capture-list keyboard navigation uses filtered positions and the retained uniform-list scroll handle. Keep capture selection single-item, clear pending approval through the existing selection path, and leave capture-search shortcuts scoped to its text input.
1. Fit-mode zoom readouts use current laid-out image bounds, not the stored manual zoom. Show one decimal place and cover both fit modes, image tabs, capture changes, and preview resizing with the `image_previews_` GUI tests.

## Checks

```sh
cargo test --features desktop --lib desktop::results::tests
cargo test --features desktop --lib snapshot::capture_report::tests
cargo test --features gui-tests --lib desktop::ui::tests -- --test-threads=1
```

Use native comparison smoke for changed menus/layouts. Cover corrupt/oversized images, escaped paths, missing metadata, renamed suites, signed offsets, selection changes, context-menu dismissal, and approval of only selected successful captures.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill. Explain identity, legacy compatibility, and resource-ownership decisions in code comments.
