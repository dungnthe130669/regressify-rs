---
name: desktop-native-ui
description: 'Use when changing GPUI desktop widgets, editor tabs, native dialogs, keyboard/pointer interactions, layout, theme, accessibility, or Windows smoke tests.'
---

# Native Desktop UI

## Start Here

[UI modules](../../../src/desktop/ui), [native tests](../../../src/desktop/ui/tests.rs), and [Windows smoke runner](../../../ci/desktop-smoke.ps1). Smoke helpers and feature flows live in [dot-sourced modules](../../../ci/desktop-smoke).

## Procedure and Invariants

1. Follow the pinned gpui-kit patterns, icons, themes, and existing control density. Keep blocking work in background tasks and notify dependent views deliberately.
1. Preserve unsaved-draft guards, tab identity, focus, keyboard selection, and disabled/confirmation states. Stable test IDs may need an outer wrapper when scrollbar helpers replace an element ID.
1. Use real pointer/keyboard tests for menus/dialogs; model tests alone cannot establish interaction behavior. Settle dialog animations with existing helpers.
1. Exercise compact/large windows, long labels, light/dark/system themes, scroll, clipboard selection, and accessibility names. Inspect actual rendered screenshots.
1. Windows smoke requires an unlocked interactive desktop and no other app instances because it temporarily changes shared preferences. Never bypass these guards or close the user's windows; report the prerequisite instead.
1. Smoke shortcuts use `Send-FixtureKeys <ConsoleKey> -Control -Shift` as needed; literal text uses `Send-FixtureText`. Keep modifier pairs and extended navigation flags in the shared native helper, and retain caret/control outcome checks. Start a fresh PowerShell session after changing inline `Add-Type` definitions.

## Checks

For smoke harness edits, run `./ci/test-desktop-smoke.ps1` in PowerShell 7 first. These [headless checks](../../../ci/test-desktop-smoke.ps1) cover module loading, switch dispatch, failure cleanup, native key-event sequences, and draft state without Windows UI or settings changes. Keep shared process and cleanup variables in the runner's script scope; do not replace dot-sourcing with child-scope script invocation or module imports.

```sh
cargo test --features gui-tests --lib desktop::ui::tests -- --test-threads=1
cargo test --features gui-tests --lib desktop::ui::git::tests -- --test-threads=1
```

Run the relevant `./ci/desktop-smoke.ps1` switch, such as `-SchemaFlows`, `-ComparisonFlows`, or `-GitFlows`; use `-SkipBuild` only with a current binary. Capture flows may require managed Chromium. Do not use browser automation as a substitute for this native UI's tests.

## Maintain

Update [desktop docs](../../../docs/desktop.md), matching smoke scenarios, and this skill. Comment toolkit workarounds and ownership rules only where non-obvious.
