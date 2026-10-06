<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 016 — Reuse the phase 2 journal for deployment-kind operations

- Slug: `deployment-journal-reuse`
- Implements plan section(s): `Phase 4 — Deployment Instrumentation#0`, `Deployment Diagnostics#0`
- Depends on: operation-kind-linkage

## Goal

Produce a mocked deployment run's journal via the same Task 2.1/2.3 reader/writer, verified by the Task 2.3 shared test helper, proving no second storage implementation exists for deployment-kind operations.

## Why this exists

Delivery Phases' corrected dependency table makes Phase 4 depend on Phase 2 specifically because deployment instrumentation must reuse the capture journal, not build its own; this task is the concrete proof point.

## Settled investigation

- Task 2.3 already produced one shared test helper exercising both OperationKind::Capture and OperationKind::Deployment synthetically; this task is the first real (non-synthetic-only) deployment caller of that same reader/writer.
- src/desktop/deployment.rs's existing phase(name: &str) helper (lines 810-813) and with_heartbeat/monitor (lines 815-826+) are the current eprintln+progress::emit call sites Task 1.1 classified; this task's journal integration point is the same functions, extended to also write a DiagnosticRecordV1.
- src/desktop/worker.rs::run_stdio is where a real deployment run begins today (request.run.command == RunCommand::Deploy branch, lines 45-77) and already installs the existing progress sink; this is the natural point to open/close the deployment-kind journal writer.

## Inputs

- Task 2.3's shared journal test helper and operation-kind-generic reader/writer
- src/desktop/deployment.rs's existing phase/with_heartbeat call sites (reference)

## Outputs

- A mocked deployment run that writes DiagnosticRecordV1s of OperationKind::Deployment through the Task 2.1 journal writer
- A test reusing Task 2.3's shared helper to verify the resulting journal

## Implementation steps

- Open a deployment-kind journal writer at the start of a deployment operation, mirroring where src/desktop/worker.rs::run_stdio currently installs the progress sink for a Deploy command, and finalize it (Task 2.2) at the operation's natural end.
- Route src/desktop/deployment.rs::phase()'s existing call site to also write a DiagnosticRecordV1 via this writer, instead of touching every provider-specific call site yet.
- Add mocked_deployment_produces_operation_kind_deployment_journal, reusing the existing mock-provider test infrastructure already present in swa.rs/netlify.rs/cloudflare.rs's own test modules.
- Reuse Task 2.3's shared test helper to verify the resulting journal's structural correctness rather than writing a second bespoke assertion.

## Done when

- Test mocked_deployment_produces_operation_kind_deployment_journal runs a mocked deployment and asserts the resulting journal is readable by Task 2.1's reader as OperationKind::Deployment.
- Task 2.3's shared exercise_journal_round_trip test helper, called against this real mocked-deployment journal, passes in the same test run.

## Evidence to capture

- The mocked deployment test's name (mocked_deployment_produces_operation_kind_deployment_journal) and confirmation it calls Task 2.3's shared helper rather than a new bespoke assertion

## Do not change

- Do not create a second journal writer/reader implementation specific to deployments — the entire point of this task is proving the Task 2.1/2.3 implementation already suffices.

## Reopen triggers

- Task 4.2/4.3's richer per-stage instrumentation finds the minimal integration this task adds (only around phase()) insufficient and needs a broader integration point.

## Context

This task is deliberately narrow: it proves the plumbing (open a deployment-kind writer, write through it, finalize it, verify with the shared helper) works before Task 4.2/4.3 add the full required-context fields per stage.

## Options & tradeoffs

n/a — the only implementation choice (reuse vs. new storage) was already settled in Shared Architecture: Producers and Consumers and proven generically in Task 2.3; this task has no remaining competing options, only integration work.

## Decision

Wire the deployment-kind journal writer around the existing src/desktop/deployment.rs::phase()/with_heartbeat call sites and src/desktop/worker.rs::run_stdio's Deploy branch, reusing Task 2.1/2.3's reader/writer unchanged.

## Acceptance criteria

- Test mocked_deployment_produces_operation_kind_deployment_journal produces a OperationKind::Deployment journal readable by Task 2.1's reader.
- The same test's journal passes Task 2.3's shared exercise_journal_round_trip helper unmodified.

## File touchpoints

- src/desktop/deployment.rs
- src/desktop/worker.rs
- src/diagnostics/journal.rs (reference)

## Tests

- cargo test --features desktop --lib desktop::deployment::tests -- mocked_deployment_produces_operation_kind_deployment_journal

## Dependencies

- operation-kind-linkage

## Risks

- Opening/closing the journal writer at the wrong point in run_stdio's control flow (e.g. before credentials are validated) could create a journal for an operation that never really started; mitigate by opening it at the same point the existing progress sink already gets installed for consistency.
