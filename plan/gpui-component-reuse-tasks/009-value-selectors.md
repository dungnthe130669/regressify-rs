# 009: Single And Multiple Value Selectors

Status: Implementation and acceptance complete; authorized local commit pending.
Depends on 001.

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

## Implementation Evidence

- Task 008 is committed as `fa13c04`; task 009 remains uncommitted.
- Baseline `setting_search_` passes (1). Control::Choice owns SelectState and
  listens only for user Confirm. The existing edit/flush/guard path remains;
  rejected writes reconcile the selected value, and unknown raw values remain
  visible with unsupported-value feedback and no synthetic history.
- Choice state is retained by workspace-root/document/selection and field key,
  with current items/value resynchronized on rebuild. The same-relative-path
  cross-workspace test failed before adding root identity and now passes.
- Focused cases passed: `value_selector_confirmation_preserves_source_and_single_history_entry`,
  `value_selector_inheritance_unknown_values_and_reset_are_lossless`,
  `value_selector_state_is_isolated_between_workspaces`, and
  `value_selector_rejected_confirmation_keeps_drafts_and_selection` (1 each).
  The last case uses real numeric-input typing before testing blocked choice
  confirmation; hidden programmatic input updates are not used as typing proof.
- Fields, deployment, tokens and Git remotes now use retained searchable Select.
  Saved IDs and provider keys are separate from labels; unknown choices remain
  visible and rejected confirmations reconcile through the existing guards.
- Final focused `value_selector_` passes (10). Added real provider/token-source,
  duplicate-name/renamed-ID, removed-selection and busy/immutable-identity cases.
  New tests failed before repairs for unchanged-refresh query loss, stale Reset
  selection and unchanged token-provider/source confirmation clearing drafts.
- Multiple Combobox replaces the two guided list menus beside raw textareas.
  `choice_combobox_` passes (4): exact history and Escape, invalid drafts,
  case-sensitive dependencies/case-insensitive viewports, stale bindings,
  unavailable-only values and scalar preservation. Discovery must finish before
  unavailable viewport choices are shown; that ordering has regression coverage.
- Existing pointer viewport tests pass (1 each); guided choices and setting
  search pass (1 each). Full Git UI passes (18), deployment UI (8), token UI (3),
  and root overlay/release checks (2). Headless smoke checks pass (18 modules,
  49 helpers, 45 dispatch/cleanup cases). An aggregate run was stopped during
  deployment after review findings; the deployment module was then rerun and passed.
- Native default initially found visible but unnamed option rows. After three
  bounded attempts, the owner authorized a public-API adapter investigation.
  Windows takes a text Label's native name from its value. Shared `ValueChoice`
  now supplies `Role::Label` and `aria_value(title)` for all picker items, without
  changing the toolkit or weakening the name/pointer assertions.
- Final native default, Git, Publishing, Deployment and Schema flows pass.
  Publishing was rerun after screenshot review found menu-before-dialog captures;
  waiting for Add named token fixes the actual compact/large light/dark shots.
  Preferences/catalogs restored; synthetic credentials/local Git fixtures only.
- Final single-selector tests pass (10); post-adapter remote tests pass (19).
  Deployment activity passes (2). Rustfmt, production GUI Clippy/build, editor
  diagnostics and headless harness checks pass. Guidance validation/self-tests
  pass; final status/maintenance lint is repeated before committing.
- Accounting: production +791/-201 (net +590); GUI tests +1844/-92;
  native flows +14/-6. Retained state adds per-field/panel selector entities,
  item/selection caches and subscriptions, plus a document-scoped map for the
  two guided list fields. Removed menu/action plumbing does not outweigh the
  controlled-state adapter; the approved maintainable-reuse policy permits growth.
- No new deferral. The existing Linux baseline exception and task 020 remain.
