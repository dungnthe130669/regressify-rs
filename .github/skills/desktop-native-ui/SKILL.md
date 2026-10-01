---
name: desktop-native-ui
description: 'Use when changing GPUI desktop widgets, editor tabs, native dialogs, keyboard/pointer interactions, layout, theme, accessibility, or Windows smoke tests.'
---

# Native Desktop UI

## Start Here

[UI modules](../../../src/desktop/ui), [list keyboard helpers](../../../src/desktop/ui/lists.rs), [native tests](../../../src/desktop/ui/tests.rs), and [Windows smoke runner](../../../ci/desktop-smoke.ps1). Smoke helpers and feature flows live in [dot-sourced modules](../../../ci/desktop-smoke).

## Procedure and Invariants

1. Follow the pinned gpui-kit patterns, icons, themes, and existing control density. Keep blocking work in background tasks and notify dependent views deliberately.
1. Preserve unsaved-draft guards, tab identity, focus, keyboard selection, and disabled/confirmation states. Stable test IDs may need an outer wrapper when scrollbar helpers replace an element ID.
1. Workspace text files use Source only, including breadcrumb navigation and Save As. Clear schema providers and switch to the toolkit's `text` highlighter when leaving YAML/JSON; keep binary rows disabled for pointer and keyboard activation. Cover this with the `workspace_files_` native tests.
1. Keep [Source line commands](../../../src/desktop/ui/source.rs) scoped to `RegressifySource > Input` and the actual source focus handle. Menus restore that handle through `action_context`. Preserve Tab completion, native undo/redo, exclusive end-line bounds, reversed selections, and JSON's no-comments rule. Test disabled menu items by pointer behavior; their wrapper may not expose a disabled-state value.
1. Scope list shortcuts to a retained list focus handle, excluding search and editor inputs. Reuse the owning selection methods; suite/scenario keyboard ranges and Ctrl+A use visible rows, while scenario selection preserves other suites. Keep single-selection lists single-select and operation locks intact.
1. For keyboard-scrolled rows, share `track_scroll` with an explicit scrollbar. Append scrollbar overlays after rows because `ScrollHandle::scroll_to_item` indexes direct children; the convenience scroll wrapper owns a separate handle. Virtualized lists use their uniform-list handle and filtered positions.
1. Use real pointer/keyboard tests for menus/dialogs; model tests alone cannot establish interaction behavior. Settle dialog animations with existing helpers.
1. New/Rename suite dialogs focus and select the filename stem after opening. Route Enter from inputs or the primary action to the guarded asynchronous commit; Cancel/X keyboard activation must remain cancellation. gpui-kit styled buttons own internal focus handles, so use a parent focus region for default-action routing. Keep X/Cancel/Escape/backdrop dismissal blocked while busy, and retain invalid input after failed writes. Test disabled controls through their effects when wrapper metadata omits the disabled flag.
1. Suite context actions and F2/Delete target the focused row, not the open editor or all checked rows. Keep `RegressifySuiteList` separate from inputs; no global F2 binding. Test Results targeting, dirty cancellation, independent filename/display-name edits, and confirmed artifact deletion with `desktop::ui::tests::suite_panel_`.
1. Exercise compact/large windows, long labels, light/dark/system themes, scroll, clipboard selection, and accessibility names. Inspect actual rendered screenshots.
1. Windows smoke requires an unlocked interactive desktop and no other app instances because it temporarily changes shared preferences. Never bypass these guards or close the user's windows; report the prerequisite instead.
1. Smoke shortcuts use `Send-FixtureKeys <ConsoleKey> -Control -Shift -Alt` as needed; literal text uses `Send-FixtureText`. Keep modifier pairs and extended navigation flags in the shared native helper, and retain caret/control outcome checks. Start a fresh PowerShell session after changing inline `Add-Type` definitions.
1. Source focus checks must compare the actual focused accessibility element, not the editor wrapper's `HasKeyboardFocus`. `-SchemaFlows` uses Alt+Tab to an owned temporary window, native return activation, and Enter/Tab/Escape without clicking the editor; keep the temporary window's cleanup in `finally`.
1. For smoke selections requiring an exclusive column-zero endpoint, finish with `Shift+Home` and assert the caret and exact edited text. Vertical key movement can land a character past the boundary; GUI regressions must exercise keys, not only `set_selected_range`.
1. Native pointer clicks synchronize compositor frames after movement and press; always release in `finally`. Back-to-back native mouse events can miss scenario clicks even when frame-stepped GPUI tests pass. Keep pointer sequencing and failure cleanup covered by the headless harness.
1. Remote management has a distinct accessible `Save remote` command so the form's submit action cannot be confused with the disabled Add/Rename toolbar controls. Test dropdowns through their popup items after they render. Keep activity below the diff pane so expanded logs cannot collapse file trees; compact tests must include expanded comparison rows, not only visible commit controls.
1. Publishing controls live in the global Publish menu. Bound long token-selector labels so Add/Edit/Remove remain inside the dialog. Landing forms retain an explicit scroll handle; offscreen inputs may not have accessibility nodes until scrolled into view. Use distinct landing deployment labels, settle password focus after native clicks, and wait for menu-driven form changes before asserting them.
1. Report and landing deployment dialogs expose the selected target's saved URL through `deploy-open-published` and `deploy-copy-published`. Use the native link style without an external icon, content-sized and left-aligned, with Copy beside it and visible for long URLs. Omit report suite listings but retain subset confirmation. Test disk reload, provider switching, pointer copy, and compact light/dark layouts.
1. Cloudflare target routing defaults to Public (production). Preserve advanced HTML/date/exclusion settings and masked credentials across edits. Keep expanded asset settings scrollable without losing the outer dialog accessibility ID; GPUI's scrollbar wrapper changes element type. `label()` in native tests reads the accessibility name when one overrides visible text. Hide public-link controls for successful private deployments.
1. Deployment activity uses the shared read-only log editor at full remaining height, not static tail lines. Keep the suite summary ellipsized and hide its capture-statistics row. Test native selection/copy with a clipboard sentinel, long lines, compact/large windows, and a distinct editor accessibility name. Runs Open Site belongs beside Open Report and resolves saved publication membership by output ID, not display name.

