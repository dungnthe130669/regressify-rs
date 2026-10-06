<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 012 — Queue/setup stage capture instrumentation

- Slug: `capture-queue-setup-stage`
- Implements plan section(s): `Phase 3 — Capture Instrumentation#0`, `Capture Diagnostics#0`
- Depends on: install-subscriber-entry-points, operation-kind-linkage, capture-journal-lifecycle-wiring

## Goal

Instrument the queue/setup stage of capture execution to record IDs, viewport, effective non-secret timeout/retry settings, queue wait, browser/page setup, and worker/runtime version, for both successful and timed-out captures.

## Why this exists

Capture Diagnostics' required-context table mandates this as the first instrumented stage; Phase 3's exit criterion requires synthetic success and targeted failure cases to identify the failing stage and cause.

## Settled investigation

- src/test/test_suites.rs::spawn_capture_jobs (lines 237-309) wraps each CaptureJob task body in tokio::spawn(progress::scope_scenario(index, async move {...})) (line 266), establishing the task-local SCENARIO_JOB id that scenario_log!/progress::emit rely on today.
- A semaphore-closed failure already emits ProgressEvent::Captured{id, error: Some("Capture semaphore closed".into()), infrastructure: true} (test_suites.rs lines 270-274) via an eprintln! at line 269.
- src/test/browsers.rs::browser_status (lines 82-87) already calls crate::progress::scenario_log! for browser/page setup status; browsers.rs and browsers/install.rs contain multiple further scenario_log! calls describing cache/executable status (e.g. install.rs lines 70-81, 285).
- ActionabilityConfig's defaults (src/test/actionability.rs lines 28-36: element_timeout_ms=30_000, actionability_timeout_ms=5_000, poll_interval_ms=100) are the existing effective timeout/retry settings this stage must record.
- 'worker/runtime version' maps to the existing CARGO_PKG_VERSION/BINARY_GIT_HASH constants already used for --version output (src/application.rs lines 42-47; src/cli/root.rs's LONG_VERSION, lines 9-14).
- Task 1.4's ProgressAdapter converts DiagnosticRecordV1 -> progress::Event for the OLD pipeline (one-way); it is not involved in getting this task's own tracing events into the new journal. Task 2.7 (capture-journal-lifecycle-wiring) defines the separate journal-writing layer and opens/registers the per-job journal writer this task's tracing calls are routed through; this task only emits tracing calls into an already-open writer, it does not open or finalize one.

## Inputs

- Task 1.5's installed subscriber
- Task 2.3's operation-kind-generic journal
- Task 1.1's audit classification for the queue/setup call sites in test_suites.rs/browsers.rs
- Task 2.7's (capture-journal-lifecycle-wiring) per-job journal writer and journal-routing layer — this task writes its records into that already-open writer; it does not itself open, register, or finalize the journal

## Outputs

- Queue/setup diagnostic records (start, duration/outcome, queue wait, timeout/retry settings, worker/runtime version) for every capture job, success or failure

## Implementation steps

- Add tracing spans/events around spawn_capture_jobs's per-job async block (test_suites.rs lines 266+), recording IDs, viewport, and the acquire_capture_permits wait duration as the queue-wait field.
- Replace the semaphore-closed eprintln! (line 269) with a tracing::error! event carrying the same message; Task 2.7's (capture-journal-lifecycle-wiring) journal-writing layer routes this event into the per-job journal already open for this execution — not Task 1.4's adapter, which only converts DiagnosticRecordV1 into progress::Event for the old pipeline and plays no part in reaching the new journal. The existing Captured{error, infrastructure:true} lifecycle event (lines 270-274) is a separate, pre-existing progress::emit call site, unaffected by this change.
- Add tracing spans/events around src/test/browsers.rs::prepare_browser/browser_status recording browser/page setup duration and outcome, converting its existing scenario_log! calls per Task 1.1's classification.
- Record ActionabilityConfig's element_timeout_ms/actionability_timeout_ms/poll_interval_ms (src/test/actionability.rs lines 28-36) and the resolved scenario_timeout_ms as the stage's non-secret timeout/retry fields.
- Record CARGO_PKG_VERSION/BINARY_GIT_HASH as the worker/runtime version field.
- Write queue_setup_stage_records_for_passed_and_timed_out_captures: run one synthetic passing capture and one synthetic timed-out capture, and assert both produce a queue/setup record with IDs, viewport, timeout/retry settings, queue wait, and worker/runtime version populated.

## Done when

- Test queue_setup_stage_records_for_passed_and_timed_out_captures produces a queue/setup record with IDs, viewport, timeout/retry settings, queue wait duration, and worker/runtime version for a synthetic passing capture.
- The same test produces an equivalent queue/setup record for a synthetic timed-out capture, with its outcome field distinguishing it from the passing case.

## Evidence to capture

- Both synthetic captures' resulting queue/setup records from queue_setup_stage_records_for_passed_and_timed_out_captures, showing the populated field set and the pass/timeout outcome distinction

## Do not change

- Do not alter spawn_capture_jobs's existing concurrency-permit acquisition logic (acquire_capture_permits) or its Captured{error, infrastructure} lifecycle event semantics; only add diagnostic recording around them.
- Do not record secret-shaped configuration even though this stage records 'effective non-secret timeout/retry settings' — only the numeric timeout/retry fields are in scope.

## Reopen triggers

- Task 3.2's navigation-stage work finds the queue/setup stage's recorded identity fields insufficient to correlate with the subsequent navigation record.
- Task 2.7's (capture-journal-lifecycle-wiring) per-job registration/lookup mechanism changes shape, requiring this task's tracing calls to adapt to a new routing API.

## Context

This is the first of four Phase 3 capture-instrumentation tasks, covering the stage before any browser navigation occurs; it is also where the task-local SCENARIO_JOB scoping that every later scenario_log! call already relies on must be bridged to the new diagnostic pipeline.

## Options & tradeoffs

Instrumenting only at spawn_capture_jobs's outer boundary (chosen) keeps this task's scope matched to the 'Queue and setup' row of Capture Diagnostics' table; instrumenting deeper inside browsers.rs's individual cache/install steps as well would add more granularity but risks scope creep into Task 3.2's navigation stage.

## Decision

Instrument at spawn_capture_jobs's per-job boundary and browsers.rs's prepare_browser/browser_status call sites, converting their existing eprintln!/scenario_log! calls per Task 1.1's classification rather than adding a parallel instrumentation path.

## Acceptance criteria

- Test queue_setup_stage_records_for_passed_and_timed_out_captures shows a queue/setup record with IDs, viewport, effective timeout/retry settings, queue wait, and worker/runtime version for a passing synthetic capture.
- The same test shows an equivalent record for a timed-out synthetic capture, with a distinguishing outcome field.

## File touchpoints

- src/test/test_suites.rs
- src/test/browsers.rs
- src/test/browsers/install.rs
- src/test/actionability.rs (reference)
- src/application.rs (reference)

## Tests

- cargo test --lib test::test_suites::tests -- queue_setup_stage_records_for_passed_and_timed_out_captures

## Dependencies

- install-subscriber-entry-points
- operation-kind-linkage
- capture-journal-lifecycle-wiring

## Risks

- Converting the semaphore-closed eprintln! (test_suites.rs line 269) to a tracing call without preserving its existing colored() terminal formatting could regress the CLI's visible error coloring for this specific failure; mitigate by keeping the console-layer output visually equivalent and covering it in the test's assertions.
