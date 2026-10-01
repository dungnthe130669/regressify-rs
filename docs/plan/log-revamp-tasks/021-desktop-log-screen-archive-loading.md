<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 021 — Evolve the scoped log screen to load archived records

- Slug: `desktop-log-screen-archive-loading`
- Implements plan section(s): `Phase 5 — Desktop and CLI Views#0`, `Desktop and CLI Experience#0`
- Depends on: legacy-migration-behavior

## Goal

Evolve src/desktop/ui/runs/log.rs's existing scoped log screen so reopening a workspace after restart shows the full archived timeline for a previously completed run, replacing the omitted-live-history notice with a loaded-from-archive state.

## Why this exists

Phase 5's own Task 5.2 acceptance criterion requires this; it is also the first UI-visible payoff of Phase 2's journal and Task 2.4's archive-state detection — Desktop and CLI Experience requires all four non-complete states (legacy, partial, missing, loading-error) to be shown explicitly, not only the legacy/partial pair.

## Settled investigation

- src/desktop/ui/runs/log.rs::RunPanel::refresh_log (lines 103-160) is the exact existing function that today renders only self.manager.progress.logs (bounded LiveProgress history, scoped) or self.manager.logs (raw child stdout/stderr, unscoped) — both live-only sources that do not survive a desktop restart.
- The existing omitted-history notice is literally the string '[{} older scenario log entries omitted from the run's bounded history]\n' built from progress.omitted_logs (log.rs lines 107-111) — this is the exact notice this task must replace with a loaded-from-archive state once an archive is available, not remove without replacement.
- RunPanel::show_scenario_log/show_job_log/show_all_logs (log.rs lines 39-101) already compute a ScenarioLogScope{jobs: BTreeSet<usize>, title: String} used to filter progress.logs by job id; any archive-backed replacement must preserve this same scenario/job navigation and scope-title behavior.
- Task 2.4 (legacy-migration-behavior) now resolves a 5-variant ArchiveState (Complete, Partial, Missing, LegacyUnavailable, LoadingError) for any scope; this task's refresh_log must render a distinct message for every one of the four non-complete variants, not only Partial/LegacyUnavailable.

## Inputs

- Task 2.4's 5-variant ArchiveState (Complete/Partial/Missing/LegacyUnavailable/LoadingError) detection
- Task 2.1's journal reader
- src/desktop/ui/runs/log.rs's existing RunPanel methods (modified, not replaced)

## Outputs

- refresh_log (or a new sibling function) renders from the archived journal when LiveProgress's bounded history has been evicted or the workspace was just reopened
- The omitted-live-history notice is replaced by an explicit loaded-from-archive state when Complete, and by one of Task 2.4's four distinct explicit states — Partial, Missing, LegacyUnavailable, or LoadingError — otherwise, never a generic error or silent fallback to the old notice

## Implementation steps

