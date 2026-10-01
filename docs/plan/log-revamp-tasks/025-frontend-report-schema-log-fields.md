<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 025 — Extend report schema (Rust and frontend/src/data/report.ts)

- Slug: `frontend-report-schema-log-fields`
- Implements plan section(s): `Phase 6 — Frontend and Packaging#0`, `Snapshot Frontend and Publication#0`
- Depends on: define-diagnostic-record-v1, legacy-migration-behavior

## Goal

Add optional execution-log reference and availability/completeness metadata fields to both the Rust report Manifest/capture types and the TypeScript manifestSchema/captureSchema, additively under the manifest's existing, unchanged version: 2 literal, round-tripping all five of Task 2.4's ArchiveState completeness states — Complete, Partial, Missing, LegacyUnavailable, and LoadingError — through a fixture report, without breaking a pre-log-fields fixture.

## Why this exists

This is the single schema change every Phase 6 frontend/packaging task (6.2, 6.3, 6.4) depends on, and it directly tests the plan's own flagged Assumption that additive fields under the current version: 2 literal are sufficient (Risk R4's mitigation). Task 6.2 (frontend-log-viewer-component) specifically requires rendering four distinct non-complete messages — legacy, missing, partial, and loading-error — so this task's own round-trip proof must cover all five states, not only 'complete' and 'field absent'.

## Settled investigation

- src/snapshot/site.rs's Manifest struct (pub(super) struct Manifest { pub version: u8, pub title: String, ... }, line 69) is constructed with version: 2 at line 424 and read back with a strict manifest.version == 2 equality check at read_manifest (line 688, 'Unsupported snapshot manifest version' on mismatch).
- frontend/src/data/report.ts's manifestSchema (lines 53-62) already has version: z.literal(2); its captureSchema (lines 64-87) already has an optional, additively-added precedent field — previews (lines 76-83, .optional()) — showing this exact codebase's established pattern for adding an optional nested object without a version bump; the plain nullable error field (line 72) must not be overloaded.
- Neither the Rust Manifest struct nor its TypeScript manifestSchema/captureSchema use deny_unknown_fields/.strict() today, confirming both sides already tolerate an added-but-unrecognized field.
- frontend/src/data/report.ts::fetchReport<T>(url, schema, signal?, reload?) (lines 135-145) is the existing generic lazy-fetch-plus-zod-validate helper already used for report JSON; Task 6.2's log viewer is expected to reuse this same function for fetching the new log asset.
- Task 2.4 (legacy-migration-behavior) defines exactly five ArchiveState variants — Complete, Partial, Missing, LegacyUnavailable, LoadingError — as its own Done-when/Acceptance-criteria already require (its own dedicated tests distinguish all four non-complete variants from each other and from Complete); this task now depends directly on legacy-migration-behavior rather than only informally referencing it, so an implementer has a concrete, already-proven five-variant source enum to mirror instead of inventing variant names independently.

## Inputs

- Task 1.3's (define-diagnostic-record-v1) DiagnosticRecordV1 (as the source of truth for what availability/completeness metadata means)
- Task 2.4's (legacy-migration-behavior) 5-variant ArchiveState enum — Complete, Partial, Missing, LegacyUnavailable, LoadingError — as the exact metadata this schema's completeness field must express and distinguish
- The existing Manifest/captureSchema definitions (modified, not replaced)

## Outputs

- New optional execution-log-reference and availability/completeness fields on both the Rust Manifest/capture report types and the TypeScript manifestSchema/captureSchema
- A fixture report round-trip test proving backward compatibility (pre-log-fields fixture) and forward round-trip of all five distinct completeness states

## Implementation steps

- Add a new optional field (e.g. executionLog: Option<ExecutionLogRef>) to the Rust per-capture report type alongside the existing reference/test/diff/previews fields, modeling ExecutionLogRef with an asset path/version plus an explicit completeness enum matching all five of Task 2.4's ArchiveState variants by name (Complete, Partial, Missing, LegacyUnavailable, LoadingError).
- Mirror the same field as an optional nested object in frontend/src/data/report.ts's captureSchema, following the exact .optional() pattern already used by previews (lines 76-83), including a matching five-value completeness literal/enum.
- Do not change manifestSchema's version: z.literal(2) or the Rust Manifest's version: 2 constructor/== 2 check; the new fields must be valid additions under the existing literal.
- Construct five fixture reports, one per Task 2.4 ArchiveState variant (Complete, Partial, Missing, LegacyUnavailable, LoadingError), each with the executionLog field populated and a distinct completeness value, plus a sixth pre-log-fields fixture lacking the field entirely; round-trip all six through both the Rust struct and the TypeScript captureSchema in manifest_round_trips_optional_execution_log_fields, asserting field-for-field equality and distinctness of the five completeness values and a successful parse of the field-absent case.

## Done when

