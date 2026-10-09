# 019: Stock Theme And Normal Sizing

Status: Verified, awaiting the authorized local commit. Windows final gates and
native acceptance pass with owner-approved contrast-only color adjustments.
Depends on 001-018, all committed; task 018 is `ecbc562`.

## Scope

Stock Kit palette and UI typography with no brand-color overrides, normal
toolkit sizing throughout every desktop surface including existing controls.
Retain Source font preferences and specialized image/editor geometry. Start at
1000x640 and raise minimum only with evidence, never beyond 1200x720.

Owner-approved contrast exception: minimally adjust only colors required to
satisfy the unchanged 4.5:1 accessibility gate; all other styling stays stock.

## Ordered Work

1. Capture current layouts at 1000x640 and large size in light/dark themes.
   Inventory palette/fonts, compact overrides, minimum widths and specialized
   geometry, beginning at [design.rs](../../src/desktop/ui/design.rs).
1. Apply stock palette/typography and remove old normal-control size overrides
   surface by surface. Preserve System/Light/Dark preference behavior, Source
   display preferences and image/editor geometry. Immediately check the first
   changed theme/layout slice before continuing.
1. Use normal sizes for all buttons/inputs, not only new components. Wrap
   toolbars and keep commands visible; no overflow-menu redesign. Review forms,
   menus/dialogs, logs, lists, trees, tables, status and empty states.
1. Gather concrete overlap/clipping/inaccessible-control evidence at 1000x640.
   Increase minimum only to the smallest size resolving demonstrated failures,
   with a 1200x720 hard ceiling; do not raise it automatically.
1. Test focus/contrast, long text, scrolling and actual editable children in both
   themes/sizes. Inspect screenshots; no per-surface user approval pause needed.
1. Update docs/guidance/memory, record chosen minimum/evidence and normal-sizing
   coverage, and account for deleted overrides versus necessary adjustments.

## Acceptance And Checks

Run focused native layout tests for touched surfaces with nonzero counts,
`cargo fmt --all -- --check`, and affected feature gates. Use a current desktop
binary for all guarded relevant native smoke flows, with screenshots at selected
minimum and large size in light/dark. Headless layout tests do not prove rendered
pixels; record exactly which screenshots were inspected.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md) and owning
skills for any defects touched. Commit stock theme/sizing, evidence-based layout
changes, tests and maintenance only. Reopen for insufficient contrast or focus,
lost specialized geometry, or a minimum above the approved ceiling.

## Open Deferral From Task 013

Native Schema smoke at the compact width clipped the appearance control and
timed out finding Dark; disclosure checks themselves passed in light mode.
The owner approved selecting themes at a wide size before retaining all requested
compact/large body assertions and screenshots in Schema/default smoke.

This task owns the actual title-bar fit repair, native theme switching at the
chosen minimum without widening, and removal of the temporary smoke workaround.
The exception does not count as green compact-header acceptance and does not
authorize raising the minimum without the required layout evidence.

Task 014 also reproduced the Git long-status title-bar failure on committed
task 013: `git_large_tree_and_status_fit_compact_and_wide_windows` finds
`open-workspace` beyond `width - 95`. The owner retained the header exception.
Close it with the unchanged geometry assertion and native long-status coverage;
passing compact body tests is not a substitute.

Task 015 native Git smoke reached the compact conflict view, then could not
find Light because the appearance control was clipped. The owner approved
wide-only theme selection at that step, retaining both 1080x720 conflict captures
and all assertions. Remove the Git workaround alongside Schema/default, and
prove theme switching with the long conflict status at the chosen minimum.

## Pilot Blocker

- Theme-preference and contrast baselines each passed 1 test. Pre-change
  native Titlebar passed with four inspected light/dark captures at the
  requested 1000x640 minimum (1016x648 outer bounds) and 1600x1000. Compact
  header clipping is still visible; no minimum-size increase has been made.
- Removing application palette/font/radius overrides compiles but fails the
  unchanged `both_palettes_keep_text_readable_and_keyboard_focus_visible`
  filter: 0 passed, 1 failed, `secondary contrast 4.35 with dark=false`, below
  the required 4.5:1. This is the explicit insufficient-contrast reopen condition.