- Add an archive-lookup path that, given the current scenario/job scope's stable identity, calls Task 2.1's reader (via Task 2.4's ArchiveState check first) to fetch the full timeline for display instead of only progress.logs.
- Modify refresh_log to prefer the archive when ArchiveState::Complete (or Partial) is available for the current scope, falling back to the existing live-buffer rendering only while a run is actively in progress and no archive exists yet.
- Replace the hardcoded omitted-entries string (log.rs lines 107-111) with a state-aware message: 'loaded from archive' when Complete, a partial notice with any known missing-range metadata when Partial, an explicit 'log unavailable for this run' message when Missing, an explicit legacy message when LegacyUnavailable, and an explicit 'archive could not be read' loading-error message when LoadingError — and the existing omitted-history notice only while still reading from the live buffer during an active run.
- Preserve ScenarioLogScope's existing jobs/title computation (log.rs lines 3-94) unchanged so archive-backed rendering still groups by the same scenario/job scope as today.
- Write run_log_loads_archive_after_restart: complete a run, drop/evict the live LiveProgress buffer, reopen the workspace, and assert the log screen shows the full archived timeline with a loaded-from-archive state.
- Write run_log_shows_loading_error_state_for_corrupt_archive: construct a scope whose Task 2.4 lookup returns LoadingError (a corrupted finalized segment), and assert refresh_log's rendered text shows an explicit loading-error message distinct from the loaded-from-archive success text and from the legacy/partial/missing notices.

## Done when

- Test run_log_loads_archive_after_restart completes a synthetic run, drops the in-memory LiveProgress buffer (simulating restart), reopens the workspace, and asserts RunPanel::refresh_log's rendered text contains the full archived timeline (matching the original record count) rather than the prior omitted-history notice.
- The same test asserts the rendered text contains an explicit 'loaded from archive' state label rather than the old '[... older scenario log entries omitted ...]' notice (src/desktop/ui/runs/log.rs lines 107-111).
- Test run_log_shows_loading_error_state_for_corrupt_archive constructs a scope whose archive Task 2.4 reports as LoadingError and asserts refresh_log's rendered text contains an explicit loading-error message, distinct from the loaded-from-archive, legacy, partial, and missing texts.

## Evidence to capture

- run_log_loads_archive_after_restart's before (live, mid-run) and after (restart, archive-loaded) rendered text samples, showing the notice-text transition
- run_log_shows_loading_error_state_for_corrupt_archive's rendered loading-error text, confirmed distinct from the other four state texts

## Do not change

- Do not remove ScenarioLogScope's existing job-filtering/title behavior — Desktop and CLI Experience requires preserving 'its scenario/job navigation and explicit omitted-live-history notices' through this evolution.
- Do not alter RunPanel::show_scenario_log/show_job_log/show_all_logs's public behavior/signatures; only refresh_log's data source changes.
- Do not render Missing or LoadingError using the same text as LegacyUnavailable or Partial — Desktop and CLI Experience requires all four to be explicit and distinguishable.

## Reopen triggers

- Task 5.4's filtering/search work finds the archive-backed text representation this task produces is not structured enough to filter by severity/stage efficiently, requiring a richer in-memory model than a single rendered string.

## Context

This is the direct evolution of the one existing log-viewing surface in the desktop app; Current Evidence is explicit that this plan should 'Extend these surfaces rather than introducing a competing log viewer,' making this task's in-place modification of RunPanel the required approach.

## Options & tradeoffs

A brand-new archive-only log viewer alongside the existing live one would be simpler to implement in isolation but directly contradicts Current Evidence's explicit instruction to extend the existing surface rather than compete with it; evolving refresh_log in place (chosen) is more delicate but satisfies that instruction.

## Decision

Evolve RunPanel::refresh_log in place to prefer archive-backed rendering when available, preserving existing scope/navigation behavior and replacing only the omitted-history notice's content, not its presence.

## Acceptance criteria

- Test run_log_loads_archive_after_restart, which completes a run, simulates a restart (live buffer gone), and reopens the workspace, shows the complete archived timeline (matching the original record count), not a truncated or empty one.
- The same test shows the rendered text contains an explicit 'loaded from archive' indicator rather than the old bounded-history omission notice.
- Test run_log_shows_loading_error_state_for_corrupt_archive shows an explicit loading-error message for a scope whose archive Task 2.4 reports as LoadingError, distinct from the loaded-from-archive, legacy, partial, and missing texts.

## File touchpoints

- src/desktop/ui/runs/log.rs
- src/diagnostics/journal.rs (reference)
- src/desktop/runner.rs (reference)

## Tests

- cargo test --features gui-tests --lib desktop::ui::tests -- run_log_loads_archive_after_restart
- cargo test --features gui-tests --lib desktop::ui::tests -- run_log_shows_loading_error_state_for_corrupt_archive

## Dependencies

- legacy-migration-behavior

## Risks

- Fetching the full archive synchronously on the UI thread when a scope is opened could block rendering for a very large log; mitigate by following the existing 'Bound event/log processing per UI tick; do not block on a worker-held progress mutex' invariant from the desktop-progress-events skill, loading progressively rather than all at once.
- Rendering five distinct state messages (success plus four problem states) risks text drifting out of sync with Task 2.4's actual enum variants if hand-copied; mitigate by matching exhaustively on Task 2.4's ArchiveState enum (compiler-enforced exhaustiveness) rather than an if/else chain that could silently fall through to a wrong default.
