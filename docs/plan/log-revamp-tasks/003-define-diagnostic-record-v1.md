<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 003 — Define the versioned diagnostic record and its enums

- Slug: `define-diagnostic-record-v1`
- Implements plan section(s): `Phase 1 — Contract and Foundations#0`, `Diagnostic record type (proposed)#0`, `Identity and Record Contract#0`
- Depends on: add-tracing-dependencies

## Goal

Implement DiagnosticRecordV1 and its OperationKind/Stage/Severity/FieldValue enums as a new, independently-versioned schema, decoupled from progress.rs's Record/Event wire format and from the snapshot report's Manifest.version.

## Why this exists

Every later Phase 1-6 task (the adapter, the journal writer, every instrumentation stage, the frontend schema) serializes or deserializes this type; it must exist and round-trip before any of them can build on it.

## Settled investigation

- progress.rs already defines an unrelated `pub struct Record { pub version: u8, pub elapsed_ms: u64, pub event: Event }` at schema VERSION: u8 = 1 (src/progress.rs lines 15, 125-129) — DiagnosticRecordV1's own schema_version must be a distinct, independently-evolving field, not reuse or collide with this constant.
- The Interfaces section's Rust sketch for DiagnosticRecordV1 is an explicit starting proposal, not a frozen contract — field names/types may be adjusted, but `fields` must stay a typed, allowlisted BTreeMap<String, FieldValue> rather than Debug-formatted free text.
- `error_cause: Option<SanitizedError>` must be constructible by wrapping/extending src/desktop/deployment/diagnostics.rs's existing sanitize()/failure() helpers for deployment-sourced errors, not a second redaction implementation; capture-sourced errors need their own sanitization path defined in this task.
- Module ownership (new src/diagnostics/ vs. extending src/progress.rs) is an explicitly open decision per Interfaces; this task's acceptance criterion only requires the new module to compile and round-trip, not a specific location.

## Inputs

- [Verified output and safe-field inventory](../../logging-audit.md).
- Task 1.2's Cargo.toml with tracing/tracing-subscriber available
- The Interfaces section's DiagnosticRecordV1 sketch
- Task 1.1's inventory for the safe-field allowlist

## Outputs

- A new Rust module defining DiagnosticRecordV1, OperationKind, Stage, Severity, FieldValue, and identity newtypes (OperationId, CaptureRunId, ExecutionId, SpanId) with Serialize/Deserialize
- A unit test constructing a synthetic record and round-tripping its (de)serialization

## Implementation steps

- Decide module ownership (new src/diagnostics/ module tree, or an addition inside src/progress.rs) and record the decision in the module's doc comment.
- Define OperationId, CaptureRunId, ExecutionId, SpanId as distinct newtypes, not raw String/usize, so they cannot be confused with the process-local usize job ID progress::Job::id uses today.
- Define OperationKind{Capture,Deployment}, CaptureRole{Reference,Test}, Severity{Trace,Debug,Info,Warn,Error}, Stage (one variant per row of the Capture Diagnostics and Deployment Diagnostics stage tables), and FieldValue as a small closed enum rather than an open serde_json::Value.
- Define DiagnosticRecordV1 with schema_version: u16 = 1 and the identity/sequence/timestamp/elapsed/severity/stage/event_name/message/fields/parent_span/duration/outcome/error_cause fields, deriving Serialize/Deserialize.
- Define a minimal SanitizedError type and a constructor wrapping src/desktop/deployment/diagnostics.rs's sanitize() for deployment error strings, plus a parallel safe path for capture-sourced error strings using the same sanitize() regexes.
- Write a unit test that constructs one DiagnosticRecordV1 for a synthetic capture event, serializes it to JSON, deserializes it back, and asserts field-for-field equality.

## Done when

- The new module compiles under default features (no desktop/gui required).
- A unit test constructs a synthetic DiagnosticRecordV1 and round-trips its JSON (de)serialization with equal field values before and after.

## Evidence to capture

- The round-trip unit test's name and cargo test output
- The chosen module path, recorded for Task 1.4/2.1 to depend on

## Do not change

- Do not modify progress.rs's existing Record/Event/VERSION — they remain the live wire format for lifecycle events, and Task 1.4 owns the one-way adapter between the two.
- Do not re-implement src/desktop/deployment/diagnostics.rs's sanitize()/failure() logic; wrap or extend it.

## Reopen triggers

- Task 2.1's journal writer needs a field this record type does not yet expose (e.g. an explicit byte-offset/chunk marker).
- Task 6.1's frontend schema round-trip needs a field renamed or retyped for clean JSON/TypeScript interop.

## Context

This is the foundational schema every durable diagnostic, in both capture and deployment operations, will be serialized as. The plan is explicit that fields must stay a typed, allowlisted map — never Debug-formatted free text — because this type is what eventually gets published in snapshot reports.

## Options & tradeoffs

An open serde_json::Value for fields would be faster to write but defeats the allowlisting goal in Privacy and Safety and Logging Mechanism: Options Considered, which both reject arbitrary Debug-formatted dumps; a small closed FieldValue enum is chosen instead, at the cost of needing to extend the enum when a genuinely new safe field type is needed.

## Decision

Implement DiagnosticRecordV1 with a closed FieldValue enum and dedicated identity newtypes, as sketched in Interfaces, choosing module ownership and documenting the choice rather than leaving it implicit.

## Acceptance criteria

- The new module compiles standalone under cargo build (default features).
- A unit test (e.g. diagnostic_record_v1_round_trips_through_json) constructs a record with a non-trivial fields map and error_cause, serializes and deserializes it, and asserts equality.

## File touchpoints

- src/diagnostics/record.rs (proposed)
- src/progress.rs (reference)
- src/desktop/deployment/diagnostics.rs (reference)

## Tests

- cargo test --lib diagnostics::record::tests (proposed, exact path depends on the module ownership decision made in this task)

## Dependencies

- add-tracing-dependencies

## Risks

- Picking field types/names now that later prove wrong for the journal (Task 2.1) or frontend (Task 6.1) would force a schema_version bump later; mitigate by keeping the enum additive-friendly and treating this as schema_version 1 that can still evolve additively before any public report ships it.
