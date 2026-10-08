# 014: Flat Collections And Row Presentation

Status: Committed as `23d805c`. Integrated UI: 255 pass, only the approved
task 019 Git header failure remains. Default/Schema/Comparison/Artifact native
flows pass. Depends on 001. Task 015 depends on this task.

## Scope

List/ListItem for [workspace files](../../src/desktop/ui/navigation.rs),
[captures](../../src/desktop/ui/reports.rs),
[action sequence](../../src/desktop/ui/sequence.rs), and suite/scenario row
presentation. Keep [lists.rs](../../src/desktop/ui/lists.rs) while its callers
need application selection operations.

## Ordered Work

1. Baseline `navigator_`, `workspace_files_`, `image_previews_` and neighboring
   sequence/selection tests with nonzero counts; read relevant skills.
1. Pilot workspace files: no duplicate search, stable paths, separate cursor
   and activation, inert binary rows, Home/End, scoped focus, virtualization and
   scroll-to-selection. Approved single-selection Up/Down may wrap.
1. Immediately validate before capture migration. Preserve capture identity on
   refresh, use `select_capture`, and dismiss approval through existing policy.
1. Migrate actions with stable action identity; preserve reorder/edit and draft
   guards. Never imply earlier actions passed just because a later one is edited.
1. Use ListItem presentation for checked suites/scenarios while retaining the
   application controller. Checked, focused and opened remain independent;
   preserve range/modifier/visible-selection policy and explicit activation.
1. Defer parent mutation if delegate callbacks would re-enter a borrowed list.
   General listeners and embedded children need guards independent of ListItem.
1. Extend existing tests for refresh/reorder/filter identity, Home/End/wrapping,
   input-focus exclusion, activation, binary inertness, approval dismissal,
   modifiers and async updates. Maintain docs/guidance/memory and code accounting.

## Acceptance And Checks

Run relevant filters with gui-tests and `--test-threads=1`, all nonzero. Add
missing surface-specific cases in existing modules, not a parallel suite. Run
`cargo fmt --all -- --check` and affected feature/native smoke gates; inspect
scroll and selection in both themes/sizes.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md),
[results review](../../.github/skills/desktop-results-review/SKILL.md) and
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md).
Commit this unit only when its surfaces are verified, or request an explicit
subunit split. Reopen for lost identity, virtualization, focus or selection
distinctions. SidebarMenuItem is not a keyboard-equivalent substitute by itself.

## Approved Test Adaptation

The owner approved replacing only the old pre-scroll assumption that every
offscreen row has bounds: the virtualized List may not mount the last row until
scrolled. Short lists must still fit completely; all exact post-scroll, scrollbar
drag, Home/End, activation and binary assertions remain. Added 64-row cases in
both themes/sizes. The final workspace filter passes 3 tests and the new
`flat_collection_workspace_` identity/guard filter passes 1.

The owner also approved adjacent acceptance repairs after five of six integrated
failures reproduced at committed task 013. Fields remount checks retain their
visibility assertions after the next layout frame; Run toolbar padding matches
the existing tab inset. The zoom fixture now identifies its five captures with
unique keys, while a separate duplicate-key test verifies deliberate refresh
fallback. Exact commands and results are in the bundle README. Only the existing
compact-titlebar exception remains assigned to task 019; do not suppress its
failing Git long-status geometry assertion.
