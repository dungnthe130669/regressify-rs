# 015: Git Change Trees

Status: Committed as `55a6142`. Full Git UI: 23 passed, only the approved
task 019 header failure remains. Final native Git, production and guidance checks
pass with the public base adapter below. Depends on 001 and 014 (`23d805c`).

## Scope

Kit Tree for Changes and Comparison Changes in
[Git view](../../src/desktop/ui/git/view.rs). Application-owned checkbox policy,
aggregation, request identity, conflict/submodule exclusions and diff behavior
remain authoritative.

## Ordered Work

1. Baseline `desktop::ui::git::tests` with gui-tests; record nonzero counts and
   inspect operation/request ownership before editing.
1. Use two retained Tree states with group plus repository-path IDs. Migrate
   flattening/expansion/scroll. On set_items, restore cursor by ID and preserve
   expansion; never restore a stale index. Immediately test refresh identity.
1. Retain group Select All, folder aggregation and exclusions. During operations
   permit read-only cursor movement, folder expansion and opening other diffs;
   lock checkbox changes and mutation commands. Reviewed in-flight request
   identity and file selection must remain immutable.
1. Enforce mutation guards in application policy; disabled item appearance does
   not secure all keyboard paths. Provide scoped Home/End and approved Up/Down
   wrapping. Leaf activation remains distinct from cursor movement.
1. Reject stale diff responses when target changes. Extend existing tests for
   both groups, cursor/expansion refresh, locked pointer/keyboard mutations,
   in-flight read-only browsing, failures, immutable requests and stale reads.
1. Review concurrency/error paths, update docs/skills/memory, inspect screenshots
   and record removed render/flattening code versus adapter state.

## Acceptance And Checks

Run `cargo test --locked --features gui-tests --lib desktop::ui::git::tests -- --test-threads=1`
with nonzero count. Add needed regressions in that module; verify any new
`git_tree_` filter. Run `cargo fmt --all -- --check` and affected feature/native
Git smoke gates using isolated local repositories only.

Follow [Git review](../../.github/skills/desktop-git-review/SKILL.md),
[native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[worker lifecycle](../../.github/skills/desktop-worker-lifecycle/SKILL.md) where
process ownership changes, and
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md).
Commit tree/tests/maintenance only. Investigate and ask before deferring if
immutable requests, locked mutation or stale-read rejection cannot coexist with
read-only browsing. Never weaken existing concurrency assertions.

## Approved Adapter

The mounted two-tree release regression reproduced a self-retaining render
closure in published styled Tree 0.7.1, even without a menu. The owner approved
using its public base Tree with the same ListItem/scrollbar presentation. Kit
still owns hierarchy, expansion, navigation and virtualization; no toolkit source
or private API is used. The release regression passes with this adapter.

The task 019 header exception also covers Git smoke's compact conflict theme
switch: widen only to select Light, then restore 1080x720 before the unchanged
capture/assertions. This is not passing compact-header acceptance.
