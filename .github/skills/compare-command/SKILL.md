---
name: compare-command
description: 'Use when changing regressify compare, capture-run selection, image diff thresholds, manifests, comparison concurrency, or report regeneration.'
---

# Compare Captures

## Start Here

[Command selection](../../../src/compare/actions.rs),
[report generation](../../../src/compare/report_gen.rs), and
[pixel comparison](../../../src/compare/image_compare.rs).

## Procedure and Invariants

1. Honor explicit run IDs before latest-run fallback. Resolve suite globs and
   scenario IDs against output/capture metadata, not current source ordering.
1. Validate manifests and expected capture inventory. Generated diffs are not
   source captures; stale diffs must disappear when a pair becomes a pass.
1. Bound parallel comparison with `asyncCompareLimit`; preserve manifest ordering,
   per-capture thresholds, labels, target names, and reference/test page URLs.
1. Missing references, capture errors, and visual mismatches remain report results.
   Malformed manifests, unreadable files, and report-write failures are process errors.
1. Preserve PNG safety limits and signed dimension differences. Do not conflate
   a size mismatch with a decoding or I/O failure.
1. Report-viewport approval recalculates the reviewed capture list, order, and
   per-pair thresholds, not a narrowed selection or the current workspace settings.

## Checks

```sh
cargo test --lib compare::
```

Cover explicit/latest run choice, missing/unexpected captures, repeat generation,
threshold boundaries, parallel order, malformed images, and failed artifact writes.

## Maintain

Update [command docs](../../../docs/commands.md) and downstream desktop/report
consumers when changing persisted report shape. Explain ordering/error invariants
with concise comments and keep this skill current.
