# 008: Managed Context Menus

Status: Not started. Depends on 001.

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
