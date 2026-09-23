---
name: desktop-results-review
description: 'Use when changing desktop comparison reports, image previews, naming/filtering, diff dimensions, per-image URLs, capture inventory, or approval selection.'
---

# Desktop Results Review

## Start Here

[Report models](../../../src/desktop/results.rs), [results UI](../../../src/desktop/ui/reports.rs), and [approval intent](../../../src/desktop/ui/runs.rs).

## Procedure and Invariants

1. Treat report JSON and relative image paths as untrusted. Enforce artifact-root containment, file/byte/image-size bounds, and safe external URL schemes.
1. Keep captured human labels separate from immutable capture keys/scenario IDs. Legacy missing metadata is unavailable, not evidence of a pass or zero difference.
1. Diff dimension offsets are signed test-minus-reference pixels. Preserve raw referenceUrl versus replaced test url; do not share a display URL fallback.
1. Inventory successful baselines and actual captures, not directories or generated diffs. Select latest captures by the existing metadata/time rules, not lexical UUID.
1. Results exposes Approve This Scenario for the current viewport, never whole-run approval. Pin the output root, suite, run, scenario, and capture key. Revalidate identity and confirm the viewport before promotion; keep whole-run approval in Runs.
1. Reopen the same report after scoped approval and recompute its complete original scope, including unapproved failures, without recapturing or changing thresholds.
1. Keep expensive image/file loading off the UI thread and reject stale completion after selection changes. Own popup menus/subscriptions without retained cycles.

## Checks

```sh
cargo test --features desktop --lib desktop::results::tests
cargo test --features gui-tests --lib desktop::ui::tests -- --test-threads=1
```

Use native comparison smoke for changed menus/layouts. Cover corrupt/oversized images, escaped paths, missing metadata, renamed suites, signed offsets, selection changes, context-menu dismissal, and approval of only selected successful captures.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill. Explain identity, legacy compatibility, and resource-ownership decisions in code comments.
