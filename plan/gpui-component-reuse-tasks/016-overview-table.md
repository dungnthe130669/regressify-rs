# 016: Overview Table And Complete Details

Status: Not started. Depends on 001.

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
