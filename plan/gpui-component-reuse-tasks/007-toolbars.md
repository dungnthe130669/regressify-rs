# 007: Toolbars

Status: Verified on Windows; ready for the authorized local commit. Depends on 001.
Tasks 018 and 019 depend on this task.

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

## Implementation Evidence

- Task 006 is committed as `5133195`; no toolbar changes are committed.
- Baseline `compact_titlebar_preserves_editor_space_and_navigation_at_both_sizes`
  passes (1) using the GUI-test command options above.
- The editor pilot uses a normal-sized, auto-height Toolbar and three semantic
  ToolbarGroups. A border/observation wrapper remains. DropdownMenuPopover is
  not Sizable, so its triggers are explicitly prepared/sized and the wrappers
  use `content`. Original callbacks, menu contexts and per-control guards remain.
- The initial `toolbar_editor_` gate reported 1 passed and 1 failed: geometry
  passed at 1000x640 and 1600x1000, but Left from Add did not wrap. Native default
  smoke reproduced the failure. Raw key dispatch, dropdown key-context wrappers
  and a traversal-distance probe did not resolve it. The reduced two-button
  toolkit fixture passes Right/Shift+Tab but fails backward toolbar wrapping.
- The owner explicitly approved a scoped public-API focus adapter. Shared
  `design::ToolbarFocus` retains one non-tab-stop focus scope, disables only
  toolkit navigation, and chooses destinations from bounded forward tab order.
  It restores focus on failure, ignores modified arrows, yields to active
  toolkit popup contexts, and lets child input actions consume keys first.
  No toolkit changes or private APIs are used.
- `cargo test --locked --features gui-tests --lib toolbar_ -- --test-threads=1`
  passes all 5 tests: normal-size geometry/actions, disabled/busy controls,
  forward/backward wrapping, Tab and Space, hosted input caret/text, dynamic
  control removal/recreation, popup/modified-key isolation and window-close
  owner release. The input probe
  focuses its actual InputState; the temporary exclusion state/method proved
  unnecessary and was removed, as were the key-context wrappers.
- Rustfmt, production GUI Clippy and desktop build pass. A fresh-STA native
  default smoke passes the previously failing scoped Add-to-More Left check
  plus the complete editing/Results/run workflow; preferences/drafts were
  restored. Compact editor screenshot review found no overlap or hidden command.
- Source, Run, shared log controls, run/capture-log headers and Git command bars
  now use normal wrapping toolbars. Log search is trailing; fixed IDs, input
  entities, callbacks, confirmation/cancellation guards and archive semantics
  remain. Capture-log Reference/Test buttons retain explicit selected state.
- Source tools (1) and `source_line_commands` (12) pass after migration. Direct
  Run commands pass (1); editor/Source/Run bounds pass at 1000x640 and 1600x1000.
  Shared log filter/search/download passes (2), full `run_log_` passes (7), and
  capture-log source identity passes (1).
- Git diff content/copy/scroll/layout checks pass (3). Selected-file safety,
  remote management and activity focus each pass (1) after their bar migrations.
  The first full Git UI run reported 13 passed/3 failed. Duplicate padding in
  already-padded remote/commit sections and header-unaware comparison flex
  allocation reduced tree space; both are fixed with normal control sizes.
  The unchanged compact layout and review-tree regressions now pass. The long
  folder-selection fixture now uses actual wheel scrolling before target clicks;
  all conflict/submodule/disabled assertions remain and the focused test passes.
- Final `cargo test --locked --features gui-tests --lib desktop::ui::git::tests
  -- --test-threads=1` passes all 16 tests. Rustfmt, production GUI Clippy with
  `-D warnings`, desktop build and headless smoke checks pass.
- Fresh-STA native default, `-SchemaFlows`, `-GitFlows` and `-DeploymentFlows`
  pass with the current binary. Default verifies backward toolbar wrap, real
  log-search Left/Right caret edits and cleared search before leaving the log.
  Preferences/drafts were restored; deployment is synthetic and Git uses only
  temporary local repositories. No remote publication or installation occurred.
- Reviewed compact/large light/dark editor, Source, log, Run/deployment and Git
  screenshots. Commands remain visible, tree panes remain usable, and no new
  overlap or blank rendering was found. The existing window minimum was not raised.
- Production accounting: eight UI owners total +962/-826 (net +136), including
  the 78-line shared focus adapter. Main GUI tests add 419 lines, Git tests add
  15, and native default smoke adds 20; tests/docs are not counted as savings.
  Thirteen toolbar instances share the adapter, including both Git copy panes;
  one keyed non-tab-stop focus handle per rendered toolbar is retained. No new
  domain state, asynchronous subscription or third-party fork was introduced.
- User docs describe the keyboard behavior. Skills/shared memory and the local
  issue list record the approved compatibility adapter. No new task deferral
  remains; the approved Linux baseline exception and task 020 final gates persist.
- Final documentation gates pass: eight scoped Markdown files, guidance
  validation for 46 skills, and all 10 self-tests. The existing MD004
  `after-marker` configuration warning is unchanged and unsuppressed.
