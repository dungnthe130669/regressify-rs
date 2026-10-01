<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 020 — CLI verbosity flags and environment precedence

- Slug: `cli-verbosity-flags-precedence`
- Implements plan section(s): `Phase 5 — Desktop and CLI Views#0`, `CLI flags and environment precedence (proposed)#0`, `Configuration-cascade integration (proposed)#0`
- Depends on: install-subscriber-entry-points, operation-journal-writer

## Goal

Finalize and implement the proposed --log-level/--log-verbose CLI flags and their CLI > environment variable > per-scenario > per-suite > workspace > built-in-default precedence, proving archive completeness is unaffected by verbosity level in every tested combination, and confirm Task 2.1's separate archive-writer-behavior flag/env var stays split from this cascade in both directions.

## Why this exists

This is Phase 5's first task and the Open Questions item 4 owner. It is also one of the two tasks (alongside Task 2.1) Configuration-cascade integration names to confirm archive-writer settings stay split from the display-verbosity cascade.

## Settled investigation

- src/cli/root.rs's Cli struct already has pub verbose: bool (line 41, --verbose) and pub debug: bool (line 70, --debug) flags; src/application.rs::set_global_vars (line 76) already threads cli.debug into the common::DEBUG OnceLock<bool> global — this task's new flags should follow this exact established pattern.
- The real ResolvedConfig/configuration cascade (src/config/mod.rs, ResolvedConfig struct at line 118) is confirmed to be a pure four-level scenario/suite/workspace/defaults file-config merge today, with no existing CLI/env precedence step at all.
- src/config/replacements.rs::get_profile_name() (lines 56-66) is the only existing CLI-then-env precedence pattern in this codebase (a 3-level fallback) and is explicitly a separate, simpler mechanism from the scenario/suite/workspace cascade.
- Archive writer behavior (journal chunk size, disk budget, retention) is a different settings family, resolved operation-level/global only, not scenario-cascaded — this task must not conflate display-verbosity's six-level cascade with that separate setting.
- This task's own acceptance criteria require reading back the Task 2.1 archive's recorded standard-event count to prove verbosity-independence; that requires Task 2.1's operation-journal-writer to exist, and the test can feed synthetic records directly to that writer/reader.
- Task 2.1 (operation-journal-writer) now implements the plan's required archive-writer CLI flag/environment variable (chunk size and disk budget), resolved once at process start via a OnceLock pattern, independent of this task's own scenario/suite/workspace/defaults display-verbosity cascade; this task's own role per Configuration-cascade integration is to confirm that split holds, not to implement the archive-writer flag itself.

## Inputs

- Task 1.5's installed subscriber (its console layer is what this task's verbosity level filters)
- src/config/mod.rs's real ResolvedConfig/cascade loader
- src/config/replacements.rs::get_profile_name as CLI-then-env precedent (reference)
- Task 2.1's journal writer/reader (to read back the archive's recorded standard-event count for the precedence-vs-completeness assertion)
- Task 2.1's (operation-journal-writer) new chunk-size/disk-budget CLI flag and environment variable, as the setting this task's split-confirmation test proves is NOT part of its own cascade

## Outputs

- --log-level <trace|debug|info|warn|error> and REGRESSIFY_LOG_LEVEL CLI/env flags
- A --log-verbose concise/verbose console toggle
- The new display-verbosity field threaded through the scenario -> suite -> workspace -> defaults cascade
- A flag/environment matrix test
- A test proving Task 2.1's archive-writer flag/env var is unreachable from, and does not affect, this cascade

## Implementation steps

- Add --log-level/REGRESSIFY_LOG_LEVEL and --log-verbose fields to src/cli/root.rs's Cli struct, following the existing --debug (line 70)/--verbose (line 41) field style.
- Add a new optional display-verbosity field to each cascade level's config struct in src/config/mod.rs (scenario, suite, workspace), resolved the same way other cascading optional settings already resolve into ResolvedConfig.
- Implement the six-level precedence as new resolution logic — do not assume src/config/replacements.rs::get_profile_name's 3-level CLI-then-env fallback generalizes directly.
- Wire the resolved display-verbosity value into Task 1.5's console-formatting subscriber layer so it filters console output only, never the archive-writing layer.
- Write log_level_precedence_matrix: set each precedence level in turn and assert the effective console verbosity follows the documented precedence order, while a parallel assertion confirms the Task 2.1 journal still records every standard event regardless of which verbosity level won.
- Write archive_writer_settings_are_not_scenario_cascaded: set a scenario-level (and separately, a suite-level and workspace-level) display-verbosity override and assert Task 2.1's resolved journal chunk-size/disk-budget value is unaffected; separately set Task 2.1's own CLI flag/environment variable and assert no scenario/suite/workspace-level display-verbosity resolution changes as a result — confirming the two settings families stay split, per Configuration-cascade integration's explicit assignment of this confirmation to Task 5.1.

## Done when

