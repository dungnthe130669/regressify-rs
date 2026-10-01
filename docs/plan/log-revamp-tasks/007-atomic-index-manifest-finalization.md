<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 007 — Atomic index/manifest finalization

- Slug: `atomic-index-manifest-finalization`
- Implements plan section(s): `Phase 2 — Durable Operation History (Captures and Deployments)#0`, `Full-Log Persistence#0`
- Depends on: operation-journal-writer

## Goal

Finalize a journal's index/manifest atomically so a reader never observes a partially-written complete archive, and an interrupted writer leaves an explicitly partial/interrupted archive instead of a silently truncated one marked complete.

## Why this exists

Phase 2's exit criterion requires 'interrupted/write-failed archives are explicit'; Definition of Done item 5 requires legacy/missing logs to show an explicit state, never a fabricated complete log.

## Settled investigation

- Task 2.1's journal writer provides the underlying segmented storage this task finalizes; this task adds the atomicity layer on top, not a new storage format.
- The plan requires finalization to happen 'before automatic comparison or snapshot generation consumes its logs' — ordered relative to existing compare/snapshot entry points such as src/compare/report_gen.rs's comparison pipeline and src/snapshot/site.rs's generation pipeline, not merely 'eventually.'
- This codebase already uses atomic-replace patterns elsewhere (e.g. src/helpers/io.rs's write_file_atomically/copy_file_atomically, referenced from src/test/test_suites.rs's imports) as a precedent for 'write to a temp path, then atomically rename' on this platform.

## Inputs

- Task 2.1's journal writer/reader
- src/helpers/io.rs's existing atomic-write helpers as a precedent (reference only)

## Outputs

- A finalize function that atomically writes/renames an index or manifest file marking the journal complete
- An explicit partial/interrupted marker state left behind when finalize never runs

## Implementation steps

- Define an index/manifest sidecar file format (small JSON) recording segment list, record count, and a completion flag.
- Implement finalize() to write this sidecar to a temporary path and atomically rename it into place only once every segment's data is confirmed flushed, reusing the existing atomic-rename pattern already used elsewhere in this codebase (e.g. src/helpers/io.rs) rather than inventing a new one.
- Ensure that if the process exits/crashes before finalize() runs, the reader detects 'segments exist but no completed index' and reports that as partial/interrupted, never silently treating missing finalization as 'no log.'
- Write finalize_marks_interrupted_write_as_partial_not_complete: write 500 of 1,000 synthetic records, drop the writer before calling finalize(), and assert the reader returns Partial.
- Write a paired normal-completion test in the same module: write all 1,000 records, call finalize(), and assert the reader returns Complete.

## Done when

- Test finalize_marks_interrupted_write_as_partial_not_complete drops the writer after 500 of 1,000 synthetic records (before finalize() runs) and asserts the reader returns ArchiveState::Partial, never ArchiveState::Complete.
- A paired normal-completion test in the same module writes all 1,000 records, calls finalize(), and asserts the reader returns ArchiveState::Complete.

## Evidence to capture

- The exact interruption point ('500 of 1,000 records written, writer dropped before finalize()') and the resulting Partial state returned by the reader in finalize_marks_interrupted_write_as_partial_not_complete
- The normal-completion test's resulting Complete state, for direct comparison against the interrupted case

## Do not change

- Do not let finalize() silently succeed when a segment write is known to have failed; the whole point of this task is that failure must be visible, not hidden by finalization.

## Reopen triggers

- Task 2.6's writer-health signal needs the partial/interrupted marker to carry additional detail (e.g. a missing-byte-range count) beyond a boolean.

## Context

A capture or deployment process can be killed, crash, or have its disk fill mid-write; without an atomic, explicit finalization step, a reader has no way to distinguish 'this log is complete' from 'this log stopped partway through' — and the plan's Definition of Done explicitly forbids ever presenting the latter as the former.

## Options & tradeoffs

Marking completion via the last record's own content (e.g. a sentinel Finish event) is simpler but is not atomic against a crash between writing that record and it being durably flushed/discoverable; a separate atomically-renamed index/manifest sidecar (chosen) adds one more file per operation but gives a true all-or-nothing completion signal independent of in-segment content.

## Decision

Use a separate, atomically-renamed index/manifest sidecar file as the sole source of truth for 'this journal is complete,' independent of any in-segment sentinel record.

## Acceptance criteria

- A test interrupts the writer after a known partial record count (e.g. 500 of 1,000 synthetic records) before finalize() runs, and the reader reports an explicit Partial/Interrupted state.
- A paired test that completes normally through finalize() with all 1,000 records reports an explicit Complete state, distinguishable from the interrupted case by direct enum comparison.

## File touchpoints

- src/diagnostics/journal.rs (proposed)
- src/helpers/io.rs (reference)

## Tests

- cargo test --lib diagnostics::journal::tests -- finalize_marks_interrupted_write_as_partial_not_complete

## Dependencies

- operation-journal-writer

## Risks

- An atomic rename is platform-dependent in its exact guarantees on Windows (the project's primary OS per environment context); mitigate by using the same rename/replace primitive already relied on elsewhere in this codebase (src/helpers/io.rs) rather than a new one, since that primitive's Windows behavior is already exercised by existing tests.
