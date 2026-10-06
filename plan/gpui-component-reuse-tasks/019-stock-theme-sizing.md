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
