<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 028 — Suite rename/delete and reference-promotion provenance for logs

- Slug: `snapshot-log-provenance-rename-delete`
- Implements plan section(s): `Phase 6 — Frontend and Packaging#0`, `Snapshot Frontend and Publication#0`
- Depends on: snapshot-log-packaging-inventory

## Goal

Make renaming/deleting a suite update associated log assets per existing inventory rules without deleting still-referenced history, and make promoting a reference not fabricate a new log for the promoted capture.

## Why this exists

Task 6.4's own Task Breakdown acceptance criterion requires exactly this; Snapshot Frontend and Publication separately requires preserving capture-to-log provenance through compare, snapshot regeneration, and reference promotion.

## Settled investigation

- src/desktop/workspace.rs::SuiteSelection::rename_suite (line 98) and Workspace::rename_suite/rename_suite_in_outputs (lines 701-708+) are the existing suite-rename entry points; src/progress.rs::LiveProgress::remap_suite (#[cfg(feature = "desktop")], line 407) and src/desktop/runner.rs::RunManager::remap_suite (line 856) are the existing progress/run-history remap functions invoked on rename/delete today.
- The desktop-results-review skill documents the existing invariant: 'Refresh catalogs/results after suite rename/delete and preserve unrelated reports' — this task's log-asset handling is a new instance of that same existing invariant.
- Snapshot Frontend and Publication is explicit that 'A successful approval does not create a new browser run' — approving/promoting a reference capture reuses the existing test capture's already-recorded execution log rather than needing a new one.

## Inputs

- Task 6.3's selection-scoped log packaging
- The existing rename_suite/remap_suite call chain in src/desktop/workspace.rs, src/progress.rs, and src/desktop/runner.rs (extended, not replaced)

## Outputs

- Log assets correctly follow a suite through rename, are removed (only when no longer referenced) on delete, and are left untouched (not fabricated or duplicated) on reference promotion

## Implementation steps

- Extend the existing rename/remap call chain (SuiteSelection::rename_suite at src/desktop/workspace.rs line 98, Workspace::rename_suite_in_outputs at lines 701-708+, LiveProgress::remap_suite at src/progress.rs line 407, RunManager::remap_suite at src/desktop/runner.rs line 856) to also update any log-asset references keyed by the old suite id/name.
- On suite delete, remove only log assets no longer referenced by any remaining selection/history, reusing Task 6.3's existing inventory rules.
- On reference promotion (approval), confirm the promoted capture's existing execution-log reference is carried over unchanged, with no new log fabricated, since approval does not create a new browser run.
- Write suite_rename_preserves_log_assets: rename a suite with an existing log asset and assert the asset is reachable under the new id/name and not duplicated.
- Write suite_delete_removes_only_unreferenced_logs: delete a suite and assert only its exclusively-owned log assets are removed while any still-referenced shared history remains.
- Write reference_promotion_does_not_fabricate_log: promote a reference and assert no new log asset is created for the promoted capture.

## Done when

- Test suite_rename_preserves_log_assets renames a suite with 1 existing log-bearing capture and asserts the asset is reachable under the new id/name with exactly 1 copy (not duplicated).
- Test suite_delete_removes_only_unreferenced_logs deletes one of 2 suites (each with its own log-bearing capture) and asserts exactly the deleted suite's log asset is removed while the remaining suite's log-asset count is unchanged.
- Test reference_promotion_does_not_fabricate_log promotes a test capture to reference and asserts the promoted capture's log-asset reference is identical (same path/hash) before and after promotion, with the total log-asset count unchanged.

## Evidence to capture

- Each of the three tests' before/after inventory counts: rename (1 asset reachable under new id), delete (exactly 1 of 2 removed), promote (0 new assets created, same reference before/after)

## Do not change

- Do not create a new, separate rename/delete handling path for logs — extend the existing rename_suite/remap_suite chain.
- Do not fabricate a log for a promoted reference capture; the existing test capture's log is the only source of truth for that execution.

## Reopen triggers

- A future task finds the existing remap_suite chain does not cover a rename/delete path this task assumed it did (e.g. a web-only deletion flow with no desktop RunManager involved).

## Context

Logs are durable artifacts tied to captures/reports, but suite identity (name/id) can change independently via rename, and suites can be deleted; without this task, a renamed suite's logs would be silently orphaned or a deleted suite's logs could leak into an unrelated new suite reusing the same id.

## Options & tradeoffs

Treating log assets as immutable artifacts keyed only by capture/execution id (never suite id) would avoid needing any rename handling at all, but the existing inventory/cleanup rules are suite-scoped today, so a log asset must still be reachable through whatever suite-scoped lookup the rest of the report uses; extending the existing remap chain (chosen) keeps this consistent with how every other suite-scoped artifact already behaves on rename/delete.

## Decision

Extend the existing rename_suite/remap_suite call chain to also carry log-asset references, and reuse Task 6.3's existing-reference inventory rule for delete-time cleanup, with reference promotion an explicit no-new-log no-op.

## Acceptance criteria

- Test suite_rename_preserves_log_assets shows a suite's existing log asset remains reachable and unduplicated (count=1) under the suite's new id/name.
- Test suite_delete_removes_only_unreferenced_logs shows only the deleted suite's exclusively-owned log asset is removed (1 of 2 total) while the remaining suite's still-referenced history (the other 1) remains, and test reference_promotion_does_not_fabricate_log shows no new log asset is created for a promoted capture (asset count unchanged, same reference).

## File touchpoints

- src/desktop/workspace.rs
- src/progress.rs
- src/desktop/runner.rs
- src/snapshot/site.rs

## Tests

- cargo test --features desktop --lib desktop::workspace::tests -- suite_rename_preserves_log_assets
- cargo test --features desktop --lib snapshot::site::tests -- suite_delete_removes_only_unreferenced_logs
- cargo test --features desktop --lib desktop::results::tests -- reference_promotion_does_not_fabricate_log

## Dependencies

- snapshot-log-packaging-inventory

## Risks

- If delete-time cleanup runs before a concurrent in-flight capture finishes writing its log, a race could delete a log still being written; mitigate by reusing Full-Log Persistence's existing requirement that cleanup 'must respect... active writes,' verified by a dedicated concurrency test alongside Task 7.1's broader concurrency suite.
