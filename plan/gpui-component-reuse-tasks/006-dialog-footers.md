# 006: Dialog Footers And Actions

Status: Verified on Windows; ready for the authorized local commit. Depends on 001.
The approved Linux baseline exception remains unchanged.

## Scope

R10 footer layout in [files](../../src/desktop/ui/files.rs),
[deployment](../../src/desktop/ui/deployment.rs),
[tokens](../../src/desktop/ui/tokens.rs),
[landing](../../src/desktop/ui/landing.rs), Git confirmation and update dialogs.
The bounded public-action pilot uses the non-busy About dialog in
[overview](../../src/desktop/ui/overview.rs), not application-specific save policy.

## Ordered Work

1. Baseline `suite_name_dialog_` and neighboring tests for each touched dialog.
   Record nonzero counts before edits; load relevant domain skills below.
1. Replace repeated footer alignment with DialogFooter and existing Buttons.
   Preserve wrapping, names, variants, disabled state and callbacks. Validate
   one footer immediately before converting the next.
1. Pilot default button props only for simple non-busy dialogs. New/Rename/Save
   and publishing retain explicit busy/disabled buttons and repeat-submit locks.
1. Keep the filename-dialog focus guard until Enter on input/primary versus
   Cancel/X and busy tests prove parity. Never use private DialogDispatchAnchor.
   Preserve Save/Discard/Cancel and inline approval as distinct policies.
1. Extend existing tests for pending saves, repeated submission, busy Escape/X/
   backdrop, errors and retry, keyboard order, cancellation and owner release.
1. Review relevant screenshots, update docs/guidance/memory and record code
   reduction including retained guards/adapters.

## Acceptance And Checks

Run `cargo test --locked --features gui-tests --lib suite_name_dialog_ -- --test-threads=1`
and touched-area native filters, all nonzero. Add missing footer/focus tests in
existing modules, explicitly identifying new filters. Run
`cargo fmt --all -- --check` and affected native/smoke gates.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md),
[deployment targets](../../.github/skills/deployment-targets/SKILL.md),
[landing apps](../../.github/skills/desktop-landing-apps/SKILL.md) and
[Git review](../../.github/skills/desktop-git-review/SKILL.md).
Commit footer presentation, tests and maintenance only. Investigate and ask
before deferring a public-API blocker; never weaken dismissal or save policy.

## Implementation Evidence

- Baseline `suite_name_dialog_` passes: 6 tests with the command above.
- The filename footer uses DialogFooter's normal row layout, keeping the
  original buttons, disabled states, callbacks and `file-confirm-focus` region.
  Immediate `suite_name_dialog_enter_activates_focused_create_and_save_buttons`
  passes (1); the final full `suite_name_dialog_` filter passes (6).
- All nine existing footer sites now use DialogFooter: three file dialogs,
  deployment, named tokens, landing, Git confirmation, updates and About.
  Landing/update wrapping and the named Git confirmation group remain.
  Explicit busy/state-dependent buttons and callbacks are unchanged.
- Baseline and post-edit focused checks pass (1 each):
  `unsaved_dialog_cancel_and_discard_preserve_their_contracts`,
  `suite_panel_context_rename_fields_and_delete_keep_artifacts_aligned`,
  `deployment_token_drafts_are_cleared_on_cancel_close_and_escape`,
  `named_token_manager_masks_saves_renames_and_confirms_removal`,
  `landing_drafts_guard_close_and_keep_remote_site_order`,
  `discard_selected_folder_requires_confirmation_and_reloads_the_open_document`,
  `update_dialog_preserves_the_draft_and_shows_build_status`, and
  `workspace_external_menus_and_about_show_version_without_changing_drafts`.
  The deployment subset/busy/unsaved-guard regression also passes (1).
- About pilots public DialogClose through its trigger builder. Its fixed inner
  `close` ID is scoped by the existing `about-close` wrapper; the wrapper's
  native group name is `About actions`. The smoke pointer click is scoped to
  that group so it cannot silently use the titlebar X.
- New `about_dialog_public_close_preserves_keyboard_focus_and_drafts` passes (1):
  Tab/Enter/Space/Escape, exact draft/saved bytes, focus return and workbench
  release with the dialog mounted. The existing About test now asserts actual
  pointer dismissal. Busy/multi-action dialogs keep explicit buttons rather
  than generic DialogAction/default props; no private dispatch API is used.
- Production GUI Clippy, Rustfmt, desktop build and headless smoke harness pass.
  Native `-ArtifactFlows`, `-PublishingFlows`, `-DeploymentFlows`, `-GitFlows` and
  `-UpdateFlows` all pass with `pwsh -NoProfile -STA -File
  .\ci\desktop-smoke.ps1 -SkipBuild <switch>`. Preferences were restored;
  publishing/deployment use isolated synthetic data, Git uses only temporary
  local repositories, and the unavailable-feed update check installs nothing.
- Inspected About, filename/deletion, compact landing/token/deployment, update
  and Git confirmation screenshots across the available light/dark sizes.
  Footer actions remain visible and aligned; no new overlap or blank view found.
- Seven production owners total +45/-47 (net two lines removed), including the
  retained Git group and About observation/accessibility adapter. Tests are
  +70/-0 and the native scope assertion is +1/-1. No new application state or
  subscriptions. No public-API blocker or new deferral remains in this task.
- User-facing workflow and configuration semantics did not change, so
  `docs/desktop.md` remains unchanged. The native UI skill and shared smoke
  memory document the verified composition and guard boundaries.
- Final documentation gates pass: five scoped Markdown files, guidance
  validation for 46 skills, and all 10 validator self-tests. The existing MD004
  `after-marker` configuration warning remains unsuppressed.
