# 016: Overview Table And Complete Details

Status: Committed as `d8e9965`. DataTable and complete
selectable Details pass focused and native acceptance. The integrated UI gate
has only the approved task 019 header failure. Depends on 001.

## Scope

Uniform Kit Table in [overview](../../src/desktop/ui/overview.rs), with independent
checkbox, status, Edit/Open Report and Details actions. Full long summaries and
errors live in an explicit selectable Details dialog.
[Source diagnostics](../../src/desktop/ui/source.rs) remain variable-height and
selectable; this task does not authorize clipping them.

## Ordered Work

1. Baseline neighboring overview/selection tests and relevant `navigator_`,
   `suite_name_dialog_` and `source_line_commands` tests. Require nonzero counts
   and inspect actual summary/error ownership before edits.
1. Pilot uniform Table rows and toolkit columns/header/scrolling. Retain existing
   row controls and separate focus, selection and activation. Immediately test
   checkbox and action isolation before adding the details workflow.
1. Open Details only through its explicit control or Enter on focused row;
   selection and single click alone must not show a modal. Show complete,
   selectable long summary/error text without losing information.
1. Preserve Edit/Open Report and row checkbox behavior. Test long content,
   missing/error states, keyboard dialog opening/close, copy/selection, refresh,
   and no unwanted opening from selection or embedded actions.
1. Keep diagnostics geometry/selectability unchanged. Review both themes and
   compact/large screenshots; update docs/guidance/memory and code accounting.

## Acceptance And Checks

Run touched-area baseline filters using gui-tests with `--test-threads=1`, all
nonzero. Add new `overview_table_` regressions to existing UI tests for the new
Details workflow and execute that filter. Run `cargo fmt --all -- --check` and
affected feature/native smoke gates. Complete text must be verifiable, not just
present in a truncated label or inaccessible tooltip.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[results review](../../.github/skills/desktop-results-review/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md) and
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md).
Commit table/details/tests/maintenance only. Reopen for lost content, keyboard
access, selection/copy or action boundaries. Keep variable-height diagnostics.

## Verification

- Four new `overview_table_` regressions pass, covering independent actions,
  complete text/copy, focus, refresh/release and uniform virtual rows.
- Full locked GUI UI gate: 265 passed, 1 failed, 0 ignored. The sole failure is
  `git_large_tree_and_status_fit_compact_and_wide_windows`, the approved task 019
  compact-header exception; its assertion remains unchanged and non-green.
- Rustfmt, production GUI Clippy with `-D warnings` and desktop build pass.
- Rebuilt native Default and Schema smoke pass. All eight Overview/Details
  screenshots were reviewed at 1080x720 and 1600x1000 in Light/Dark; selectable
  Source diagnostics retain wrapping and exact copy behavior. Clipboard and
  desktop preferences were restored. Wide-only theme selection remains owned
  by task 019, not passing compact-header evidence.
- Production net +316 lines; GUI tests +306; native smoke +35. No dependency,
  toolkit, Source diagnostic or persistence changes. Linux remains deferred.
