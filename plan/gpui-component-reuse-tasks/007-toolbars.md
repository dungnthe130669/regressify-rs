# 007: Toolbars

Status: Not started. Depends on 001. Tasks 018 and 019 depend on this task.

## Scope

R4 bars in [editor](../../src/desktop/ui/mod.rs),
[Source](../../src/desktop/ui/source.rs), [runs](../../src/desktop/ui/runs.rs),
[log controls](../../src/desktop/ui/log_controls.rs), Capture Log headers and
[Git review](../../src/desktop/ui/git/view.rs). Normal toolkit sizes, visible
commands and wrapping are approved; no overflow-menu redesign.

## Ordered Work

1. Baseline relevant `source_line_commands`, `run_log_` and
   `desktop::ui::git::tests`; use `desktop::logs::tests` with the desktop feature
   when model behavior is involved. Record nonzero counts and load owning skills.
1. Pilot the editor Toolbar/ToolbarGroup. Use child/children for sized controls,
   content for spacers, labels, separators and custom wrappers. Immediately test
   keyboard/layout behavior and measure added adapters before propagation.
1. Migrate the remaining named bars individually. Retain every callback,
   tooltip/name, individual disabled condition, action context and title-bar
   mouse-down propagation guard. Toolbar disabled does not disable children.
1. Verify arrows/Tab roving focus, wrapping and visibility at minimum width.
   Keep hosted inputs trailing where needed; caret shortcuts remain with inputs.
   Do not hide commands, move title-bar policy or change menu behavior here.
1. Extend existing native keyboard, disabled-child and narrow-layout tests;
   review screenshots and update docs, skills and shared memory.

## Acceptance And Checks

Run applicable baseline filters with nonzero counts after each bar. Introduce
`toolbar_` in existing native tests for uncovered keyboard/wrapping behavior and
verify it actually runs. Run `cargo fmt --all -- --check` and affected feature
and native smoke gates. Validate actual input editing, not only wrapper focus.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md),
[diagnostics](../../.github/skills/logging-diagnostics/SKILL.md) and
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md).
Commit toolbar changes/tests/maintenance only. Reopen for stolen input keys,
hidden commands, lost operation guards or unsupported wrapping. Task 019 owns
final all-surface stock theme/sizing verification.
