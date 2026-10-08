# 019: Stock Theme And Normal Sizing

Status: Not started. Depends on 001-018, including toolbar task 007.

## Scope

Stock Kit palette and UI typography with no brand-color overrides, normal
toolkit sizing throughout every desktop surface including existing controls.
Retain Source font preferences and specialized image/editor geometry. Start at
1000x640 and raise minimum only with evidence, never beyond 1200x720.

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
