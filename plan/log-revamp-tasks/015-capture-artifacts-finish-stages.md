<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 015 — Capture/artifacts and finish stage instrumentation

- Slug: `capture-artifacts-finish-stages`
- Implements plan section(s): `Phase 3 — Capture Instrumentation#0`, `Capture Diagnostics#0`
- Depends on: capture-queue-setup-stage

## Goal

Instrument capture/artifacts (capture ID, dimensions, encode/write duration, safe artifact identity, manifest completion, write/cleanup failures) and finish (capture outcome, teardown, terminal error chain, cancellation cause, diagnostic archive completeness) stages, keeping comparison outcome distinct from capture failure.

## Why this exists

Task 3.4's own acceptance criterion requires a passed capture with a visual mismatch to show capture outcome = success and comparison outcome = mismatch as distinct fields.

## Settled investigation

- src/test/capture.rs already defines ArtifactWriteError/artifact_write_error/is_artifact_write_error (lines 36-60) as the existing marker for 'tool failures' distinct from a page that fails to render — this task's write/cleanup-failures field must key off this existing distinction.
- src/test/test_suites.rs's TaskStatus enum (lines 38-46: Passed, PageFailed, InfraFailed) and src/test/test_suite.rs's ScenarioOutcome (imported at test_suites.rs line 29) are the existing capture-outcome classifications; this task's capture-outcome field must map onto these, not invent a fourth parallel one.
- progress::Event::Compared{suite_id, capture, label, scenario_id, passed, error} (src/progress.rs lines 102-110) is the existing lifecycle event carrying comparison (not capture) outcome — confirming capture outcome and comparison outcome are already two separate concepts this task's records must keep separate.

## Inputs

- Task 3.1's queue/setup stage instrumentation
- src/test/capture.rs's ArtifactWriteError marker (reference)
- src/test/test_suites.rs's TaskStatus/ScenarioOutcome (reference)

## Outputs

- Capture/artifacts and finish diagnostic records, with capture outcome and comparison outcome as distinct fields

## Implementation steps

- Add tracing spans/events around the screenshot capture call (dimensions, encode/write duration) and around artifact write/cleanup, using is_artifact_write_error (src/test/capture.rs lines 36-60) to classify a write/cleanup failure distinctly from a page-level capture failure.
- Record a safe artifact identity (capture ID plus suite/scenario identity) rather than a raw filesystem path.
- Add a finish-stage record capturing the final TaskStatus/ScenarioOutcome-derived capture outcome, teardown duration, any terminal error chain (reusing Task 1.3's SanitizedError), cancellation cause when applicable, and the archive's own completeness state.
- Ensure the finish record's capture-outcome field is populated independently of any later comparison outcome, and that a subsequent progress::Event::Compared does not overwrite or merge into this record.
- Write passed_capture_with_mismatch_shows_distinct_capture_and_comparison_outcomes: run a synthetic scenario that captures successfully but whose resulting image is compared against a deliberately different reference, and assert the capture's finish record shows capture outcome = success while a separately linked comparison record shows comparison outcome = mismatch.

## Done when

- Test passed_capture_with_mismatch_shows_distinct_capture_and_comparison_outcomes shows capture outcome = success and comparison outcome = mismatch as two distinct, non-overwriting fields for the same capture.

## Evidence to capture

- The test's two distinct outcome fields (capture vs. comparison) and confirmation neither was overwritten by the other's later-arriving event

## Do not change

- Do not conflate TaskStatus/ScenarioOutcome's existing capture-success/page-failure/infra-failure classification with progress::Event::Compared's separate passed/failed comparison outcome — they must remain two distinct fields in this task's records.
- Do not overwrite the original capture-stage error when a later cleanup failure also occurs; preserve causality per Capture Diagnostics' 'do not overwrite the original error.'

## Reopen triggers

- Task 7.1's concurrency suite finds a cancellation-mid-capture case this task's finish-stage record does not yet distinguish clearly from a normal failure.

## Context

This is the last of the four Phase 3 stages, covering the capture's own terminal state; the plan is explicit that capture failure, comparison mismatch, and infrastructure failure must stay three distinct, never-conflated concepts, mirroring how TaskStatus/ScenarioOutcome and progress::Event::Compared already keep them distinct today.

## Options & tradeoffs

Recording one merged 'outcome' field covering both capture and comparison would be simpler but would make it impossible to later distinguish 'capture succeeded, image differs' from 'capture itself failed' purely from the diagnostic record; two distinct fields (chosen) cost one more field but satisfy Task 3.4's own acceptance criterion directly.

## Decision

Record capture outcome (derived from TaskStatus/ScenarioOutcome) and comparison outcome (derived from progress::Event::Compared) as two separate, independently-set fields on related but distinct records, never merging them.

## Acceptance criteria

- A passed capture with a visual mismatch shows capture outcome = success and comparison outcome = mismatch as distinct fields in the resulting records.
- A cleanup failure that occurs after a capture failure does not overwrite the original capture-stage error in the resulting record.

## File touchpoints

- src/test/capture.rs
- src/test/test_suite.rs
- src/test/test_suites.rs (reference)
- src/progress.rs (reference)

## Tests

- cargo test --lib test::capture::tests -- passed_capture_with_mismatch_shows_distinct_capture_and_comparison_outcomes

## Dependencies

- capture-queue-setup-stage

## Risks

- If the finish-stage record is written before the later comparison completes (capture and compare are separate phases per progress.rs's Comparing/Compared events), a naive implementation might need to mutate the capture record retroactively; mitigate by keeping the two as separate linked records from the start rather than merging fields into one record after the fact.
