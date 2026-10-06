<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 004 — Implement the progress-protocol adapter

- Slug: `implement-progress-adapter`
- Implements plan section(s): `Phase 1 — Contract and Foundations#0`, `Adapter boundary (proposed)#0`
- Depends on: define-diagnostic-record-v1

## Goal

Build the one-way ProgressAdapter (or equivalent function) that converts a DiagnosticRecordV1 into existing progress::Event values, so the CLI renderer and desktop live view keep working unmodified while migration proceeds.

## Why this exists

Phase 1's exit criterion requires CLI and worker to 'emit equivalent attributed records without duplicate output or changed result semantics' — this adapter is what makes the new tracing layer additive rather than a breaking replacement of progress.rs consumers.

## Settled investigation

- progress::Event (src/progress.rs lines 76-121) has exactly 11 variants today (Phase, SuiteQueued, Queued, Started, Log, Captured, Comparing, Compared, SuiteFinished, Deployed, Finished); progress::emit(event: Event) (lines 172-180) is the sole public entry point consumers already call.
- progress::LiveProgress::apply(&mut self, record: Record) (lines 454-614) is what both the CLI renderer and desktop live view ultimately consume; this task must not change its signature or behavior.
- The Interfaces section's sketch trait is `pub trait ProgressAdapter { fn on_diagnostic(&self, record: &DiagnosticRecordV1); }`, explicitly one-way: existing progress.rs event types are the output, never the input, of this trait.

## Inputs

- [Verified output and event inventory](../../logging-audit.md).
- Task 1.3's DiagnosticRecordV1 type
- src/progress.rs's existing Event/emit API (read-only)

## Outputs

- A new adapter function/trait implementation that, given a DiagnosticRecordV1, calls progress::emit with an equivalent existing Event when one applies
- A passing cargo test --lib progress::tests run (unchanged)
- A new test proving the CLI renderer and desktop live view receive the same typed events as before when the tracing layer is installed

## Implementation steps

- Confirm the exact trait shape (or conclude a plain function suffices) against the real progress::Event enum.
- For each DiagnosticRecordV1 stage/event this task's scope covers, implement the mapping to the matching progress::Event variant, using the stable identity fields from Task 1.3 to recover the process-local usize job id Event variants still expect.
- Where no existing Event variant applies, the adapter must emit nothing on the progress channel — it is one-way and additive, not a replacement enum.
- Add a test harness that installs both the legacy direct progress::emit call path and the new adapter path side by side against a synthetic scenario run, and asserts the sequence of Events received by a test LiveProgress is identical either way.
- Run cargo test --lib progress::tests unchanged to confirm no regression.

## Done when

- cargo test --lib progress::tests passes unchanged.
- A new test proves the CLI renderer/desktop live view receive the same typed Event sequence whether events arrive through the legacy direct progress::emit calls or through the new adapter fed by DiagnosticRecordV1s.

## Evidence to capture

- The new test's name and output
- A short note on the final trait-vs-function shape decision for Task 1.5 to wire up

## Do not change

- Do not alter progress::Event's variants, progress::LiveProgress::apply's matching logic, or any existing progress.rs test.
- Do not make progress.rs depend on the new diagnostics module — the dependency direction is adapter-depends-on-progress, never the reverse.

## Reopen triggers

- A later Phase 3/4 instrumentation task needs a progress::Event this adapter does not yet produce for an existing lifecycle transition.
- Task 1.5's entry-point wiring finds the trait shape awkward to install at both main.rs and worker.rs, motivating a plain-function alternative.

## Context

Existing CLI and desktop consumers read progress::Events through LiveProgress::apply; the plan requires zero behavior change for them during migration. This adapter is the seam that lets the new tracing-based pipeline coexist with — not replace — that existing, working protocol.

## Options & tradeoffs

A trait (ProgressAdapter) gives a swappable implementation point (e.g. for tests); a plain function is simpler if only one implementation will ever exist. The Interfaces section explicitly defers this choice to this task; the trait is chosen here for testability, but a plain function remains an equally valid closing decision if the trait proves to add no value once implemented.

## Decision

Implement ProgressAdapter as a trait with one production implementation, so Task 1.5's install step and this task's own test harness can both construct and swap it without a global function pointer.

## Acceptance criteria

- cargo test --lib progress::tests passes with no modifications to existing test assertions.
- A new test demonstrates that a synthetic sequence of DiagnosticRecordV1s fed through the adapter produces the identical Event sequence (verified via LiveProgress's resulting scenarios/logs state) as the equivalent direct progress::emit calls.

## File touchpoints

- src/diagnostics/adapter.rs (proposed)
- src/progress.rs (reference)

## Tests

- cargo test --lib progress::tests
- cargo test --lib diagnostics::adapter::tests (proposed, new)

## Dependencies

- define-diagnostic-record-v1

## Risks

- Recovering a stable usize job id from the new identity types for existing Event variants could reintroduce the job-ID-as-identity anti-pattern Identity and Record Contract warns against; mitigate by keeping the usize mapping entirely inside the adapter and never exposing it as the diagnostic record's own durable identity.
