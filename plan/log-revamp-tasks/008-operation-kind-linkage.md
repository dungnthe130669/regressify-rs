<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 008 — Operation-kind linkage for captures and deployments

- Slug: `operation-kind-linkage`
- Implements plan section(s): `Phase 2 — Durable Operation History (Captures and Deployments)#0`, `Producers and Consumers#0`
- Depends on: operation-journal-writer

## Goal

Make the journal reader/writer operation-generic (parameterized by OperationKind::Capture or OperationKind::Deployment), with one shared test helper exercising both kinds through the same code path.

## Why this exists

This resolves the Delivery Phases dependency fix explicitly called out in the plan: Phase 4 (deployment instrumentation) depends on Phase 2, not only Phase 1, specifically because it reuses this same journal rather than building a second storage implementation.

## Settled investigation

- Task 2.3 does not add new DiagnosticRecordV1 fields; it proves the existing operation_kind: OperationKind and capture_run_id: Option<CaptureRunId> fields from Task 1.3 are sufficient for both kinds end-to-end.
- The plan is explicit the journal is 'already defined generically per operation, not only per capture' in Identity and Record Contract and Full-Log Persistence, and that Deployment Diagnostics 'reuse[s] the same operation-owned durable journal' — this task makes that reuse concrete in code, not just in the schema.

## Inputs

- Task 2.1's journal writer/reader
- Task 1.3's OperationKind enum

## Outputs

- The journal reader/writer accepts and round-trips records of both OperationKind::Capture and OperationKind::Deployment
- One shared test helper function usable by both this task's own tests and Task 4.1's deployment-side test

## Implementation steps

- Audit Task 2.1/2.2's writer/reader/finalize implementation for any capture-specific assumption (e.g. treating capture_run_id as required) and relax it to treat OperationKind as a first-class discriminant.
- Extract a shared test helper (e.g. exercise_journal_round_trip(kind: OperationKind, records: Vec<DiagnosticRecordV1>)) that writes, finalizes, and reads back records for a given operation kind, asserting ordering/completeness.
- Call that helper once with synthetic capture-kind records (capture_run_id: Some(...)) and once with synthetic deployment-kind records (capture_run_id: None), in the same test module.
- Confirm neither path requires desktop/gui features to compile or run.

## Done when

- One shared test helper function (e.g. exercise_journal_round_trip(kind: OperationKind, records: Vec<DiagnosticRecordV1>)) is called once with a synthetic capture-kind record set and once with a synthetic deployment-kind record set, in the same test module.
- Both invocations pass, each asserting capture_run_id.is_some() for the capture-kind call and capture_run_id.is_none() for the deployment-kind call.

## Evidence to capture

- The shared test helper's signature and both call sites (capture-kind and deployment-kind), for Task 4.1 to cite directly
- The exact assertion distinguishing the two kinds (capture_run_id presence/absence)

## Do not change

- Do not introduce a parallel, deployment-specific journal writer/reader — the explicit point of this task is proving one implementation suffices for both kinds.

## Reopen triggers

- Task 4.1's real deployment integration finds a field or behavior this synthetic test helper did not actually exercise (e.g. provider-specific metadata that does not fit the existing schema).

## Context

The plan's Delivery Phases table originally said deployment instrumentation could proceed after phase 1 alone; this was corrected to depend on phase 2 as well, specifically because deployment diagnostics reuse this same journal. This task is where that correction becomes a concrete, tested fact rather than only a planning statement.

## Options & tradeoffs

A capture-only journal with a deployment-specific journal bolted on later would let Phase 2 ship slightly faster in isolation, but directly violates 'do not add a second implementation' and would force Task 4.1 to redo storage work; proving operation-genericity now (chosen) costs one extra test but de-risks all of Phase 4.

## Decision

Prove operation-kind genericity with one shared test helper exercised against both kinds in Phase 2, before any Phase 4 task starts.

## Acceptance criteria

- A single test helper function, not two parallel ones, is called with both a capture-kind synthetic record set and a deployment-kind synthetic record set in the same test run.
- The capture-kind call asserts capture_run_id.is_some() and the deployment-kind call asserts capture_run_id.is_none(), both passing in the same test run.

## File touchpoints

- src/diagnostics/journal.rs (proposed)

## Tests

- cargo test --lib diagnostics::journal::tests -- operation_kind_linkage

## Dependencies

- operation-journal-writer

## Risks

- If the shared helper is written loosely enough to pass trivially for both kinds without really exercising kind-specific fields, the test could give false confidence; mitigate by asserting capture_run_id.is_some()/is_none() explicitly for each kind, not just overall round-trip success.
