# 008: Managed Context Menus

Status: Verified on Windows; ready for the authorized local commit. Depends on 001.

## Scope

R5 suite menus in [navigation](../../src/desktop/ui/navigation.rs), then image
menus in [reports](../../src/desktop/ui/reports.rs). Preserve clicked identity,
focus, application state and invalidation behavior.

## Ordered Work

1. Baseline `suite_panel_`, `navigator_` and `image_previews_` as relevant;
   require nonzero counts and inspect refresh/target tests before editing.
1. Attach ContextMenuExt to actual suite-row triggers with explicit stable IDs.
   Preserve clicked path, Rename/Delete guards and action_context/weak ownership.
   A valid right-click updates focused row but not opened or checked selection.
1. Reconcile current right-mouse propagation with toolkit bubble handling;
   immediately test opening, Escape/focus return, edge positioning, repeated
   menus, reorder/refresh while open and workbench release.
1. Pilot image menus only after suite parity. Freeze capture/image identity,
   keep validated path/URL handling, cancel panning and dismiss on refresh,
   capture/mode change, Results exit and owner release.
1. Investigate generation-scoped trigger IDs or removal with a focused native
   reproduction before treating absent public dismissal as a blocker. If parity
   remains impossible, ask the user before retaining custom code. No toolkit
   edits, issues, fork or private-field dependency is authorized.
1. Extend existing tests, update docs/guidance/memory and account for deleted
   popup owners/subscriptions/mounting versus retained adapters.

## Acceptance And Checks

Run `cargo test --locked --features gui-tests --lib suite_panel_ -- --test-threads=1`
and relevant navigator/image filters, all nonzero. Add `context_menu_` cases in
existing modules when needed, verifying target identity, invalidation and owner
release. Run `cargo fmt --all -- --check` and affected native/smoke gates.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[results review](../../.github/skills/desktop-results-review/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md) and
[path safety](../../.github/skills/artifact-path-safety/SKILL.md).
Commit only verified menu migration/tests/maintenance. Record blocker evidence,
downstream impact and a recommendation before requesting a decision; independent
work may proceed only without violating dependencies or shared-file ownership.

## Implementation Evidence

- Task 007 is committed as `065c0df`; task 008 is uncommitted.
- Baseline `suite_panel_` passes (5). Suite rows now use ContextMenuExt with
  workspace/path/generation identity. The existing indexed observation wrapper
  remains for tests. Valid right-clicks update list focus in capture phase without
  opening/checking the suite; busy/running rows do not mount managed menus.
- Removed manual SuiteMenu entity/position/subscription state and popup mounting.
  The generation invalidates menus on catalog refresh and starting suite actions;
  callbacks retain weak workbench ownership and the clicked Entry.
- A new Escape test found focus remained on the disappearing managed popup.
  The owner approved a guarded public-API dismissal adapter. Its weak deferred
  callback retires the menu generation and restores list focus only if that
  dismissed popup still owns focus, preserving newer menus and Rename dialogs.
- `cargo test --locked --features gui-tests --lib context_menu_suite_ -- --test-threads=1`
  passes 2 tests for identity, focus, repeated opens, bounds, busy/refresh/reorder
  invalidation, Rename input focus, cancellation and owner release. Waiting for
  cross-suite asynchronous loading avoids asserting before Rename exists.
- Post-edit `suite_panel_` passes (5) and `navigator_` passes (7) with the same
  command options. The suite builder and callbacks also reject stale generations.
- Image scroll surfaces now use ContextMenuExt with mode/capture/refresh/Results
  visibility generations. The path/URL builder retains frozen validated targets;
  failed synthetic previews without a path still offer no external menu.
  Manual ImageMenu owner/position/subscription and mounting are removed.
- The existing `image_context_menus_offer_external_targets_without_switching_tabs`
  test passes with exact URLs, disabled legacy actions, Diff exclusions, all modes,
  both sizes, mode/capture/refresh/exit dismissal, Escape focus and owner release.
- Extended held-pan coverage found mixed Left-down/Right-up clicks reached the
  pane-activation callback and invalidated the menu. Mouse activation now requires
  both down/up to be Left; keyboard/touch remain unchanged. All 3 `image_previews_`
  tests pass, including cancelled pan, unchanged offsets and preserved three panes.
- Native Artifact passed for the initial suite candidate; Comparison passed for
  the image candidate before the final mixed-button guard. One comparison run
  stopped on crosshairs after the user moved the mouse; the undisturbed retry
  passed. A later missed Diff click was also interrupted by user pointer movement;
  no comparison assertion or timing change was applied, and the final retry passed.
- Native Artifact now explicitly checks right-click identity, Escape dismissal
  and restored F2 targeting. Headless harness passes (18 modules, 49 helpers,
  45 dispatch/cleanup cases). Final native Artifact and full Comparison pass with
  fresh STA hosts and the current binary. Preferences were restored; all fixtures
  are isolated and no real remote action occurred.
- The approved dismissal compatibility is consolidated in the 29-line
  `design::restore_managed_menu_focus` helper. Both suite lifecycle tests and the
  image action/lifecycle test pass after consolidation; the failed-reference
  pathless-menu guard also passes (1). Rustfmt, GUI Clippy with `-D warnings`,
  desktop build and editor diagnostics are clean.
- Reviewed suite popup and compact/large light/dark comparison screenshots.
  Action identity, error images, guide pixels, pan/zoom, approval scope and
  empty-suite transitions remain intact. No new overlap or blank rendering found.
- Accounting: five production owners are +311/-248 (net +63), including the
  shared helper; GUI tests are +367/-1 and native Artifact adds 15 lines. Manual
  SuiteMenu/ImageMenu owners, stored positions and mounting are removed. Two
  generation counters replace owner slots; one weak per-menu dismissal adapter
  subscription remains alongside toolkit-managed lifecycle.
- User docs, native/results skills, shared memory and the local issue list record
  the tested behavior. Task 008 is verified with no new deferral; the existing
  Linux baseline exception and task 020 final acceptance remain.
- Final required suite-panel filter passes (5). Documentation checks pass for
  eight Markdown files, 46 skills and 10 validator self-tests; the existing
  MD004 `after-marker` configuration warning is unchanged.
