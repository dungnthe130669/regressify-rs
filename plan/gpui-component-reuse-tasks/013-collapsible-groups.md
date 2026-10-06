# 013: Controlled Collapsible Groups

Status: Not started. Depends on 001 and 004.

## Scope

Controlled Accordion for Fields and applicable groups, Collapsible for rich
triggers in [Fields](../../src/desktop/ui/mod.rs),
[navigation](../../src/desktop/ui/navigation.rs) and
[run activity](../../src/desktop/ui/runs/activity.rs).

## Ordered Work

1. Baseline relevant `setting_search_`, `navigator_`, `run_log_` and progress
   tests; record nonzero counts and load owning skills before edits.
1. Replace matching disclosure presentation with controlled components, one
   surface at a time. Keep multiple groups open, stable IDs, filtered mapping
   and Expand All; labels/filtered indices are not identity.
1. Errors or search matches force the entire group open while retaining the
   underlying user collapse preference separately. Restore it when the forced
   condition clears. Do not restore the old partial-error presentation.
1. Immediately test the first group. Extend existing cases for overlapping
   search/errors, multiple forced groups, collapse while forced, clearing
   conditions, filtered reorder, Expand All, keyboard focus and async updates.
1. Maintain docs/skills/memory, inspect screenshots and record adapter cost.

## Acceptance And Checks

Run relevant known filters with gui-tests and `--test-threads=1`; require nonzero
counts. Introduce `controlled_group_` in existing tests only if needed and run
it. Run `cargo fmt --all -- --check` and affected feature/native smoke gates.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md),
[progress events](../../.github/skills/desktop-progress-events/SKILL.md) and
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md).
Commit groups/tests/maintenance only. Reopen if controlled multi-open or
independent user preference cannot be preserved. Keep tri-state checkboxes and
flat page sections; do not add nested decorative cards.
