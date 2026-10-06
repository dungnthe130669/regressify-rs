<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 009 — Legacy/migration behavior and archive-state detection

- Slug: `legacy-migration-behavior`
- Implements plan section(s): `Phase 2 — Durable Operation History (Captures and Deployments)#0`, `Desktop and CLI Experience#0`
- Depends on: atomic-index-manifest-finalization

## Goal

Make loading a pre-revamp report or saved desktop run produce one of five explicit, mutually-exclusive archive states — Complete, Partial, Missing, LegacyUnavailable, or LoadingError — covering all four non-complete states Desktop and CLI Experience names ('legacy, partial, missing, and loading-error'), never an error or a fabricated empty-but-complete log.

## Why this exists

Definition of Done item 5 requires legacy reports and previously saved desktop Activity to remain readable with an explicit legacy/log-not-available state; Desktop and CLI Experience requires legacy, partial, missing, and loading-error to each be shown as explicit, distinguishable states; Task 5.2, Task 6.1/6.2, and Task 7.1's 'missing/corrupt log assets' Verification Strategy scenario all depend on this complete state model existing first, not a 3-state subset of it.

## Settled investigation

- Today, src/desktop/runner.rs's RunManager persists LiveProgress (bounded live history only) via its persistence background save, and src/desktop/ui/runs/log.rs's RunPanel::refresh_log reads only self.manager.progress.logs/self.manager.logs — neither path has any concept of a durable archive today, so 'no archive found' is the universal case for every report/run that exists before this plan ships.
- The desktop-progress-events skill documents that 'Saved activity version 2 preserves optional report overrides. Loading version 1 converts only exact prefilled title/heading/subheading defaults to unset values' — i.e. this codebase already has a working precedent for versioned-load-with-explicit-legacy-conversion that this task's archive-presence check should follow structurally.
- Desktop and CLI Experience requires: 'Show explicit legacy, partial, missing, and loading-error states; never invent a successful historical log.' Full-Log Persistence separately requires: 'Mark archive state explicitly: complete, partial/interrupted, unavailable for legacy captures, or intentionally excluded local-only diagnostics' and 'Missing expected logs must also be detectable on later load.' Verification Strategy separately lists 'missing/corrupt log assets' as its own scenario, distinct from 'legacy schemas.'
- LegacyUnavailable and Missing are distinguished by the containing record's own format, not by archive absence alone: a containing report/saved-run format that predates any diagnostics-archive concept at all is LegacyUnavailable; a containing record already in a diagnostics-aware format (the same kind of version marker the version-2-vs-version-1 saved-activity precedent above already uses) whose referenced archive path still has no files — whether deleted, never started, or suppressed by Task 2.6's kill-switch — is Missing instead.
- LoadingError is distinct from Partial: Partial means Task 2.2's finalize step never completed (segments exist, no completed index, or the index itself is marked partial); LoadingError means a finalized index and/or its segment files are present and claim a usable state, but a later read of their content fails (parse/checksum/IO error) for a reason inconsistent with an ordinary interrupted write, e.g. post-finalization disk corruption or truncation by an unrelated process.

## Inputs

- Task 2.2's finalize/partial-state reader
- The existing RunManager/report-loading code paths (read-only in this task; Task 5.2/6.1 own the UI-visible wiring)
- A containing-record archive-awareness signal (a version/flag on the surrounding report or saved-run format, following the existing saved-activity-version precedent) that this task consumes as an input to distinguish LegacyUnavailable from Missing, but does not itself own or define the format of

## Outputs

- A reader-side function/state value that reports exactly one of: Complete, Partial, Missing, LegacyUnavailable, or LoadingError, for any given operation/report identity — covering all four non-complete states Desktop and CLI Experience names plus the success case

## Implementation steps

- Define an explicit enum ArchiveState{Complete, Partial, Missing, LegacyUnavailable, LoadingError} returned by the journal-presence check, rather than overloading Option<Journal>/Result<Journal,Error> in a way that conflates 'predates the feature', 'expected but absent', and 'an actual read error occurred.'
- Implement the presence check to accept the containing record's own archive-awareness signal alongside the per-operation archive path: when that signal says the containing format predates archive-awareness, return LegacyUnavailable regardless of what is on disk at the archive path.
- When the containing format is archive-aware but the archive path has zero files, return Missing, distinct from LegacyUnavailable.
- When the archive path has segments but no finalized index (or a finalize marker explicitly recorded as partial), return Partial, per Task 2.2 unchanged.
- When the archive path has a finalized index but reading/parsing a referenced segment fails (I/O error, checksum mismatch, or malformed content inconsistent with a normal interrupted-write truncation), return LoadingError rather than propagating the error or silently treating it as Complete.
- Write legacy_report_without_archive_is_explicit_not_an_error: load a directory lacking any archive, with the containing-format signal set to pre-archive-aware, and assert the result is exactly ArchiveState::LegacyUnavailable, not an Err.
- Write a second test that loads a directory with a Task 2.2 partial/interrupted marker and asserts it is ArchiveState::Partial, distinct from LegacyUnavailable.
- Write archive_missing_when_expected_is_distinct_from_legacy: construct two otherwise-identical empty-archive-path fixtures that differ only in the containing-format signal (one pre-archive-aware, one archive-aware), and assert the first returns LegacyUnavailable while the second returns Missing.
- Write loading_error_when_segment_corrupt_despite_finalized_index: finalize a valid Complete index over real segments, then corrupt one segment's bytes in place (not merely truncate its tail, which would look like an ordinary interrupted write) and assert the reader returns ArchiveState::LoadingError, not Complete or Partial.