## Checks

For smoke harness edits, run `./ci/test-desktop-smoke.ps1` in PowerShell 7 first. These [headless checks](../../../ci/test-desktop-smoke.ps1) cover module loading, switch dispatch, failure cleanup, native key-event sequences, and draft state without Windows UI or settings changes. Keep shared process and cleanup variables in the runner's script scope; do not replace dot-sourcing with child-scope script invocation or module imports.

```sh
cargo test --features gui-tests --lib suite_name_dialog_ -- --test-threads=1
cargo test --features gui-tests --lib source_line_commands -- --test-threads=1
cargo test --features gui-tests --lib deployment_activity_ -- --test-threads=1
cargo test --features gui-tests --lib runs_open_site_ -- --test-threads=1
cargo test --features gui-tests --lib desktop::ui -- --test-threads=1
```

Run the relevant `./ci/desktop-smoke.ps1` switch, such as `-SchemaFlows`, `-ComparisonFlows`, or `-GitFlows`; the default flow includes pointer-focused suite/scenario/checklist shortcuts. `-SchemaFlows` also covers source line commands and the Edit menu. Use `-SkipBuild` only with a current binary. Capture flows may require managed Chromium. Do not use browser automation as a substitute for this native UI's tests.

`-ArtifactFlows` also exercises New/Rename filename focus and stem selection, Enter from inputs/focused Create, X cancellation, F2 rename, independent display-name editing in Rename and Fields, byte-preserving reference/history migration, and Delete cancellation/confirmation. Inputs and the deletion prompt need accessible names, not only visible text or test IDs.

`-PublishingFlows` covers automatic landing storage, Markdown/remote URLs, draft guards, local removal, named tokens, Cloudflare account inheritance, and both themes/window sizes. It backs up/restores global catalogs and cannot use KeepOpen.

`-DeploymentFlows` covers synthetic target URL reload, native link/adjacent Copy, Runs Open Site placement, exact deployment-log copying, and missing-token outcomes without remote publication. It preserves clipboard data around copy checks.

## Maintain

Update [desktop docs](../../../docs/desktop.md), matching smoke scenarios, and this skill. Comment toolkit workarounds and ownership rules only where non-obvious.