- Test log_level_precedence_matrix exercises at least 3 combinations (CLI flag + env var both set, env var set alone with no flag, neither set with only workspace-level config set) and asserts the documented precedence order (CLI > env > scenario > suite > workspace > default) is followed in each.
- A paired assertion confirms the Task 2.1 archive's recorded standard-event count is identical across at least 2 different resolved verbosity levels (trace and error) for the same synthetic run.
- Test archive_writer_settings_are_not_scenario_cascaded shows a scenario/suite/workspace-level display-verbosity override leaves Task 2.1's resolved chunk-size/disk-budget value unchanged, and shows setting Task 2.1's own CLI flag/environment variable leaves every tested display-verbosity cascade level's resolution unchanged.

## Evidence to capture

- log_level_precedence_matrix's full combination table (which setting won at each of the 3+ combinations tested) and the archive-event-count comparison across the 2 verbosity levels
- archive_writer_settings_are_not_scenario_cascaded's before/after values on both sides of the split (display-verbosity resolution and Task 2.1's chunk-size/disk-budget resolution), proving neither setting family leaks into the other

## Do not change

- Do not let any verbosity level, at any precedence tier, suppress a required standard event from reaching the Task 2.1 journal — only console/UI display filtering is in scope.
- Do not conflate this display-verbosity cascade with the separate archive-writer-behavior settings (chunk size, disk budget) Task 2.1 implements as a non-cascaded CLI flag/environment variable — this task's archive_writer_settings_are_not_scenario_cascaded test is what proves that separation holds, not merely a documented intention.

## Reopen triggers

- Task 7.3's documentation pass finds a flag name or precedence detail inconsistent with what actually shipped here.
- The real src/config/mod.rs cascade loader turns out to resolve optional fields differently than assumed, requiring this task's resolution logic to change.
- Task 2.1's chunk-size/disk-budget flag/env var naming or reading mechanism changes shape, requiring this task's split-confirmation test to be updated to match.

## Context

This is the only task in the whole plan that touches the real configuration cascade in src/config/mod.rs; Open Questions and Remaining Decisions item 4 and the Assumptions section both flag this as genuinely new design work with an explicit re-check point, not a known-safe extension. Configuration-cascade integration also assigns this task a confirming role for Task 2.1's separate archive-writer-behavior flag/env var, which this task now actively tests rather than merely documents as excluded.

## Options & tradeoffs

Reusing get_profile_name's CLI-then-env shape alone (2 levels) would be far simpler but would not integrate with the existing scenario/suite/workspace cascade other settings already use; the chosen six-level precedence is more design work but keeps this setting consistent with how every other cascading setting in this codebase already resolves.

## Decision

Add display verbosity as a new optional field at each existing cascade level, resolved with CLI flag and environment variable as two new tiers above the existing scenario -> suite -> workspace -> defaults order, keeping archive-writer behavior a separate, non-cascaded setting (now implemented by Task 2.1) whose independence from this cascade this task actively proves.

## Acceptance criteria

- Test log_level_precedence_matrix exercises at least CLI-flag-wins, env-var-wins-without-a-flag, and cascade-level-wins-without-either cases and confirms the documented precedence in each.
- The same or a paired test confirms the Task 2.1 archive's recorded standard-event count is identical across at least two different resolved verbosity levels (e.g. trace and error) for the same synthetic run.
- Test archive_writer_settings_are_not_scenario_cascaded confirms Task 2.1's chunk-size/disk-budget CLI flag/environment variable and this task's own display-verbosity cascade resolve completely independently of each other, in both directions.

## File touchpoints

- src/cli/root.rs
- src/config/mod.rs
- src/application.rs
- src/config/replacements.rs (reference)
- src/diagnostics/journal.rs (reference)
- src/common/globals.rs (reference)

## Tests

- cargo test --lib config:: -- log_level_precedence_matrix
- cargo test --lib cli:: -- log_level (proposed, new flag-parsing tests following the existing cli_rejects_zero_global_capture_limit-style tests in src/cli/root.rs)
- cargo test --lib config:: -- archive_writer_settings_are_not_scenario_cascaded

## Dependencies

- install-subscriber-entry-points
- operation-journal-writer

## Risks

- Adding a new cascading config field without deny_unknown_fields-safe handling at every level could silently accept a typo'd field name; mitigate by following the existing config-cascade skill's explicit requirement to keep serde names, unknown-field rejection, and schema constraints aligned when adding a field.
- This task's archive-event-count assertion requires Task 2.1's reader API to already exist with a readable record-count accessor; mitigate by sequencing this task after operation-journal-writer (already reflected in Dependencies) rather than treating the Task 2.1 reference as advisory-only.
- If this task's split-confirmation test only checks one direction (e.g. only that display-verbosity overrides don't affect the archive writer, not the reverse), a real conflation bug in the other direction could still slip through; mitigate by asserting both directions explicitly in archive_writer_settings_are_not_scenario_cascaded.