- Test manifest_round_trips_optional_execution_log_fields serializes and deserializes five Rust fixture reports, one per ArchiveState variant (Complete, Partial, Missing, LegacyUnavailable, LoadingError), each with the executionLog field populated, asserting field-for-field equality and a distinct completeness value per fixture, while Manifest.version remains 2 in every case.
- A second case in the same test deserializes a stored pre-log-fields fixture report (lacking executionLog entirely) and asserts it still parses successfully on both the Rust struct and the updated captureSchema, with version remaining 2 in both.
- The frontend bun run test suite parses all five completeness-state fixture values through captureSchema and asserts each produces its own distinct, recognized value — not a parse failure, not a silent default, and not collapsing two variants to the same TypeScript value.

## Evidence to capture

- All five fixture reports (one per ArchiveState variant) plus the pre-log-fields fixture, and manifest_round_trips_optional_execution_log_fields's output proving each of the five round-trips distinctly and the sixth (pre-log-fields) still parses
- Explicit confirmation (via git diff) that src/snapshot/site.rs line 688 and frontend/src/data/report.ts line 55 were not modified

## Do not change

- Do not modify src/snapshot/site.rs's manifest.version == 2 equality check (line 688) or the TypeScript version: z.literal(2) (report.ts line 55) — this task's whole point is proving the new field fits without touching either.
- Do not overload the existing error: z.string().nullable() field (report.ts line 72) to also carry log-availability information.
- Do not let two of the five completeness states serialize to the same TypeScript literal value; each of Complete/Partial/Missing/LegacyUnavailable/LoadingError must remain distinguishable end-to-end.

## Reopen triggers

- The round-trip fixture test in this task fails to express completeness/availability metadata cleanly under the additive-only constraint, which per the plan's own Assumptions would require 'a genuine version bump,' escalating this from a schema-addition task to a breaking-change proposal needing its own review.
- Task 6.2's (frontend-log-viewer-component) fixture-based browser test finds one of the four non-complete states does not actually render distinctly from another, indicating this task's round-trip test passed without truly proving end-to-end distinctness.

## Context

This task is the empirical test of the plan's own flagged Assumption: 'Execution-log fields can land as additive, fully optional fields under the report manifest's existing, already-hardcoded version: 2.' If this task's round-trip test cannot express the needed metadata cleanly for all five ArchiveState variants, the additive approach is falsified and a real schema bump would be required instead. Task 6.2 (frontend-log-viewer-component) depends on this task having already proven all five states are distinct, not just two.

## Options & tradeoffs

A version bump (e.g. to a literal 3) would give a clean slate for the new fields but requires relaxing the hard manifest.version == 2 equality check without breaking already-deployed reports and older desktop builds; an additive optional field under the existing version: 2 (chosen, matching the previews precedent) is far less disruptive and is the plan's explicitly preferred default, contingent on this task's own five-state round-trip test succeeding.

## Decision

Add the execution-log reference and completeness metadata, covering all five Task 2.4 ArchiveState variants, as new optional fields under the unchanged version: 2 literal, following the existing previews field's additive pattern on both the Rust and TypeScript sides, and depend directly on legacy-migration-behavior for the authoritative variant set.

## Acceptance criteria

- Test manifest_round_trips_optional_execution_log_fields round-trips five fixture reports — one for each of Complete, Partial, Missing, LegacyUnavailable, and LoadingError — through the Rust generator and validates each successfully against the updated captureSchema, with each fixture's completeness value distinct from the other four.
- The same test's pre-existing fixture report (captured before this change, lacking the new field entirely) still validates successfully against both the updated Rust struct's deserialization and the updated captureSchema, with the manifest's version literal unchanged at 2 in both cases.
- A TypeScript-side test in frontend/src/data/report.test.ts parses all five completeness-state values and asserts each is recognized as its own distinct enum value by captureSchema, matching the Rust enum's five serialized variant names exactly.

## File touchpoints

- src/snapshot/site.rs
- frontend/src/data/report.ts
- frontend/src/data/report.test.ts

## Tests

- cargo test --features desktop --lib snapshot::site::tests -- manifest_round_trips_optional_execution_log_fields
- bun run test (frontend/src/data/report.test.ts, new cases covering all five completeness states — complete, partial, missing, legacyUnavailable, loadingError — plus backward compatibility with a pre-log-fields fixture)

## Dependencies

- define-diagnostic-record-v1
- legacy-migration-behavior

## Risks

- Modeling the completeness-state enum inconsistently between Rust (ArchiveState from Task 2.4/legacy-migration-behavior) and TypeScript (a separate hand-written literal union) could let the two drift, especially now that all five variants (not just two) must stay in lockstep; mitigate by deriving the TypeScript literal union's variant names directly from the Rust enum's serialized camelCase representation and asserting the match for all five in the round-trip test, not just a sampled subset.
- This task now depends on legacy-migration-behavior's ArchiveState shape; if that enum's variant names or count change before this task starts, this task's completeness enum must be updated to match — mitigate by re-reading Task 2.4's final Done-when/output before implementing this task's enum, rather than relying on the variant names recorded here.
