---
name: desktop-progress-events
description: 'Use when changing structured run events, scenario counts, UI polling budgets, saved run history, omitted logs, or recovery from comparison reports.'
---

# Progress and Saved Activity

## Start Here

[Progress model](../../../src/progress.rs),
[runner persistence/polling](../../../src/desktop/runner.rs), and
[run UI](../../../src/desktop/ui/runs.rs).

## Procedure and Invariants

1. Preserve stable job/suite/scenario/capture identities. Multiple screenshots
   from one scenario must not inflate scenario counts or lose viewport context.
1. Bound event/log processing per UI tick; do not block on a worker-held progress
   mutex. Notify views only when their observable revision changes.
1. Distinguish execution failure, cancellation, visual mismatch, capture error,
   and successful completion. Keep structural errors out of website-result counts.
1. Persist only finished, supported activity for the matching workspace. Surface
   persistence errors and retain ownership until the write completes.
1. Recover reports by suite/output/run identity; do not infer successful execution
   logs or silently approve missing/ambiguous scenario metadata. Preserve history
   across later runs and different output directories.

## Checks

```sh
cargo test --lib progress::tests
cargo test --features desktop --lib desktop::runner::tests
```

Cover partial/invalid events, unchanged polling, contended locks, overflow/omission,
late writes, reload without rerun, absent suites, and legacy metadata.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill when status or
persistence semantics change. Comment identity and nonblocking polling invariants.
