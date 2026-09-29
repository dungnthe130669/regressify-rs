---
name: compare-command
description: 'Use when changing regressify compare, capture-run selection, image diff thresholds, manifests, comparison concurrency, or report regeneration.'
---

# Compare Captures

## Start Here

[Command selection](../../../src/compare/actions.rs), [report generation](../../../src/compare/report_gen.rs), and [pixel comparison](../../../src/compare/image_compare.rs).

## Procedure and Invariants

1. Honor explicit run IDs before latest-run fallback. Resolve suite globs and scenario IDs against output/capture metadata, not current source ordering.
1. Validate manifests and expected capture inventory. Generated diffs are not source captures; stale diffs must disappear when a pair becomes a pass.
1. Bound parallel comparison with `asyncCompareLimit`; preserve manifest ordering, per-capture thresholds, labels, target names, and reference/test page URLs.
1. Keep pixel sensitivity separate from pass/fail percentage. `pixelMatchThreshold` uses 0-100 (default 5), normalized by 100 only in the image comparer. Read per-capture values from `.report-pixel-thresholds.json`, including prefix fallback for intermediate captures; absent metadata uses 5, malformed metadata is an artifact error.
1. Read optional suite-root `suite.json` through the bounded metadata reader and emit `displayName` independently of `id`/`testSuite`. Captures write metadata even with skip-compare. Malformed metadata is an artifact error, not an absent label.
1. Missing references, capture errors, and visual mismatches remain report results. Malformed manifests, unreadable files, and report-write failures are process errors.
1. Preserve PNG safety limits and signed dimension differences. Do not conflate a size mismatch with a decoding or I/O failure.
1. Report-viewport approval recalculates the reviewed capture list, order, and per-pair thresholds, not a narrowed selection or the current workspace settings.
1. Persist `pixelMatchThreshold` on report pairs. A reviewed legacy report without it used normalized 0.1, so deserialize that missing field as 10 for approval; do not substitute the new-capture default.

## Checks

```sh
cargo test --lib compare::
```

Cover explicit/latest run choice, missing/unexpected captures, repeat generation, threshold boundaries, parallel order, malformed images, and failed artifact writes.

## Maintain

Update [command docs](../../../docs/commands.md) and downstream desktop/report consumers when changing persisted report shape. Explain ordering/error invariants with concise comments and keep this skill current.