## Done when

- Test legacy_report_without_archive_is_explicit_not_an_error loads a directory containing zero archive files with a pre-archive-aware containing-format signal and asserts the result equals ArchiveState::LegacyUnavailable, not an Err.
- A paired test loads a directory containing only a Task 2.2 partial-marker index (no completed index) and asserts the result equals ArchiveState::Partial, distinct from LegacyUnavailable by direct enum comparison.
- Test archive_missing_when_expected_is_distinct_from_legacy shows two otherwise-identical empty-archive fixtures, differing only in the containing-format signal, resolving to ArchiveState::LegacyUnavailable and ArchiveState::Missing respectively, confirmed unequal by direct comparison.
- Test loading_error_when_segment_corrupt_despite_finalized_index shows a finalized-but-subsequently-corrupted segment resolving to ArchiveState::LoadingError, distinct from both Complete and Partial by direct comparison.

## Evidence to capture

- Both original tests' fixture directory contents (zero files vs. a partial-marker index only) and their asserted resulting ArchiveState values
- archive_missing_when_expected_is_distinct_from_legacy's two fixture signals (pre-archive-aware vs. archive-aware) and their resulting LegacyUnavailable/Missing values
- loading_error_when_segment_corrupt_despite_finalized_index's exact corruption method (in-place byte corruption, not truncation) and the resulting LoadingError value

## Do not change

- Do not treat 'archive directory missing' and 'archive directory present but empty/corrupt' as the same state — they have different implications for the user and Desktop and CLI Experience requires distinguishing them.
- Do not conflate Missing (archive-aware format, no files found) with LegacyUnavailable (pre-archive-aware format) even though both currently present as 'nothing on disk at the archive path' — the distinguishing signal is the containing record's format, not the archive path alone.

## Reopen triggers

- A later task (e.g. the desktop saved-activity version bump or Task 6.1's report schema) finds the archive-awareness signal this task assumes does not yet exist in the shape expected, requiring this task's presence-check input to change.
- Verification Strategy or a later audit finds a sixth distinguishable state is needed beyond these five.

## Context

Every report and saved run that exists in the wild today was created before this plan's archive exists at all; this task defines the single source of truth for 'is there a durable log here, and if not, why not' that both the desktop (Task 5.2) and web frontend (Task 6.1/6.2) will render from, so they cannot independently invent inconsistent legacy-detection logic. Desktop and CLI Experience names four distinguishable non-complete states (legacy, partial, missing, loading-error); this task's ArchiveState enum must express all four, not a subset of two.

## Options & tradeoffs

Conflating 'missing' and 'corrupt/partial' into one generic 'unavailable' state would be simpler to implement but violates Desktop and CLI Experience's explicit requirement to distinguish legacy/partial/missing/loading-error; a dedicated 5-variant enum (chosen) is more plumbing but satisfies that requirement directly and gives Task 5.2/6.1 a single typed value to match on for every one of the four named states, not just two of them.

## Decision

Return a dedicated, exhaustive 5-variant archive-state enum (Complete, Partial, Missing, LegacyUnavailable, LoadingError) from the presence/load check, distinguishing LegacyUnavailable from Missing via the containing record's own archive-awareness signal, and distinguishing LoadingError from Partial via whether a finalized index's referenced content actually parses/reads cleanly.

## Acceptance criteria

- Test legacy_report_without_archive_is_explicit_not_an_error loading a directory with zero archive files and a pre-archive-aware containing-format signal returns ArchiveState::LegacyUnavailable, not an error.
- A paired test loading a directory with a Task 2.2 partial-marker archive returns ArchiveState::Partial, verified by asserting the two test outcomes are not equal to each other.
- Test archive_missing_when_expected_is_distinct_from_legacy shows an archive-aware fixture with zero archive files returns ArchiveState::Missing, distinct from the LegacyUnavailable result of an otherwise-identical pre-archive-aware fixture.
- Test loading_error_when_segment_corrupt_despite_finalized_index shows a finalized index whose segment is subsequently corrupted in place returns ArchiveState::LoadingError, distinct from Complete and Partial.

## File touchpoints

- src/diagnostics/journal.rs (proposed)
- src/desktop/runner.rs (reference)

## Tests

- cargo test --lib diagnostics::journal::tests -- legacy_report_without_archive_is_explicit_not_an_error
- cargo test --lib diagnostics::journal::tests -- archive_missing_when_expected_is_distinct_from_legacy
- cargo test --lib diagnostics::journal::tests -- loading_error_when_segment_corrupt_despite_finalized_index

## Dependencies

- atomic-index-manifest-finalization

## Risks

- If this task's presence check is implemented here but Task 5.2/6.1 each independently re-derive 'legacy'/'missing'/'loading-error' from different signals, the clients could disagree; mitigate by making this task's ArchiveState the single function both later tasks must call, not a pattern they each reimplement.
- The Missing-vs-Legacy distinction depends on a containing-format archive-awareness signal that is not itself finalized until the desktop save-format or Task 6.1's report schema lands; mitigate by defining that signal as a single explicit boolean/version-threshold input to this task's own function now, so downstream format changes only need to supply that one signal correctly rather than re-deriving the whole state machine.
