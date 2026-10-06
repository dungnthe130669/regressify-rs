# 011: Log Tabs And Pagination

Status: Not started. Depends on 001.

## Scope

R12 source TabBar and non-tail Pagination in
[Capture Log](../../src/desktop/ui/reports/log.rs) and
[run logs](../../src/desktop/ui/runs/log.rs), using the bounded reader in
[desktop/logs.rs](../../src/desktop/logs.rs). Keep rolling Follow separate.

## Ordered Work

1. Baseline `desktop::logs::tests` with desktop feature and `run_log_` /
   `log_health_warning_` with gui-tests. Record nonzero counts before edits.
1. Replace Reference/Test buttons with segmented TabBar; preserve source/load
   callbacks, offset reset, unavailable text, stable scope and stale-read guards.
1. Adopt compact Pagination for non-tail archives. Derive pages with
   `matched.div_ceil(PAGE_RECORDS).max(1)` and page with
   `offset / PAGE_RECORDS + 1`. Convert requested pages using checked arithmetic
   and current async loader/archive-key invalidation.
1. Refetch/reconcile shrinking totals or changed filters that invalidate offset;
   model and displayed page must agree. Keep loading/availability guards,
   partial/missing/legacy/error text and export restrictions. Matched count
   never proves archive completeness.
1. Leaving Follow explicitly loads the last page-aligned page. Do not label
   `matched - returned_records` as an ordinary numbered page. Preserve bounded
   reads and complete-filter export without allocating the full log.
1. Validate each small change immediately; extend existing tests for 0/1/500/501
   matches, first/last, later-page filter, shrinking totals, loading clicks,
   source switching, stale results, partial archives and live Follow transitions.
1. Maintain docs/skills/memory and commit with exact evidence and code accounting.

## Acceptance And Checks

```sh
cargo test --locked --features desktop --lib desktop::logs::tests
cargo test --locked --features gui-tests --lib run_log_ -- --test-threads=1
cargo test --locked --features gui-tests --lib log_health_warning_ -- --test-threads=1
cargo fmt --all -- --check
```

Require nonzero counts. Add boundary tests in existing modules and verify the
selected filters execute them. Follow
[diagnostics](../../.github/skills/logging-diagnostics/SKILL.md),
[progress events](../../.github/skills/desktop-progress-events/SKILL.md),
[results review](../../.github/skills/desktop-results-review/SKILL.md),
[native UI](../../.github/skills/desktop-native-ui/SKILL.md) and
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md).
Commit tabs/paging/tests/maintenance only. Reopen for unbounded allocation,
stale-source overwrite, false completeness or unsupported page representation.
