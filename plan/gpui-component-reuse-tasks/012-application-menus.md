# 012: Application Menu Bar

Status: Not started. Depends on 001.

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
