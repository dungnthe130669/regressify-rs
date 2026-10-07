# 011: Log Tabs And Pagination

Status: Implementation and acceptance complete; authorized local commit pending.
Depends on 001.

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

## Implementation Evidence

- Task 010 is committed as `41fd5cc`; task 011 is uncommitted.
- Baselines passed: bounded reader (5), `run_log_` (7), `log_health_warning_` (1).
- Capture Log uses segmented Reference/Test TabBar and compact Pagination.
  Kit 0.7.1 tabs are pointer-only; the owner explicitly approved a scoped public
  adapter with two retained tab-stop handles and Enter/Space activation. Existing
  toolbar arrows perform traversal. The keyboard test failed before the adapter
  and passes using actual handles, since Tab observation omits focus metadata.
- Non-tail reader pages remain aligned and retain at most 500 records while
  clamping an out-of-range request to the last available page in one pass.
  Page arithmetic is checked. Empty/filtered/shrinking totals agree with displayed
  offsets; counts do not imply archive completeness or permit incomplete exports.
- Run Pagination is hidden during rolling Follow. Turning Follow off requests
  the last aligned page; invalidating an in-flight read allows its replacement
  to start and generation/key checks reject the stale completion. Existing
  no-process polling fixture is shared with the UI test through test-only scope.
- `log_pagination_` passes (2), covering 0/1/500/501 matches, huge/unaligned
  requests, partial archives, last-page Follow and in-flight filter changes.
  Capture source/page test passes (1) for later-page reset, rapid source changes
  and keyboard activation. Full reader passes (6), run logs (7), health warning (1).
- Native default passes with source-tab Right/Enter and Left/Space. Its first
  attempt could not focus the interactive desktop; the user requested a retry,
  and a fresh-build rerun passed with guards intact. Native Deployment also passes;
  an intermediate clipboard-restoration error was reported, then an unchanged
  complete retry passed. No clipboard data was logged or cleanup relaxed.
- Rustfmt, production GUI Clippy/build, editor diagnostics and headless smoke
  harness pass. Reviewed capture-log tabs/page status and compact activity layout.
  Docs, skills and shared memory describe bounded page/Follow/focus contracts;
  final guidance checks run before commit.
- Accounting: production +145/-96 (net +49), tests/helpers +280/-9, native
  assertions +17. The adapter adds two retained focus handles; paging reuses
  existing offset, loading and generation state and allocates at most one page.
- No new deferral. The approved Linux baseline exception and task 020 remain.
