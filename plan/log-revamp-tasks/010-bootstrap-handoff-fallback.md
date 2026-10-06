<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 010 — Bootstrap handoff with retained source and failure fallback

- Slug: `bootstrap-handoff-fallback`
- Implements plan section(s): `Phase 2 — Durable Operation History (Captures and Deployments)#0`, `Findings and Limits#0`
- Depends on: operation-journal-writer

## Goal

Regression-test and implement the bootstrap-to-operation-journal handoff so a failed copy/open retains the original bootstrap log, raises a visible warning, and never emits a successful-copy event on that failure path.

## Why this exists

This directly regression-tests the Istari review's top finding — 'Bootstrap handoff can discard recovery data' — which this plan explicitly calls out as a known defect pattern in the reference implementation that Regressify must not repeat.

## Settled investigation

- The Istari review found that common::logging::LoggingGuard::attach_project_log 'warns when the bootstrap copy fails, but then reports that it copied the log and attempts to remove the source once the destination writer opens' — the review explicitly notes 'failed-copy injection was not executed' in Istari's own validation, meaning this specific scenario has no existing proof in either codebase yet.
- Regressify's desktop worker (src/desktop/worker.rs::run_stdio) has an analogous early-startup moment — before crate::progress::install(path)? succeeds — where a bootstrap/startup diagnostic currently has nowhere durable to go if that install fails; this task's 'bootstrap log' is this plan's new pre-operation-journal diagnostic stream, not the existing progress sink itself.

## Inputs

- Task 2.1's journal writer
- The Istari review's exact finding text (Istari Design Review / Findings and Limits, first bullet)

## Outputs

- A handoff function that copies/opens a bootstrap log into the operation journal, retaining the original file until the destination write is verified
- A test that forces the copy/open to fail and asserts the original is retained, a warning is raised, and no successful-copy event is emitted

## Implementation steps

- Implement the handoff to write/open the destination (operation-journal-owned bootstrap segment) first, verify the write succeeded (read back a sentinel or confirm byte count), and only then remove/mark-consumed the original bootstrap source file — never the reverse order.
- On any failure during destination open/write, leave the original bootstrap file fully intact, raise an explicit warning, and do not emit any 'copied successfully' record.
- Write bootstrap_handoff_retains_source_on_destination_failure: inject a failure at the destination-open step (e.g. an unwritable directory) and assert (a) the original bootstrap file still exists with its original content, (b) a warning was raised, (c) no successful-copy event appears.
- Write a second test for the normal success path, confirming the original is removed/marked-consumed only after the destination is confirmed written.

## Done when

- Test bootstrap_handoff_retains_source_on_destination_failure forces a destination-open/write failure (e.g. pointing the destination at a read-only directory) and asserts the original bootstrap source file still exists with byte-identical content afterward.
- The same test asserts a warning was raised and that zero successful-copy records appear in the resulting output.

## Evidence to capture

- The exact failure-injection point used in bootstrap_handoff_retains_source_on_destination_failure (e.g. 'destination directory made read-only before open') and its three assertions: source retained byte-identical, warning raised, zero successful-copy records

## Do not change

- Do not remove or truncate the original bootstrap source file before the destination write is verified — this ordering is the entire point of the regression test.
- Do not adopt Istari's own current attach_project_log behavior as-is; the review explicitly found it defective.

## Reopen triggers

- A later desktop-startup task finds a second bootstrap-logging code path (beyond the one this task covers) that has the same discard-on-failure risk.

## Context

This is one of the few places this plan explicitly names a concrete defect in the system it reviewed (Istari) and requires Regressify to provably not repeat it. The review notes this exact scenario — failed-copy injection — was never actually executed against Istari either, so this task is net-new proof.

## Options & tradeoffs

Deleting the source optimistically after initiating (not confirming) the copy, as Istari's reviewed code does, is simpler but is the exact discard-recovery-data bug this task exists to avoid; verifying the destination write before touching the source (chosen) costs one extra confirmation step but makes data loss on this path structurally impossible rather than merely unlikely.

## Decision

Verify destination-write success before removing or marking the bootstrap source consumed, and never emit a successful-copy record on any failure path.

## Acceptance criteria

- Test bootstrap_handoff_retains_source_on_destination_failure proves the original bootstrap log file is retained, byte-identical, after a forced destination-open/write failure.
- The same test proves zero successful-copy events/records are emitted when the destination write failed.

## File touchpoints

- src/diagnostics/journal.rs (proposed)
- src/desktop/worker.rs (reference)

## Tests

- cargo test --lib diagnostics::journal::tests -- bootstrap_handoff_retains_source_on_destination_failure

## Dependencies

- operation-journal-writer

## Risks

- A test that only checks the function's return value without independently re-reading the filesystem for the original file's continued existence could pass even if the implementation deletes-then-fails; mitigate by asserting on the actual filesystem state of the source path, not just the function's reported outcome.
