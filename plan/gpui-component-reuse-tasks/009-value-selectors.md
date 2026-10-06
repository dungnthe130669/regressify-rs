# 009: Single And Multiple Value Selectors

Status: Not started. Depends on 001.

## Scope

R6 single-choice Fields, deployment, token and Git remote pickers; guided
multi-choice `viewportNames`/`needs` in [choices](../../src/desktop/ui/choices.rs).
Select/Combobox represent values, not commands. Raw textareas and application
write policy remain authoritative.

## Ordered Work

1. Baseline relevant `setting_search_`, `deployment_activity_`,
   `desktop::ui::git::tests` and source/choice tests with nonzero counts.
   Read the owning skills before editing each surface.
1. Pilot Select for Control::Choice. Retain state per binding/panel, subscribe
   only to user Confirm and route through the existing guarded write. Sync
   rebuilt values/items by stable identity; programmatic selection must not
   create an override or document-history entry.
1. Display effective inherited values without writing them; explicit same-value
   confirmation may create an override. Unknown source text stays visible with
   invalid-state feedback until an explicit choice. Preserve reset separately.
1. Propagate to deployment, tokens and Git remotes through current callbacks,
   preserving provider/token identity locks, drafts and operation validation.
1. Pilot multi Combobox beside raw textareas. Apply Change once per accepted
   toggle; Confirm/Escape close without duplicate writes or rollback. Keep
   unknown entries, scalar/list syntax, viewport case-insensitivity, dependency
   case-sensitivity, inherited defaults and last-viewport guard. Reconcile a
   rejected toggle back into controlled selection.
1. Immediately validate each pilot, then extend existing tests for keyboard/
   pointer choices, same-value confirmation, refresh while searching, unknown
   values, reset, invalid drafts, failed writes and exact history counts.
1. Maintain docs/skills/memory and measure deletion versus adapter state.

## Acceptance And Checks

Run relevant baseline native filters with nonzero counts. Add clearly new
`value_selector_` or `choice_combobox_` tests in existing modules for missing
coverage; test user confirmation separately from programmatic sync. Run
`cargo fmt --all -- --check` and affected feature/native smoke gates.

Follow [lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md),
[config cascade](../../.github/skills/config-cascade/SKILL.md),
[deployment targets](../../.github/skills/deployment-targets/SKILL.md),
[Git review](../../.github/skills/desktop-git-review/SKILL.md) and
[native UI](../../.github/skills/desktop-native-ui/SKILL.md).
Commit these integrations/tests/maintenance only. Reopen for lost raw values,
stale identity, bypassed guards or duplicate/rolled-back history. Keep command
actions as menus and investigate before asking to retain custom pickers.
