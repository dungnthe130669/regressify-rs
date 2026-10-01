<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 023 — Filtering/search/follow-tail/copy/download for log views

- Slug: `desktop-log-filtering-search-export`
- Implements plan section(s): `Phase 5 — Desktop and CLI Views#0`, `Desktop and CLI Experience#0`
- Depends on: desktop-results-capture-log-view

## Goal

Add severity/stage filtering, search, timestamps/durations, follow-tail for active runs, and copy/download of sanitized logs to both Activity (live) and Results (archived) views, without the filtering changing the stored timeline and without blocking the UI on a large synthetic log.

## Why this exists

Task 5.4's own Task Breakdown acceptance criterion requires a UI test or documented manual script exercising these controls without blocking the UI on a large synthetic log.

## Settled investigation

- src/desktop/ui/runs/log.rs::render_log (lines 162-252) already renders a read-only CodeEditor with a single 'Copy log' button (lines 220-231) that copies self.log_text verbatim via cx.write_to_clipboard — this existing copy mechanism is the precedent this task extends with filtering/search/download.
- src/desktop/ui/runs/activity.rs::RunPanel::outcome_heading/status_color (lines 4-36+) already models run-level status/outcome for the Activity tab; this task's filtering additions sit alongside this existing Activity rendering.
- The desktop-native-ui skill documents that 'Deployment activity uses the shared read-only log editor at full remaining height, not static tail lines' — the existing editor widget is the one this task must keep using unless performance testing proves it cannot handle a large synthetic log.
- The desktop-progress-events skill's existing invariant 'Bound event/log processing per UI tick; do not block on a worker-held progress mutex. Notify views only when their observable revision changes' (keyed off LiveProgress::revision(), src/progress.rs lines 443-445) is the existing nonblocking-UI pattern this task's filtering/search must not violate.

## Inputs

- Task 5.2's archive-loading RunPanel (Activity-side)
- Task 5.3's Capture Log view (Results-side)
- The existing CodeEditor/copy-to-clipboard pattern in src/desktop/ui/runs/log.rs (reference)

## Outputs

- Severity/stage filter controls, a search input, visible timestamps/durations, a follow-tail toggle for active runs, and a download-sanitized-log action, present in both the Activity and Results log views

## Implementation steps

- Add severity/stage filter controls and a search input above the existing CodeEditor in render_log (src/desktop/ui/runs/log.rs lines 162-252), filtering the displayed text/entries without mutating self.log_text's underlying stored timeline.
- Add a follow-tail toggle that, when enabled during an active run, keeps the view scrolled to the latest entry as new records arrive.
- Extend the existing 'Copy log' button's sibling area (log.rs lines 220-231) with a 'Download' action that writes the currently filtered-or-full sanitized log to a file, reusing the same sanitized text already safe for clipboard copy.
- Generate a large synthetic log (reusing Task 2.1's >10,000-record test fixture) and drive it through both the Activity and Results views in run_log_filter_search_download, asserting filter/search operations complete without a dropped frame/blocked input event.

## Done when

- Test run_log_filter_search_download drives severity/stage filtering and a search query against a log fixture of at least 10,000 synthetic records in both the Activity and Results views, and asserts the UI's per-frame processing stays within the existing UI test harness's default responsiveness bound with no dropped input.
- The same test triggers the download action and asserts the resulting file's bytes match the view's currently-filtered rendered text exactly, byte-for-byte.

## Evidence to capture

- run_log_filter_search_download's fixture record count (>=10,000), the measured filter/search responsiveness, and a byte-level diff between the downloaded file and the view's rendered text

## Do not change

- Do not let filtering/search mutate the stored timeline — Desktop and CLI Experience is explicit that 'Filtering changes the view, not the stored timeline.'
- Do not replace the existing CodeEditor-based read-only rendering widget without first proving it cannot meet the large-log responsiveness requirement; the desktop-native-ui skill documents it as the established pattern for this exact kind of view.

## Reopen triggers

- Task 5.5's log-health warning work needs a filter state (e.g. 'show only entries contributing to the health warning') this task's filter model does not yet support.

## Context

Both the live Activity view (Task 5.2) and the archived Results view (Task 5.3) need the same filter/search/export controls; building them once as a shared control surface avoids the two views drifting into inconsistent filtering behavior.

## Options & tradeoffs

Building separate filter implementations for Activity and Results would let each evolve independently but risks exactly the inconsistency Current Evidence warns against ('Extend these surfaces rather than introducing a competing log viewer'); a shared filter/search/export component used by both (chosen) costs more upfront design but keeps the two views consistent.

## Decision

Implement filtering/search/follow-tail/copy/download as one shared control surface used by both the Activity and Results log views.

## Acceptance criteria

- Test run_log_filter_search_download drives severity/stage filtering and search against a >=10,000-record synthetic log (Task 2.1's fixture scale) and asserts the UI remains responsive (no blocked input/frame) in both the Activity and Results views.
- The download action in the same test produces a file containing the currently filtered (or full) sanitized log text, verified byte-for-byte against the view's own rendered text.

## File touchpoints

- src/desktop/ui/runs/log.rs
- src/desktop/ui/runs/activity.rs
- src/desktop/ui/reports.rs

## Tests

- cargo test --features gui-tests --lib desktop::ui::tests -- run_log_filter_search_download

## Dependencies

- desktop-results-capture-log-view

## Risks

- Driving a >10,000-record log through the existing single CodeEditor widget (which renders one large text blob today per refresh_log's String::with_capacity/push_str pattern, log.rs lines 140-153) may not scale to progressive loading without blocking as-is; mitigate by measuring actual responsiveness in the test early, and if it fails, escalate to a virtualized/paginated rendering approach rather than silently accepting UI jank.