- Await an owner decision on palette policy; do not lower test thresholds or
  continue the normal-size/header migration silently. Exact commands and
  evidence are in the [ledger](README.md#task-019-verification).
- The pilot is uncommitted. Task 020 remains blocked; plans are retained.

The owner subsequently approved minimal contrast-only color adjustments while
retaining the existing accessibility gate and stock typography/sizing. Aggregate
all failing pairs before choosing adjustments; no brand palette is authorized.

## Integrated Progress

- Contrast now passes every original pair plus danger hover/active (1 test).
  Full Git UI passes (24). Native minimum-size switching passes in Titlebar and
  long-conflict Git flows without the removed widening workarounds.
- The normal-control sweep is implemented across the desktop.
  Focused counts and exact commands are in the [ledger](README.md#task-019-verification).
  Final Rust acceptance gates passed in the approved consolidated run.
- Owner-approved metadata test update retains 8px outer control gaps and adds
  the stock 10px text inset; exact URLs, copies, 160px height and alignment stay.
  Two-column paired URL layout passes the metadata regression (1).
- Owner-approved fit-width measurement repair pixel-aligns only side-by-side
  FitWidth pane widths. One-pixel tolerance, no-upscaling, zoom and scrolling
  remain unchanged. Full `image_previews_` passes (3), with neighboring checks.
- Window minimum is still 1000x640. Task 019 remains uncommitted; all plan files
  remain. Native evidence and remaining Rust gates are recorded below.

## Focused Resume (2026-10-09)

The authoritative full UI run was 271 passed, 3 failed (2872.10s), not the
delegate's inconsistent 24-test report. All three failures were offscreen-control
clicks. Measured wheel scrolling now brings each target into view without
changing production behavior or any existing behavioral assertion.

The three exact filters passed together: 3 passed, 0 failed, 1174 filtered,
16.69s after one 2m08s rebuild. Rustfmt and editor diagnostics pass. See the
ledger for the exact command. Further broad runs require owner approval; the
focused result is not a full/native acceptance claim. Changes remain uncommitted.

## Native Acceptance (2026-10-09)

Titlebar, Git, Schema, Default, Comparison (including error images), Artifact,
Publishing and Deployment flows have passing current-implementation runs, with
minimum/large light/dark captures. The last sidebar allocation change was then
verified by the focused resize/workspace-file tests and Artifact, Publishing and
Deployment on the rebuilt final binary; the other flow results predate that
sidebar-only change. Exact commands, retries and inspected images are in the
ledger. No real remote deployment or release occurred; cleanup guards remain.

Native findings repaired locally include long workspace-name header overflow,
stale/inaccessible Git tooltip text, and suite rows clipped after shrinking the
window. Fixtures use actual Tab navigation for dropdown focus and bring
scrollable targets into view before clicks. The owner approved the appearance
dropdown's Tab/Enter/Escape contract instead of unsupported InvokePattern;
ordinary command buttons retain Invoke checks.

Native acceptance was followed by the approved consolidated full Rust gate.
No task 019 commit or task 020 completion is claimed before their local records.

Final formatting, production GUI Clippy, guidance validation (46 skills), all
10 guidance self-tests and scoped Markdown checks pass. Read-only review found
no concrete defect. Production net +52 lines is separate from GUI tests (+528),
native flows (+302) and headless coverage (+187). Final broad Rust verification
still requires owner approval; it has not been silently skipped or marked green.

The owner approved one consolidated final Rust pass, shared with task 020 only
while the implementation remains unchanged. Per-task local commits and retained
plans remain required.

## Final Verification

- Default CLI: 524 passed, 0 failed, 23 ignored.
- Windows desktop models: 864 passed, 0 failed, 31 ignored.
- GUI all-target gate: 1,146 library tests passed, 0 failed, 32 ignored;
  worker integration: 11 passed, 0 failed, 4 ignored. Exact commands and timings
  are in the ledger; the GUI run was executed once for the final candidate.
- Runtime fingerprint was unchanged before/after verification. Production GUI
  build, Clippy, Rustfmt, guidance validation and 10 self-tests pass.
- Native flows and screenshots establish the unchanged 1000x640 minimum and
  close the header exception. Linux remains non-green under its separately
  approved baseline/CI disposition; Windows is not Linux evidence.
- Task 019 is verified and awaits its authorized local commit. Plans are retained.
