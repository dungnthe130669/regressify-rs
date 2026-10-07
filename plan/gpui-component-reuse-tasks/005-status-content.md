# 005: Status And Content Components

Status: Verified on Windows; ready for the authorized local commit. Requires task 001.
The approved Linux baseline exception remains unchanged.

## Scope

R8 and matching content: StatusBar in [Source](../../src/desktop/ui/source.rs),
Marker for actionable passive notices, Tag for existing status labels, Badge
for existing dots, Separator, Label, named message-only errors (the approved
Marker/Alert-wrapper substitution), capture DescriptionList and whole-pane Empty.
Other owners include
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
Commit this presentation/tests/maintenance and the explicitly approved native
harness repairs only. Reopen for changed status
meaning, unsafe URLs, lost selectable text, live-region changes or weakened
action guards; component existence alone is not acceptance.

## Implementation Evidence

- Source uses Kit StatusBar's left/right slots for the existing language,
  caret-position, completion, issue-navigation and clean-state children. Child
  IDs, status roles/names, conditional issue action and guards remain. Its
  decorative toolbar line and the Fields group-heading line use Separator;
  run options already use the task-004 titled GroupBox, not a standalone rule.
- External-file and update-ready notices use Marker for their action rows. The
  existing polite Role::Status wrappers retain their names; reload draft flush,
  confirmation, busy guard, update install guard, and primary-foreground
  contrast remain. Marker has no accessible-label builder in 0.7.1, so the
  wrapper remains intentional. No shimmer or assertive role was added.
- Overview and live activity status labels use outlined Tag with status-derived
  variants; Overview cells retain their fixed column and expose the exact value
  through Role::Status. Activity and Source diagnostic dots use Badge in their
  existing fixed-size hosts and labeled status wrappers. Deployment destination
  uses Label primary plus secondary text under the original full status name.
  Capture rows use Label for the existing scenario title and outcome/detail pair.
- Capture information uses a one-column, unbordered DescriptionList with
  existing stable value IDs and custom raw URL/open/copy children. It retains
  `image_page_url` scheme validation, raw reference/test distinction, signed
  dimensions and unavailable values. The extended hover test covers both
  window sizes, exact copied URLs, and rejected javascript/data URLs.
- Empty is used for the no-workspace Overview, command-palette no-match state,
  no-capture preview/list states, and no-match Run Settings pane. Existing
  polite status wrappers and Open Workspace action remain. Loading is not
  rendered as absence; inline absence labels and diagnostic selection remain.
  Empty preview IDs include the pane index: a shared ID caused a native
  duplicate-a11y-node panic when an empty suite rendered side-by-side panes.
  The existing suite-navigation regression now checks distinct empty panes;
  the complete native comparison flow passes that transition.
- The owner approved presentational Marker inside the existing named Role::Alert
  wrapper for message-only capture/approval errors. Public Alert with a neutral
  observer wrapper lost the native accessible error name; the replacement keeps
  one assertive region and passes native error-name checks. Source's selectable
  diagnostic renderer, run errors without an existing alert role, log-health warnings,
  counters, panel borders, image rulers and pointer guides remain unchanged.
- Focused post-edit filters passed with nonzero counts: `source_line_commands`
  (12), `external_reload` (8), `update_progress_` (1), `deployment_activity_`
  (2), `live_dashboard_tracks_parallel_scenarios_and_filters_failures` (1),
  `overview_suite_columns_align_with_headers_at_both_sizes` (1),
  `schema_issue_panel_scrolls_and_keeps_text_inside_compact_and_large_windows`
  (1), `setting_search_` (1),
  `run_settings_are_available_before_execution_and_validate_only_relevant_commands`
  (1), `scenario_information_hover_shows_metadata_and_copies_recorded_urls`
  (1), `results_show_scenario_names_instead_of_generated_capture_and_run_ids`
  (1), `results_filter_errors_and_load_image_comparison_modes` (1),
  `report_approval_pins_the_viewed_run_and_rejects_changed_output_identity`
  (1), `overview_empty_state_preserves_open_workspace_action` (1),
  `command_palette_keeps_disabled_actions_inert_and_handles_empty_search` (1),
  `run_log_` (7), `log_health_warning_` (1), and `image_previews_` (3).
- Final repaired-slice checks pass: `results_suite_navigation_is_isolated_and_preserves_drafts_during_refresh`
  (1) and `results_filter_errors_and_load_image_comparison_modes` (1).
- Native Schema smoke passed for the Source slice. Final default, Comparison
  and Deployment flows pass in fresh STA hosts using `pwsh -NoProfile -STA -File
  .\ci\desktop-smoke.ps1 -SkipBuild` with the corresponding optional
  `-ComparisonFlows` or `-DeploymentFlows` switch. Comparison covers both
  window sizes/themes, exact metadata/menu URLs, image gestures/pixels, error
  images, empty suites, and historical viewport approval. Deployment uses
  synthetic fixtures without remote publication. Preferences were restored.
- The owner approved harness repair without weaker assertions: balanced left
  and right clicks release in `finally`; clipboard-busy retries are bounded
  and reject other exceptions; a synthetic sentinel and clipboard sequence
  change precede exact text reads to avoid competing with GPUI's write.
  Headless checks pass for 18 modules, 49 helpers, 45 dispatch/cleanup cases,
  pointer cleanup, clipboard retry boundaries, missing/wrong writes and order.
  Failure-only pointer/bounds diagnostics do not disclose clipboard contents.
- The user confirmed moving the mouse during popup/tab failures. The complete
  undisturbed comparison run passes without a popup lifecycle or navigation
  assertion change. Native runs require untouched pointer/keyboard input.
- `cargo fmt --all -- --check`, production `cargo clippy --locked --features gui
  --lib -- -D warnings`, and `cargo build --locked --features gui --bin
  regressify-desktop` pass. The guidance validator passes for 46 skills and its
  self-test passes 10/10. Scoped `rumdl check --no-exclude` passes for all eight
  changed Markdown files; the existing MD004 `after-marker` config warning is
  not suppressed.
- Production UI code totals 368 added and 244 removed lines (net +124), including
  adapters, with no new application state/subscriptions. GUI tests are +86/-14;
  native harness helpers/flows are +90/-22 and headless harness tests +77/-3.
  Test/guidance changes are not counted as production savings. Correct toolkit
  reuse despite source growth is approved; the Alert substitution is explicit.
- User-facing workflows and config did not change, so `docs/desktop.md` remains
  unchanged. Screenshot review includes compact/large Source, capture metadata,
  named errors, empty suites, overview, run settings and deployment logs in both
  themes. No new overlap or blank rendering was found. Task 005 is verified;
  the final all-targets gates and remaining migrations belong to later tasks.
