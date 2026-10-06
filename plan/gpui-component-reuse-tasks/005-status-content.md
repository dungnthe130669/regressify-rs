# 005: Status And Content Components

Status: Not started. Depends on 001.

## Scope

R8 and matching content: StatusBar in [Source](../../src/desktop/ui/source.rs),
Marker for actionable passive notices, Tag for existing status labels, Badge
for existing dots, Separator, Label, message-only Alert, capture DescriptionList
and whole-pane Empty. Other owners include
[activity](../../src/desktop/ui/runs/activity.rs),
[settings](../../src/desktop/ui/runs/settings.rs),
[files](../../src/desktop/ui/files.rs), [updates](../../src/desktop/ui/updates.rs)
and [reports](../../src/desktop/ui/reports.rs).

## Ordered Work

1. Baseline the nearest `source_line_commands`, `external_reload`, `run_log_`,
   `update_progress_` and `image_previews_` tests for each touched surface.
   Require nonzero counts; load native UI and relevant domain skills.
1. Replace Source footer with StatusBar slots; preserve child roles, names, IDs,
   language, caret, completion and issue-navigation actions. Use normal sizing.
1. Use Marker for passive notices with actions and Alert only for suitable
   assertive errors. Preserve wording, severity, names, callbacks and guards;
   never make ordinary progress/log health repeated assertive announcements.
1. Adopt Tag only for existing labels, Badge in a nonzero host with accessible
   status wrapper, Separator only for decorative lines, and Label for matching
   text. Keep exact zero/large counters, focus associations, diagnostic selection,
   panel borders, image rulers and pointer guides unchanged.
1. Use DescriptionList with safe raw reference/test URL and copy children,
   signed dimensions and unavailable values. Use Empty only for whole-pane
   states, preserving real actions and accessibility wrappers.
1. Validate each small replacement immediately. Extend existing tests for
   live-region semantics, boundaries, unavailable/failed states, retained action
   keyboard behavior, metadata safety, empty actions and theme contrast.
1. Review screenshots, update docs/guidance/memory and record reduction/adapters.

## Acceptance And Checks

Run relevant known filters above with
`cargo test --locked --features gui-tests --lib <filter> -- --test-threads=1`.
Add missing semantics cases in existing modules with explicit new filters;
verify nonzero execution. Run `cargo fmt --all -- --check` and affected gates.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[results review](../../.github/skills/desktop-results-review/SKILL.md),
[updates](../../.github/skills/desktop-updates/SKILL.md),
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md) and
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md).
Commit this presentation/tests/maintenance only. Reopen for changed status
meaning, unsafe URLs, lost selectable text, live-region changes or weakened
action guards; component existence alone is not acceptance.
