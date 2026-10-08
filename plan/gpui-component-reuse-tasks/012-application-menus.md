# 012: Application Menu Bar

Status: Implementation and acceptance complete; authorized local commit pending.
Depends on 001.

## Scope

R9 File/Publish/Help in [title bar](../../src/desktop/ui/overview.rs),
[file commands](../../src/desktop/ui/files.rs) and
[Source commands](../../src/desktop/ui/source.rs). Preserve explicit document
targeting, source focus and platform behavior.

## Ordered Work

1. Baseline `suite_name_dialog_`, `suite_panel_`, `source_line_commands` and
   relevant publishing/deployment tests; require nonzero counts before editing.
1. Pilot AppMenuBar/owned menus. Populate the app-menu registry and reload the
   retained entity only when definitions/flags change, not every render. Keep
   names, commands, checked/disabled flags, busy state and normal-size fit.
1. Route File actions explicitly to the workbench/open document. Captured focus
   may belong to the suite list, whose FileCommand targets its focused row.
   Keep source edit actions scoped to the editor; do not access private context.
1. Immediately test menu switching, keyboard/hover behavior and exact targets.
   Extend existing tests for changed enabled flags, stale menu snapshots,
   suite-list versus open-document focus, unsaved cancellation, source edits,
   focus return and workbench release.
1. Preserve platform distinctions: Windows/Linux AppMenuBar does not establish
   macOS native menus. Review screenshots and update docs/guidance/memory.

## Acceptance And Checks

Run applicable baseline filters with
`cargo test --locked --features gui-tests --lib <filter> -- --test-threads=1`.
Introduce `app_menu_` in the existing native module for uncovered cases and
verify nonzero counts. Run `cargo fmt --all -- --check` plus affected gates
and native smoke. Test disabled items through actual pointer/keyboard effects.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md),
[landing apps](../../.github/skills/desktop-landing-apps/SKILL.md) and
[deployment targets](../../.github/skills/deployment-targets/SKILL.md).
Commit menu routing/presentation, tests and maintenance only. Investigate and
ask before deferring an exact-target, public-API or platform behavior blocker.

## Implementation Evidence

- Task 011 is committed as `521e14f`; task 012 is uncommitted.
- Baselines pass: suite-name dialogs (6), suite-panel targets (5), Source line
  commands (12), named-token workflow (1). The existing File/Help/About draft
  regression passes after integration (1).
- Windows/Linux uses retained AppMenuBar and owned File/Publish/Help menus.
  Enabled flags trigger registry reload only when changed. Dedicated AppFileCommand
  routes to the workbench/open document and revalidates current guards, avoiding
  the suite-list FileCommand handler. macOS keeps its previous presentation;
  this change does not establish a macOS native menu.
- The pinned bar/menu strong cycle failed a release probe. Public empty reload
  breaks the cycle in the workbench release callback, restoring the shared
  registry afterward. Open-window release and two-window isolation tests pass.
- Flag reload initially left focus behind. Synchronous public
  FocusHandle::dispatch_action with base::actions::Cancel restores the captured
  context before reload, only when the menu scope owns focus. The failed-before
  Source-focus regression passes without private toolkit access.
- Final `app_menu_` passes (5), existing File/Help/About (1), Source commands (12)
  and root lifecycle (2). Native default, Artifact and Publishing pass with the
  current binary, original guards and restored preferences/catalogs. Reviewed
  open File menu and compact dark token/header layouts; no overlap found.
- Production GUI Clippy/build and diagnostics pass; final Rustfmt and guidance
  checks run before commit. macOS fallback is preserved but not executed on this
  Windows host; the existing Linux baseline exception remains.
- Production accounting: +141/-25 (net +116), tests +352/-3. New state is the
  retained bar, one non-tab-stop focus scope and enabled-flag snapshot; public
  cleanup is eight lines. No toolkit changes or private action-context access.
- No new deferral. Task 020 still owns fresh final feature/platform gates.
