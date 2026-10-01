<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 024 — Log-health warnings and disabled exports for partial logs

- Slug: `desktop-log-health-warnings`
- Implements plan section(s): `Phase 5 — Desktop and CLI Views#0`, `Desktop and CLI Experience#0`, `Definition of Done#0`
- Depends on: desktop-results-capture-log-view, desktop-log-filtering-search-export, writer-health-kill-switch

## Goal

Ensure a fixture run whose capture passes but whose archive is marked partial (via Task 2.6's writer-health signal) shows an unchanged pass badge, an explicit log-health warning in both the Capture Log view and the run summary, and a disabled (not shortened) download/export control — the exact control Task 5.4 (desktop-log-filtering-search-export) introduces; a fully-available-log fixture shows that same export control enabled.

## Why this exists

This is Task 5.5's own exact acceptance criterion in Task Breakdown and is independently restated as Definition of Done item 10 — a hard completion gate, not merely a nice-to-have.

## Settled investigation

- Task 2.6 (writer-health-kill-switch) already defines the writer-health signal this task consumes; this task does not invent a new health source, only renders the existing one.
- src/desktop/results.rs::Outcome (lines 20-37: Passed/Mismatch/Error/Unknown) is the existing pass/fail badge model; this task must leave its values and meaning completely unchanged for a partial-log capture — only an additional, separate warning indicator is added alongside it.
- The desktop-progress-events skill's existing invariant 'Results badges prefer the loaded report's scenario/viewport counts while idle... Retain live counts during runs' establishes that badge computation already has established rules this task must not disturb; the log-health warning is an orthogonal annotation, not a badge-computation change.
- Task 5.4 (slug desktop-log-filtering-search-export, Task 023 in this decomposition) is the task whose own Outputs introduce the download/export control and the 'download-sanitized-log' action this task disables — it is a sibling of Task 5.3 (desktop-results-capture-log-view) that itself depends on Task 5.3, not an ancestor reachable through Task 5.3 alone, so it must be listed directly in this task's own Dependencies, not only named in Inputs/Implementation steps.

## Inputs

- Task 2.6's (writer-health-kill-switch) writer-health signal and kill-switch
- Task 5.3's (desktop-results-capture-log-view) Capture Log view
- Task 5.4's (desktop-log-filtering-search-export) export/download control, which this task disables for partial-archive captures
- src/desktop/results.rs's existing Outcome badge model (unchanged)

## Outputs

- A log-health warning rendered in both the Capture Log view and the run summary for any capture whose archive is marked partial/unavailable
- A disabled download/export control (with a stated reason) for that capture's log
- The pass badge for that capture remains unchanged

## Implementation steps

- Confirm Task 5.4's (desktop-log-filtering-search-export) download/export control is in place before wiring the disable logic — this task's own acceptance test disables that exact control, so it must already exist, not only Task 5.3's view.
- Thread Task 2.4's ArchiveState/Task 2.6's writer-health signal through to the Capture Log view (Task 5.3) and the run summary rendering, as an additional field alongside (never replacing) the existing Outcome badge value.
- Render an explicit log-health warning in both locations whenever the archive state is partial or unavailable for a capture that otherwise passed.
- Disable Task 5.4's download/export control for that capture's log, with a stated reason, rather than allowing a truncated download.
- Build log_health_warning_partial_archive_keeps_pass_badge with two fixtures: one whose capture passes but whose archive is forced partial via Task 2.6's writer-health signal, and one whose log is fully available; assert the first shows (a) unchanged pass badge, (b) the log-health warning in both locations, (c) disabled export with a stated reason; assert the second shows the same export control enabled.

## Done when

- Test log_health_warning_partial_archive_keeps_pass_badge builds a fixture whose capture passes but whose archive is forced Partial via Task 2.6's writer-health signal, and asserts (a) the Outcome::Passed badge value is unchanged, (b) both the Capture Log view and run summary show an explicit log-health warning, (c) the export control introduced by Task 5.4 is disabled with a stated reason.
- A second fixture in the same test module, whose log is fully available (ArchiveState::Complete), shows that same Task 5.4 export control enabled.

## Evidence to capture

- Both fixtures' exact archive-state inputs and the resulting rendered badge/warning/export-control states for each, from log_health_warning_partial_archive_keeps_pass_badge
- Confirmation that Task 5.4's (desktop-log-filtering-search-export) download/export control, not a placeholder, is the control exercised by this task's test

## Do not change

- Do not change Outcome's pass/fail/mismatch computation or its existing badge color/label rules for a partial-log capture — Definition of Done item 10 and Desktop and CLI Experience both require the pass badge to stay unchanged.
- Do not offer a shortened/truncated download as if it were complete; the export control must be disabled, not silently degraded.

## Reopen triggers

- A future workspace-level compliance feature later needs a different warning-suppression rule than this task's unconditional surfacing.
- Task 5.4 ships its download/export control with a different disable/reason API than assumed here, requiring this task's wiring step to adapt.

## Context

This task is the user-visible proof that 'Logging failure is warning-only' does not mean 'invisible' — a passed capture with a degraded log must still make that degradation visible, and must not let a user download a shortened file believing it is the complete record. It directly depends on Task 5.4's export control existing, since disabling a control that is not yet built cannot be tested.

## Options & tradeoffs

Silently keeping the capture passed with no visible indication would satisfy 'pass badge unchanged' trivially but violates the explicit requirement to surface the warning; disabling the export outright (chosen) is more restrictive for the user than offering a partial download, but matches 'Disable unavailable exports rather than presenting a shortened download as complete' exactly.

## Decision

Add an orthogonal log-health warning annotation alongside the existing unchanged pass badge, and disable (not degrade) the Task 5.4 export control for any capture whose archive is not fully available; depend directly on Task 5.4 (desktop-log-filtering-search-export) rather than only on Task 5.3, since the control being disabled is Task 5.4's deliverable.

## Acceptance criteria

- Test log_health_warning_partial_archive_keeps_pass_badge's partial-archive fixture pass badge value (Outcome::Passed) is identical before and after this task's change, verified by direct comparison.
- The same test's fixture shows an explicit log-health warning in both the Capture Log view and run summary, and Task 5.4's export control is disabled with a stated reason rather than producing a shortened file; a second, fully-available-log fixture in the same test shows that same control enabled.

## File touchpoints

- src/desktop/ui/reports.rs
- src/desktop/ui/runs/log.rs
- src/desktop/ui/runs/activity.rs
- src/desktop/results.rs (reference)

## Tests

- cargo test --features gui-tests --lib desktop::ui::tests -- log_health_warning_partial_archive_keeps_pass_badge

## Dependencies

- desktop-results-capture-log-view
- desktop-log-filtering-search-export
- writer-health-kill-switch

## Risks

- If the warning indicator and the pass/fail badge share visual styling (e.g. both using a colored dot), a user could misread the warning as a second status; mitigate by using a visually distinct warning affordance distinct from the existing status_color-driven badge, and covering this distinction explicitly in the native-UI test.
- If Task 5.4's export control ships with a different disable/reason API than assumed, this task's wiring step must adapt; mitigate by treating Task 5.4 as a hard Dependencies entry (not an advisory Inputs-only reference) so this task cannot start before that control's real shape is known.
