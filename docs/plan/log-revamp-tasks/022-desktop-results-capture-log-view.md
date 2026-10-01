<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 022 — Desktop Results Capture Log view

- Slug: `desktop-results-capture-log-view`
- Implements plan section(s): `Phase 5 — Desktop and CLI Views#0`, `Desktop and CLI Experience#0`
- Depends on: desktop-log-screen-archive-loading

## Goal

Add a Capture Log view to Desktop Results for passed and failed items, grouped by execution/viewport/attempt, with reference and test executions independently selectable where both exist and shared scenario history deduplicated.

## Why this exists

Task 5.3's own Task Breakdown acceptance criterion requires a fixture run with two viewports, two attempts, and both reference and test captures to show each combination independently selectable with deduplicated shared history.

## Settled investigation

- src/desktop/results.rs already defines the Report/Outcome (Passed/Mismatch/Error/Unknown, lines 20-37) model used for reading existing report artifacts 'without treating CLI exit zero as a visual pass' — this is the existing Results data model this task's Capture Log view attaches to; the desktop-results-review skill confirms src/desktop/results.rs (models) and src/desktop/ui/reports.rs (UI) are the two files that already own this surface.
- The desktop-results-review skill documents that Results already 'Inventory successful baselines and actual captures, not directories or generated diffs'; independently-selectable reference/test executions are a new requirement this task adds, since today's Results review is per-capture, not per-execution-with-attempts.
- Task 1.3's ExecutionId/attempt: u32/CaptureRole fields from DiagnosticRecordV1 are the exact identity fields this task's grouping-by-execution/viewport/attempt must key on, per Identity and Record Contract's statement that one scenario/viewport execution can produce several images sharing one timeline.

## Inputs

- Task 5.2's evolved archive-loading pattern (as a precedent for reading the journal, not literally reused code)
- Task 2.1's journal reader
- src/desktop/results.rs's existing Report/Outcome model (reference)

## Outputs

- A new Capture Log view reachable from Desktop Results, grouped by execution/viewport/attempt, with independently selectable reference/test executions and deduplicated shared scenario history

## Implementation steps

- Extend src/desktop/results.rs's report-reading model (or add a sibling model) to resolve each capture's ExecutionId/viewport/attempt/CaptureRole identity from the archived journal, keyed consistently with Task 5.2's scenario/job scope keying.
- Add a Capture Log view/tab in src/desktop/ui/reports.rs (or a new sibling module) that groups captures by execution/viewport/attempt and renders reference/test as independently selectable tabs or toggles when both exist for one scenario.
- Deduplicate shared scenario history: when reference and test captures share one execution's queue/setup/navigation history, render that shared portion once, not duplicated per capture.
- Write capture_log_view_groups_by_execution_viewport_attempt: build a fixture run with 2 viewports, 2 attempts, and both reference and test captures (8 combinations), and assert each is independently selectable with the shared history deduplicated.

## Done when

- Test capture_log_view_groups_by_execution_viewport_attempt builds a fixture run with 2 viewports x 2 attempts x {reference, test} (up to 8 combinations) and asserts each combination is independently selectable in the new view.
- The same test asserts the shared per-execution history renders once per execution, not once per capture, by comparing the rendered entry count against the raw per-capture record count.

## Evidence to capture

- capture_log_view_groups_by_execution_viewport_attempt's exact viewport/attempt/role matrix (2x2x2=8) and a count comparing how many times the shared preamble appears (expected: once per execution, i.e. 4 times, not 8)

## Do not change

- Do not alter src/desktop/results.rs's existing Outcome/pass-fail semantics — the Capture Log view is additive to Results, not a replacement for its existing pass/fail/mismatch model.
- Do not conflate reference and test execution identities — Identity and Record Contract requires 'a capture must retain its original execution identity across later runs.'

## Reopen triggers

- Task 5.5's log-health warning work finds this view's grouping model cannot easily attach a per-execution health flag without restructuring.

## Context

Desktop Results today reviews comparison outcomes per capture; this task adds the complementary execution-level diagnostic view the plan requires, reusing the same execution/attempt/role identity fields Task 1.3 defined specifically because 'One scenario/viewport execution can produce several images.'

## Options & tradeoffs

Grouping only by capture (today's model) would be simplest to extend but cannot show 'two viewports, two attempts' as distinct groups without duplicating the underlying shared history per capture; grouping by execution/viewport/attempt with capture-role as a selectable axis (chosen) matches the plan's explicit acceptance criterion at the cost of a more complex view model.

## Decision

Add execution/viewport/attempt grouping with an independently-selectable reference/test axis to Desktop Results, deduplicating shared history at the execution level rather than the capture level.

## Acceptance criteria

- Test capture_log_view_groups_by_execution_viewport_attempt, with a fixture of 2 viewports x 2 attempts x {reference, test} (up to 8 combinations), shows each combination independently selectable in the new view.
- The same test's shared per-execution history (e.g. queue/setup/navigation) renders exactly once per execution (4 times for 2 viewports x 2 attempts), not once per capture (8 times), verified by comparing rendered entry counts against the raw per-capture record count.

## File touchpoints

- src/desktop/results.rs
- src/desktop/ui/reports.rs
- src/diagnostics/journal.rs (reference)

## Tests

- cargo test --features desktop --lib desktop::results::tests -- capture_log_view_groups_by_execution_viewport_attempt
- cargo test --features gui-tests --lib desktop::ui::tests -- results_capture_log_view

## Dependencies

- desktop-log-screen-archive-loading

## Risks

- Resolving which journal entries belong to which (execution, viewport, attempt, role) tuple when a scenario was re-run (new attempt) could misattribute an older attempt's history to a newer one if attempt numbers are not threaded consistently; mitigate by asserting attempt-number fidelity explicitly in the fixture test, not just final-state correctness.
