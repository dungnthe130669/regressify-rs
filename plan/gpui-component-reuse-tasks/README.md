# GPUI Kit Component Reuse Tasks

Status: Planning and tasks 001-012 committed. Task 013 controlled groups passes
acceptance and awaits its authorized local commit. The approved Linux baseline
and task 019 compact-header exceptions remain in force.

This bundle translates the approved decisions in the [refined plan](../gpui-component-reuse.md) into separately reviewable implementation units. The refined plan remains the behavior authority; this directory does not copy or replace it. Task IDs and dependency order are stable.

## Authorization And Status

- Task 000 is the reviewed plan and task bundle; it has no separate task file.
- The user's interview authorizes one local commit per completed, verified task. The main agent owns integration and commits; a delegate's no-commit instruction does not change that authorization. No branch, push, release, credential change, live remote operation, toolkit edit, vendoring, fork, or plan deletion is authorized.
- Delegation of independent work is approved; shared files, builds and commits remain serialized. An implementation blocker requires the decision process in the implement-plan skill.
- Keep this bundle and the refined plan.
- Task 001 Windows upgrade gates passed; the separately approved Linux baseline exception remains pending CI.

## Baseline Evidence

Baseline revision: `be843359a593b19d155c82bdcea0ee163e015423`.

- `cargo test --locked --features gui-tests --all-targets -- --test-threads=1`: compilation failed before any tests ran due to disk exhaustion, OS error 112. No pass count or test count.
- `cargo check --locked --features gui --bin regressify-desktop`: failed during PDB link with `LNK1201` / OS error 112. No successful GUI compile.
- Storage recovery: the user authorized removal of unneeded build artifacts. Only the regenerable incremental cache was removed, recovering about 48.6 GiB of free space; binaries, fixtures, smoke artifacts, and screenshots were preserved. Use ephemeral `CARGO_INCREMENTAL=0` for this migration's build commands.
- Retried `cargo check --locked --features gui --bin regressify-desktop` with `CARGO_INCREMENTAL=0`: passed on 0.6.1. Full GUI baseline displayed a schema visibility failure; the terminal's final aggregate summary was not retained, so no aggregate pass count is claimed.
- The retained 0.6.1 test executable independently reproduced `schema_issues_have_precise_jumps_hover_help_and_keep_the_save_guard`: 0 passed, 1 failed, with diagnostic 3 offscreen. The 0.7.1 executable failed identically. The user authorized a test-only scroll repair that preserves every existing assertion.
- The approved target is frozen at exact published 0.7.1. Do not chase newer releases.
- Refined-plan lint: `rumdl check --no-exclude plan/gpui-component-reuse.md` passed with a pre-existing warning: MD004 has unknown option `after-marker`. No suppression/config change was made.

These are historical baseline facts, not evidence that the migration compiled or that Linux passed. Task 001's Windows upgrade gates are complete as recorded below; Linux remains an explicit exception.

## Coverage Matrix

- Planning/reconciliation: 000 (this README status only).
- Published Kit 0.7.1 dependency, lockfile, launch/root and fixture API compatibility: [001](001-upgrade-root.md).
- R1 progress and R11 binary Git file checkbox: [002](002-progress-checkbox.md).
- R2 numeric fields with lossless draft/step adapter: [003](003-numeric-fields.md).
- R3 Forms, Fields, normal GroupBox; run-options pilot then all identified forms: [004](004-forms.md).
- R8 StatusBar, Marker, Tag, Badge, Separator, Label, Alert, DescriptionList, Empty: [005](005-status-content.md).
- R10 dialog footer/action presentation: [006](006-dialog-footers.md).
- R4 toolbars, normal sizing, wrapping, keyboard and hosted inputs: [007](007-toolbars.md).
- R5 managed suite and image context menus/lifecycle: [008](008-context-menus.md).
- R6 single Select and multi Combobox value selectors: [009](009-value-selectors.md).
- R7 exact-text Clipboard controls and gated exceptions: [010](010-clipboard.md).
- R12 source TabBar and non-tail Pagination: [011](011-log-navigation.md).
- R9 File/Publish/Help application menu bar and explicit routing: [012](012-application-menus.md).
- Controlled Accordion/Collapsible error/search opening and collapse preference: [013](013-collapsible-groups.md).
- Flat workspace/capture/action collections and suite/scenario ListItem rows: [014](014-flat-collections.md).
- Git Changes and Comparison Changes Tree pilots: [015](015-git-tree.md).
- Overview Table plus full Details dialog; diagnostics remain variable-height: [016](016-overview-table.md).
- Command palette actionless Command, stock substring search and Escape sequence: [017](017-command-palette.md).
- Source editor font size NumberInput, integer range 10..24: [018](018-source-font-size.md).
- Stock theme, normal sizing on every desktop surface, window-size evidence: [019](019-stock-theme-sizing.md).
- Fresh final gates, smoke/platform coverage, documentation and reduction accounting: [020](020-acceptance.md).

Coverage includes all R1-R12, all Other Matching Components, and every collection/workflow pilot in the refined plan. Retentions (tri-state selection, keyboard breadcrumbs and guarded links, diagnostic escaping, line-operation policy, image comparison, split Git diff, bounded logs, domain state, and existing toolkit infrastructure) are explicit acceptance constraints in the linked plan and owning task files; they are not missing migrations.

## Dependencies And Execution Order

Default execution is ascending task ID, subject to these dependencies:

- 000: no dependency; planning and reconciliation status lives in this README.
- 001: depends on 000; establish fresh baseline and prove GUI/root compatibility before components.
- 002-012, 014, 016-017: depend on 001; every component migration follows the complete upgrade gates.
- 018: depends on 001, 003, and 007; Source font-size reuses numeric-input semantics and shares Source toolbar ownership.
- 013: depends on 001 and 004; controlled groups follow the form/field presentation migration they extend.
- 015: depends on 001 and 014; Tree uses collection identity and keyboard/selection contracts established by the flat-list pilot.
- 019: depends on 001-018; final stock theme/sizing review includes all adopted controls.
- 020: depends on 001-019; reconcile and verify every implementation task.

Tasks without a direct dependency may be delegated independently, but do not edit shared files concurrently. Execute focused checks and commits serially. A dependency does not authorize changing another task's scope.

## Implementation Checklist

- [x] 000 Planning: 36-answer interview, release/source review, requirement coverage, dependencies, and all 20 tasks reconciled; scoped Markdown checks pass. Committed as `278be67`.
- [x] 001 Upgrade and root API: 0.7.1 pin/lock, production/test roots and authorized compatibility fixes verified; Windows gates pass, Linux baseline exception recorded. Committed as `4809144`.
- [x] 002 Progress and binary checkbox: Kit Progress at both bars and binary Git-file Checkbox verified; aggregates remain mixed-state. Committed as `b8eb5c4`.
- [x] 003 Numeric fields: controlled styled NumberInput verified for drafts, inheritance, limits, history and native workflows. Committed as `3b57c38`.
- [x] 004 Forms: five form areas verified with focused tests, root lifecycle and native publishing/deployment/schema/default flows. Committed as `7bbf39b`.
- [x] 005 Status and content components: focused GUI tests, native flows,
  screenshot review and production gates pass. Committed as `888aade`.
- [x] 006 Dialog footers: all nine footers and the public Close pilot pass
  focused, native and screenshot checks. Committed as `5133195`.
- [x] 007 Toolbars: named bars, approved focus adapter and final native gates pass;
  committed as `065c0df`.
- [x] 008 Context menus: suite/image parity, guarded dismissal and final native gates pass;
  committed as `fa13c04`.
- [x] 009 Value selectors: single/multiple controls, guards and native acceptance pass; committed as `b77d5a6`.
- [x] 010 Clipboard: eligible controls, gated exceptions and acceptance pass; committed as `41fd5cc`.
- [x] 011 Log navigation: bounded pages, Follow and native keyboard acceptance pass; committed as `521e14f`.
- [x] 012 Application menus: exact targets, focus/lifecycle and native acceptance pass; committed as `6b0fbf8`.
- [x] 013 Collapsible groups: Fields/workspace/activity, focused/native checks and production gates pass; commit pending, task 019 header exception recorded.
- [ ] 014 Flat collections: not started.
- [ ] 015 Git Tree: not started; awaits 014.
- [ ] 016 Overview Table: not started.
- [ ] 017 Command palette: not started.
- [ ] 018 Source font size: not started; awaits 003 and 007.
- [ ] 019 Stock theme and normal sizing: not started; awaits remaining 003-018.
- [ ] 020 Final acceptance: not started; awaits remaining 003-019.

When a task changes state, record the exact focused command, exact outcome and nonzero test count, required gates, any approved deferral and its owner, and commit ID if one is authorized and created. Do not mark a failed, skipped, zero-test, or unavailable gate green.

## Verification Evidence

### Task 001 Verification

- `cargo check --locked --features gui --bin regressify-desktop`: passes with
  Kit/Base/Component 0.7.1 and GPUI Pre 0.3.8 after correcting the helper's
  result handling. No optional feature boundary was changed.
- `cargo test --locked --features gui-tests --lib root_overlay_ -- --test-threads=1`:
  2 passed, 0 failed; covers content identity, unique dialog mounting,
  pointer/Escape dismissal, focus restoration and owner release.
- `cargo test --locked --features gui-tests --lib schema_issues_have_precise_jumps_hover_help_and_keep_the_save_guard -- --test-threads=1`:
  1 passed after the authorized scrolling repair. Before repair it failed on
  both 0.6.1 and 0.7.1; no behavioral assertion was removed.
- `cargo fmt --all -- --check`: passed after scoped root/fixture formatting.
- `cargo tree --locked --features gui -i gpui-pre-wgpu`: no active dependency
  path on the Windows target; the lockfile entry does not enable that backend.
- The first full GUI run reported 1,079 passed, 1 failed, 32 ignored. Its one
  failure exposed 0.7.1 collapsing reverse ranges in `set_selected_range`.
  After a keyboard-driven reproduction, the owner approved forward-oriented
  selections after line commands while preserving selected text and exact
  CRLF/Unicode edits. The focused regression and final full rerun now pass.
- Native Git smoke exposed activity refresh focusing its read-only log. The
  owner approved a production fix: non-focusing range positioning. New
  `git_activity_refresh_preserves_editing_focus` failed before the fix and
  passes for name/URL/commit inputs with Follow on and off. Full Git smoke passes.
- `cargo test --locked --all-targets -- --quiet`: 524 passed, 23 ignored.
- `cargo test --locked --features desktop --lib -- --quiet` on Windows:
  863 passed, 31 ignored.
- Production GUI Clippy with `-D warnings` and desktop build passed, including
  final lint after the Git focus fix and removal of a redundant conversion.
- Headless smoke checks pass: 18 modules, 46 helpers, 45 dispatch/cleanup cases.
  Guidance self-tests: 10 passed. Guidance validation passes for 46 skills after
  correcting four pre-existing links to the moved plan directory.
- Native Windows default, Artifact, Schema, Comparison, Git, Publishing and
  Deployment flows all passed with the current upgrade binary. Authorized
  harness repairs use the current capture error artwork, bounded exact-clipboard
  waits, split Git diff assertions, and separate deployment error-banner versus
  structured archive assertions. One deployment attempt encountered clipboard
  restoration contention; subsequent complete flow passed with cleanup.
- Inspected compact/large light/dark Source and overview screenshots, compact
  schema/error dialogs, Git split diff, and landing forms. No blank rendering or
  new overlap found in those views. Stock theme/normal sizing remains task 019.
- Final `cargo test --locked --features gui-tests --all-targets -- --test-threads=1 --quiet`:
  1,081 library tests passed, 32 ignored; 11 worker tests passed, 4 ignored.
  The later Clippy-only removal of a same-type conversion passed production
  lint and the focused Git regression again; it changes no behavior.
- Code accounting: three explicit overlay-mount calls and manual Root wrapping
  were removed; fixture content capture now comes from Kit's return value.
  Reverse-selection branching was removed under the approved policy. Added
  state/subscriptions: none. Three native lifecycle/focus regressions and
  existing-test/smoke repairs are coverage, not counted as code savings.
- Task 001 is committed as `4809144`. Linux is not claimed green.

### Task 002 Verification

- Replaced run/update painted tracks with labeled Kit Progress, converting
  fractions to 0..100 and retaining outcome colors, download denominator guard,
  clamping and update lifecycle. Normal toolkit bar sizing is intentional.
- Individual Git-file selection uses styled Checkbox with unchanged paths and
  disabled conditions. Mixed folder/suite adapters remain. Added explicit
  comparison-group keyboard and no-file-open assertions to the existing test.
- Focused checks pass: `update_progress_` (1),
  `live_dashboard_tracks_parallel_scenarios_and_filters_failures` (1),
  `desktop::ui::git::tests` (16), and the extended
  `comparison_changes_require_explicit_selection_and_stay_separate` (1).
  Commands use `cargo test --locked --features gui-tests --lib <filter> -- --test-threads=1`.
- Rustfmt, production GUI Clippy with `-D warnings`, desktop build, editor
  diagnostics and headless smoke harness checks pass.
- Native default and Git smoke flows pass with the current binary. The new
  native assertion checks actual progress role/range/zero-total value. Reviewed
  run progress, file-checkbox tree and compact dark conflict screenshots.
- Initial update check exposed missing headless observation for styled Progress;
  retaining the existing observed wrapper fixed it. A headless accessibility
  snapshot probe was unavailable and removed; native accessibility now covers
  that contract without private toolkit APIs.
- Production accounting: 33 lines added, 46 removed across activity, update and
  Git-row renderers (net 13 removed), including the retained wrapper. No added
  state or subscriptions. Test/smoke additions are excluded from savings.

### Task 003 Verification

- Numeric Fields now use styled NumberInput at normal size, retaining the same
  InputState, explicit no-mask setting, Change subscription and `adjust_field`.
  Each numeric state calls public `set_step(None)` to disable Kit's default
  fixed step one; this corrected the proposal's false unset-step assumption
  after a failing inherited-value pilot and explicit owner approval.
- Focused native `numeric_stepper_`: 2 passed, covering inherited increments,
  actual editable child values, invalid/intermediate drafts after blur and
  rejected steps, exponent notation, Up/Down, busy state, undo/redo and upper
  limits. `pixel_match_threshold_`: 12 passed, including explicit zero/reset,
  schema/cascade/document/manifest contracts. `setting_search_`: 1 passed.
- The broad workbench run reported 135 passed and one obsolete wrapper-value
  assertion. That assertion now reads the actual Input child via its public
  element identity; focused `problem_links_focus_drafts_and_resets_preserve_other_invalid_fields`
  passes with every draft, focus and reset assertion retained. The broad result
  is not reported as a 136-test passing run; final acceptance owns a fresh run.
- Production GUI Clippy, desktop build and Rustfmt pass. Native Schema and
  default smoke pass; the first Schema attempt could not focus the desktop,
  then passed after the user confirmed availability. Preferences were restored.
  Inspected large/compact Fields and schema screenshots; no new overlap found.
- Production accounting: 36 lines added, 42 removed in the field owner (net 6
  removed), including one retained Step subscription per numeric field and
  stable labeled wrappers. No new document state or parsing/history policy.
  Test additions and selector repairs are not counted as production savings.

### Task 004 Verification

- Adopted Form/Field (aliased `FormField`) in run settings, configuration
  Fields, deployment target editing, named-token editing, and landing app/site
  editing. Run-option categories and landing sites use normal unframed
  GroupBox. Configuration's controlled/collapsible sections remain application
  owned for task 013.
- Preserved state ownership and stable input/test/accessibility IDs, raw input
  entities, provider-dependent fields, password masking, hidden errors, reset
  callbacks, inherited-value hints, save/cancel locks, landing site action IDs,
  outer deployment/landing IDs, and the landing scroll handle. Long structured,
  Markdown, URL, and Cloudflare exclude-path controls span the form width where
  appropriate.
- Pre-edit baselines passed: `run_settings_are_available_before_execution_and_validate_only_relevant_commands` (1) and `setting_search_` (1).
- Focused native checks passed with `CARGO_INCREMENTAL=0`: run settings (1),
  `setting_search_` (1), `problem_links_focus_drafts_and_resets_preserve_other_invalid_fields` (1),
  `numeric_stepper_` (2), `pixel_match_threshold_` (12), deployment target
  masking/storage (1), Netlify provider scoping (1), Cloudflare settings/token
  preservation (1), deployment secret draft cancellation (1), named-token
  masking/save/rename/removal (1), landing draft guard (1), landing site
  action/scroll (1), landing managed save path (1), and saved URL reopen/copy
  (1). Each filter ran serially with `--test-threads=1`.
- Rustfmt, production GUI Clippy with `-D warnings`, desktop GUI build, guidance
  validator/self-tests (46 skills/10 tests), and scoped Markdown lint passed.
  Parent `root_overlay_` checks pass (2). Native Publishing, Deployment, Schema
  and default flows pass with the current binary. One Deployment attempt hit
  existing clipboard restoration contention; the complete retry passed.
- Screenshot review caught duplicate required asterisks and unnecessary inline
  help in Fields. The final renderer uses one toolkit required marker and keeps
  the prior help presentation. `setting_search_` passed again after the repair,
  followed by a rebuilt binary and passing default smoke. Inspected compact
  Fields/run settings, landing, token and deployment views across themes.
- Five Rust owners show 207 insertions and 125 deletions overall
  (+82 net, including the landing interaction assertion). Form/grid wrappers
  were removed, but adapters and toolkit field composition mean this task is
  not a net source-reduction win. No new state or subscriptions were added.
- User-facing workflow contracts did not change, so `docs/desktop.md` remains
  unchanged. Owning skills and shared source/publishing memory record the new
  presentation-only contract.
- Task 004 is committed as `7bbf39b`. The Linux baseline exception is unchanged.

### Task 005 Verification

- Adopted StatusBar, Marker, Tag, Badge, Separator, Label, DescriptionList and
  Empty in the scoped presentation owners, preserving status meanings, exact
  counts, raw safe URLs, stable names/actions and application state. Detailed
  filters and scope are in [task 005](005-status-content.md).
- Owner-approved exception: capture/approval errors use presentational Marker
  inside their original named Role::Alert wrappers. Styled Alert with a neutral
  observer wrapper lost the native error name; the final replacement keeps one
  assertive region and passes native error-name checks.
- Empty preview wrappers are keyed by pane index. The original shared ID
  caused a native duplicate-a11y-node panic on empty side-by-side suites.
  `cargo test --locked --features gui-tests --lib results_suite_navigation_is_isolated_and_preserves_drafts_during_refresh -- --test-threads=1`
  passes (1), as does `results_filter_errors_and_load_image_comparison_modes`
  (1) with the same command options. Full native comparison also passes the
  formerly crashing empty-suite transition.
- Approved harness repairs retain every exact URL/pixel/action assertion:
  balanced frame-synchronized clicks, bounded clipboard-busy-only retries,
  synthetic sentinels, and sequence-change waits before reading exact text.
  `./ci/test-desktop-smoke.ps1` passes: 18 modules, 49 helpers, 45 dispatch/cleanup
  cases, plus pointer cleanup and clipboard ordering/failure regressions.
- The user confirmed moving the mouse during popup/tab failures. An undisturbed
  `pwsh -NoProfile -STA -File .\ci\desktop-smoke.ps1 -SkipBuild -ComparisonFlows`
  passes both sizes/themes, metadata/menu copies, pixel/gesture checks, empty
  suites, historical viewport approval and error images. No popup lifecycle
  change or navigation assertion relaxation was needed.
- Final default and `-DeploymentFlows` native checks pass in fresh STA hosts;
  Schema previously passed for the unchanged Source slice. Preferences and
  clipboard were restored, with synthetic deployment fixtures and no remote
  publication. Inspected compact/large light/dark screenshots for Source,
  metadata, errors, empty panes, overview, run settings and deployment logs.
- Final Rustfmt, production GUI Clippy with `-D warnings`, and desktop GUI build
  pass. Guidance validation passes for 46 skills; self-tests pass 10/10. Scoped
  Markdown lint passes for eight files with only the existing MD004 config warning.
- Accounting: nine production UI owners are +368/-244 (net +124), including
  semantic adapters and no new state/subscriptions. GUI tests are +86/-14;
  native helper/flow changes are +90/-22 and headless tests +77/-3. The approved
  correct-reuse-over-net-reduction decision applies.
- Task 005 is committed as `888aade`. No additional
  task deferrals were introduced; the Linux baseline exception remains below.

### Task 006 Verification

- Baseline `cargo test --locked --features gui-tests --lib suite_name_dialog_ -- --test-threads=1`:
  6 passed, covering input/button Enter, cancellation, busy dismissal/submission,
  and failed-write retry.
- The filename dialog now uses DialogFooter with its original explicit buttons
  and `file-confirm-focus` wrapper. Immediate focused
  `suite_name_dialog_enter_activates_focused_create_and_save_buttons` passes (1)
  with the same command options; the final full filename filter passes (6).
- All nine footer sites are converted with explicit busy/confirmation callbacks
  retained. Baseline/post-edit owner regressions pass as listed in
  [task 006](006-dialog-footers.md). About's public DialogClose pilot preserves
  its application wrapper and passes pointer, keyboard, focus/draft and owner
  release checks. Native smoke now scopes Close to its `About actions` group.
- Rustfmt, production GUI Clippy/build and the headless harness pass. Native
  Artifact, Publishing, Deployment, Git and Update flows all pass using fresh
  STA hosts and the current binary. Each native flow tests About's actual footer
  trigger; source/draft guards, named-token and landing workflows, reviewed Git
  operations and unavailable-update controls remain intact. Preferences were
  restored; no real remote deployment or update installation occurred.
- Inspected compact/large light/dark dialog screenshots; footer actions remain
  visible and aligned. Production owners are +45/-47 (net two lines removed),
  tests +70/-0, native helper +1/-1, with no new state/subscriptions. Explicit
  busy/multi-action callbacks and semantic wrappers are intentional adapters.
- Task 006 is committed as `5133195`. The existing
  Linux baseline exception is unchanged; full final gates remain task 020.
  Scoped Markdown lint passes for five files; guidance validation passes for
  46 skills and self-tests pass 10/10, with only the existing MD004 config warning.

### Task 007 Verification

- Editor/titlebar baseline passes (1). The first editor Toolbar/ToolbarGroup
  pilot retains normal size, automatic height, all commands and operation guards.
  Non-Sizable dropdown wrappers use `content` with prepared/sized triggers.
- `cargo test --locked --features gui-tests --lib toolbar_editor_ -- --test-threads=1`:
  initially 1 passed, 1 failed. Geometry/actions pass at both supported fixture sizes;
  Left from the focused Add trigger does not wrap. A key-context wrapper probe
  did not resolve it. This is not yet a proven toolkit defect.
- Native default smoke and a reduced two-button toolkit fixture reproduce the
  backward-wrap failure. The owner approved a public-API focus adapter in
  `design::ToolbarFocus`, using bounded forward-order traversal with scoped
  destinations, child-first handling, modifier guards and no toolkit changes.
- `cargo test --locked --features gui-tests --lib toolbar_ -- --test-threads=1`
  now passes 5 tests, including the unchanged failing assertion, disabled/busy
  actions, hosted input editing, popup/modified-key isolation, dynamic controls
  and owner release. An actual
  input focus handle is required in the input fixture; no exclusion list remains.
- Rustfmt, production GUI Clippy/build and fresh-STA native default smoke pass.
  The native Add-to-More Left assertion passes with exact editing/Results/run
  checks and restored preferences/drafts. Screenshot review passes.
- Remaining named bars are converted. Focused Source (12 line-command tests and
  1 tools test), Run (1), logs (7), capture-log (1), Git diff (3), remote/selection
  and activity-focus checks pass; details are in [task 007](007-toolbars.md).
  The initial combined Git gate had 13 passes/3 failures. Duplicated Git toolbar
  padding and comparison-header flex allocation are repaired without shrinking
  controls. A virtualized long-folder fixture now scrolls actual targets before
  clicks, retaining every assertion. Final full Git UI gate passes 16/16.
- Final Rustfmt, production GUI Clippy/build and headless harness pass. Fresh-STA
  native default, Schema, Git and Deployment flows pass. Default also verifies
  exact log-search caret edits and clearing. Preferences/drafts were restored;
  no real remote publication or installation occurred. Reviewed compact/large
  light/dark toolbar, log and Git screenshots without new overlap or hidden actions.
- Accounting: production +962/-826 (net +136), including a 78-line shared public
  focus adapter; main GUI tests +419, Git tests +15, native smoke +20. Thirteen
  toolbar instances use one keyed focus handle each; no new domain state or
  subscriptions. Correct reuse despite net source growth is approved.
- Task 007 is committed as `065c0df`. No new deferral was
  introduced; the non-green Linux baseline exception and task 020 remain.
  Scoped Markdown lint passes for eight files, guidance validation passes for
  46 skills, and self-tests pass 10/10 with only the existing MD004 config warning.

### Task 008 Verification

- Baseline `suite_panel_` passes (5). Managed suite-row ContextMenuExt replaces
  the manual popup owner, position/subscription storage and deferred mounting.
  Workspace/path/generation trigger identity, weak callbacks and capture-phase
  list focus preserve clicked identity without opening/checking the row.
- Published managed-menu dismissal left focus on the disappearing popup after
  Escape. The owner approved a guarded deferred dismissal adapter: only when
  that menu still owns focus, retire its generation and restore the suite list;
  newer menus/dialogs keep their focus. No toolkit source or private API changes.
- `context_menu_suite_` passes (2): focus/checked/open identity, repeated opens,
  bounds, refresh/busy/reorder invalidation, Rename input focus and owner release.
  Rename's cross-suite asynchronous open is awaited before input assertions.
- Final suite-panel regression filter passes (5); `navigator_` passes (7).
- Image menus use managed, generation-scoped scroll triggers and frozen safe
  path/URL actions. The existing image-menu test passes with source/mode/refresh/
  Results-exit invalidation and owner release. A mixed-button click guard fixes
  Left-held/Right-release incorrectly opening a pane; all 3 image-preview tests
  pass with interrupted-pan coverage.
- Native Artifact and undisturbed Comparison passed intermediate candidates;
  final current-binary reruns also pass after the last guards. The user confirmed
  pointer movement in failed crosshair/tab-click attempts; unchanged assertions
  pass undisturbed. Native Artifact now explicitly
  verifies right-click identity, Escape and F2 focus, and headless harness passes.
- Shared `restore_managed_menu_focus` consolidates the guarded weak/deferred
  adapter. Final suite lifecycle (2), image lifecycle (1) and pathless failed-image
  guard (1) pass. Rustfmt, production GUI Clippy/build and editor diagnostics pass.
  Native preferences are restored; screenshots show intact menus, comparison
  gestures/pixels, error images, approval scope and empty-suite behavior.
- Accounting: production +311/-248 (net +63), including the 29-line helper;
  GUI tests +367/-1 and native Artifact +15. Two generation counters replace
  manual popup owner slots, and one weak per-menu focus-repair subscription remains.
- Task 008 is committed as `fa13c04`. No new deferral;
  the approved non-green Linux baseline and task 020 final gates remain.
  Final suite-panel filter passes (5), scoped Markdown lint passes (8 files),
  guidance validates 46 skills, and self-tests pass 10/10.

### Task 009 Verification

- Baseline `setting_search_` passes (1). Guided Choice fields now retain Select
  state by workspace-root/document/selection/field identity. Programmatic item
  and value synchronization does not write; user Confirm uses the existing
  guarded edit path and reconciles rejected choices to authoritative values.
- `value_selector_` passes (10): lossless inheritance/unknown values, same-value
  confirmation, stable workspace/field/UUID identity, real search and provider/
  token/target choices, and rejected busy/immutable/draft changes. Regressions
  failed before fixes for query loss on unchanged rebuild, stale inherited Reset
  display and secret clearing when confirming an unchanged provider/token source.
- `choice_combobox_` passes (4), including one undo per accepted toggle, Escape
  without rollback, retained search, scalar/unknown-only values, invalid raw
  drafts, case matching, last-viewport rejection and stale binding isolation.
  Viewport discovery completes before unavailable choices are classified.
- Existing pointer viewport checks pass (1 each); guided choices and setting
  search (1 each), full Git UI (18), deployment UI (8), token UI (3), and root
  lifecycle (2) pass. Production GUI Clippy passed before the final loading guard;
  the final gates and native candidate are pending. Headless harness passes
  (18 modules, 49 helpers, 45 dispatch/cleanup cases).
- Default smoke now toggles multiple items in one popup, then closes with Escape.
  Native unnamed-row failures led to an explicit owner-approved bounded public
  adapter investigation. The shared item renderer supplies Label role and value,
  which Windows uses for native names. All five affected native flows now pass:
  default, Git, Publishing, Deployment and Schema. No toolkit edits or weakened
  assertions. Publishing screenshot readiness now waits for the actual dialog;
  reviewed token/provider/viewport/Git screenshots show intact control bounds.
- Final `value_selector_` passes (10), `remote_` passes (19), and deployment
  activity passes (2). Rustfmt, final GUI Clippy/build, diagnostics and headless
  harness pass. Guidance validation/self-tests passed before this status update;
  final maintenance checks repeat before commit.
- Production accounting is +791/-201 (net +590), GUI tests +1844/-92, native
  flows +14/-6. Item caches, retained entities and guarded subscriptions replace
  manual menu plumbing; correctness/maintainability was explicitly prioritized
  over net source reduction. Task 009 is committed as `b77d5a6`.
  No new deferral; the Linux exception is unchanged.

### Task 010 Verification

- Toolkit Clipboard now handles saved deployment URLs, Git activity and both
  source-diff sides, update failure details and capture summaries. `CopyText`
  keys feedback by exact-value generation without text-bearing IDs; it retains
  stable observation/name, size, summary tooltip and weak copied-status callback.
- Gated empty/incomplete run/deployment/capture log copies/exports and Results
  URL/menu copies remain guarded Buttons because Clipboard lacks disabled state.
  No availability guard or secret boundary changed.
- Focused checks pass: saved URLs (1), `clipboard_copy_` (1), Git text diff (3),
  update error details (1), capture summary/name (1), source line commands (12),
  image previews (3), run logs (7), deployment activity (2), Git remote/activity
  workflow (1). Tests exercise keyboard/pointer, exact/changed/revisited text,
  unchanged suppression, missing URLs, and preserved error/export guards.
- Production GUI Clippy/build, Rustfmt, diagnostics and headless harness pass.
  Native Deployment, Git, Comparison and offline Update pass with restored
  preferences/clipboard and unchanged assertions. Update failure copy bytes are
  covered by the GUI test; no live update/install or remote publication occurred.
  Reviewed compact saved-URL and both diff-side controls fit without overlap.
- Production +98/-61 (net +37), including the 72-line CopyText wrapper; GUI tests
  +103. Feedback/timers remain toolkit-owned; guards remain application-owned.
  Task 010 is committed as `41fd5cc`, with
  no new deferral. The approved Linux exception and task 020 remain.

### Task 011 Verification

- Baselines pass: reader (5), run logs (7), health warning (1). Capture Log now
  uses segmented source tabs and compact Pagination; run archives use non-tail
  Pagination and hide numbering during rolling Follow.
- The owner approved two public tab-stop handles and Enter/Space activation
  after reproducing Kit 0.7.1's pointer-only Tab behavior. Existing toolbar
  arrows handle traversal. Actual-handle keyboard and rapid-source tests pass.
- The bounded reader clamps/aligned pages in one pass with at most 500 records;
  checked page arithmetic and post-load offset synchronization keep model/display
  consistent for empty, filtered or shrinking totals. Follow-off loads the last
  aligned page and invalidated in-flight loads can be superseded safely.
- Focused paging tests pass (2), capture source/page test (1), full reader (6),
  run logs (7) and health warning (1). Partial page counts retain export guards.
  Headless harness passes with new native Right/Enter and Left/Space assertions.
- Native default passes after the user-requested retry of an interactive-focus
  prerequisite failure. Native Deployment passes after an unchanged retry of
  a clipboard-restoration failure; cleanup assertions were not relaxed and no
  clipboard contents were logged. Reviewed log tabs/page status and compact layout.
- Rustfmt, GUI Clippy/build, diagnostics and headless harness pass; final guidance
  validation follows the maintenance update. Production +145/-96 (net +49),
  tests/helpers +280/-9, native checks +17. Two focus handles are the only added
  retained navigation state; bounded page loading reuses existing query/generations.
  Task 011 is committed as `521e14f`. No new deferral; Linux exception
  and task 020 remain.

### Task 012 Verification

- Baselines pass: suite dialogs (6), suite targets (5), Source commands (12),
  named tokens (1). Existing File/Help/About draft test passes after integration.
- Retained Windows/Linux AppMenuBar uses owned menu snapshots and change-only
  enabled-flag reloads. Dedicated AppFileCommand bypasses focused suite-row
  targeting while revalidating open-document guards. macOS keeps its old UI.
- Public empty reload resolves a reproduced menu/bar ownership cycle on release
  and restores the shared registry. A scoped synchronous Cancel before flag
  reload resolves reproduced Source-focus loss; no private context or toolkit edit.
- Final menu cases pass (5), File/Help/About (1), Source commands (12) and root
  lifecycle (2). Native default, Artifact and Publishing pass; reviewed open-menu
  and compact dark header/dialog screenshots remain intact. Preferences/catalogs
  restored, no remote publication or toolkit changes.
- GUI Clippy/build and diagnostics pass; final Rustfmt/maintenance checks follow.
  Production +141/-25 (net +116), tests +352/-3; state is one bar, non-tab-stop
  focus scope and enabled-flag snapshot, with an eight-line cleanup helper.
  macOS fallback remains unexecuted on Windows. Task 012 is committed as `6b0fbf8`;
  no new deferral, and the Linux exception/task 020 remain.

### Task 013 Verification

- Resumed from clean `dc64241`; task 012's `6b0fbf8` commit and acceptance body
  verified independently of the stale pending-commit notes.
- Pre-edit baselines with ephemeral `CARGO_INCREMENTAL=0` pass:
  `setting_search_` (1), `navigator_` (7), `run_log_` (7), and
  `live_dashboard_tracks_parallel_scenarios_and_filters_failures` (1), using
  `cargo test --locked --features gui-tests --lib <filter> -- --test-threads=1`.
- New `controlled_group_errors_expand_peers_without_changing_collapse_preference`
  failed before the change because a sibling field remained hidden. The Fields
  Accordion pilot passes it after accounting for the kit's animated panel
  mounting with bounded waits; all absence/presence assertions remain.
- The actual pointer-then-Space regression reproduced a 0.7.1 trigger keyboard
  gap: the focused filter had 1 pass and 1 failure. The owner approved a scoped
  public focus/keyboard adapter, not a toolkit edit or weaker assertion.
- The approved pilot retains one focus handle per stable field group, guards
  Enter/Space by actual label focus and modifiers, yields to open popups, and
  leaves field input keys alone. Content clicks do not change forced-open
  collapse preference. `controlled_group_` now passes 2 tests with the same
  command flags; task 013 is not yet complete or committed.
- Remaining: overlapping forced-state and interaction tests, navigation/activity
  disclosure migration, native smoke/screenshots, feature gates, documentation,
  guidance and final code accounting. Tasks 014-020 remain unstarted.
- Subsequent focused checks pass: `controlled_group_` (5), `numeric_stepper_`
  (2), `setting_search_` (1), `workspace_files_` (3), and the expanded live
  dashboard regression (1). The unsupported-choice regression failed before
  extending the forcing predicate. Existing numeric collapse assertions remain
  unchanged; closed form content now unmounts immediately.
- The owner approved correcting the new workspace test's observation target to
  the existing `workspace-files` wrapper and a real file row. Actual Tab focus
  precedes Space because styled Button intentionally avoids pointer-down focus.
- Schema smoke now includes disclosure focus, forced unknown-choice visibility,
  clearing conditions, exact source, and both window sizes/themes. Headless
  harness passes (18 modules, 49 helpers, 45 dispatch/cleanup cases); native
  execution and final production/guidance gates are still pending.
- Final candidate: `controlled_group_` (5), `setting_search_` (1), `navigator_`
  (7), `run_log_` (7), `root_overlay_` (2), and the existing unknown-value/reset
  selector test (1) pass. Numeric checks (2), workspace files (3), and the
  expanded live-dashboard failure/preference test (1) also pass. All commands
  use locked gui-tests, lib filters, one test thread and `CARGO_INCREMENTAL=0`.
- Rustfmt, production GUI Clippy with `-D warnings`, desktop build and editor
  diagnostics pass. Guidance self-tests pass 10/10; validation passes for 46
  skills using existing `target/release-test-deps` on temporary PYTHONPATH. The
  first plain-Python attempt lacked PyYAML; no global package was installed.
- Native Schema and default smoke pass in fresh STA hosts with preferences,
  clipboard and drafts restored. Initial attempts stopped at process/focus
  prerequisites. The user-approved header exception is recorded below.
- Screenshot review caught a false-positive absence check while still in Source.
  Final Schema smoke explicitly waits for Fields and verifies the actual search
  input before disclosure clicks. The unchanged focus/state/source assertions
  pass at both sizes/themes; reviewed final forced/restored Fields screenshots,
  default compact views and activity output. No native header-fit pass is claimed.
- Production owners are +202/-57 (net +145), excluding the five-line test-only
  activity accessor; UI tests add 350 lines. Smoke changes are separate coverage.
  Six retained group focus handles implement the approved adapter; no document
  state, new subscriptions, toolkit code or dependency changes were introduced.
- Task 013 is ready for its local commit. Tasks 014-020 remain unstarted;
  task 019 owns the compact header repair and task 020 owns fresh final gates.

### Approved Compact Header Exception

- Origin: task 013 native Schema retry passed disclosure/focus/source checks in
  large and compact light layouts, then failed `Native control 'Dark' was not
  found`. The compact screenshot (requested 1000x720, rendered 1016x720) shows
  the appearance control clipped beyond the title bar's right edge.
- Owner decision: defer compact header fit to existing task 019 and select themes
  at a wide size before exercising compact/large body checks. Schema/default
  smoke retain their original assertions and capture dimensions. This is not
  passing compact-titlebar evidence and does not authorize an increased minimum.
- Temporary rationale: task 013's disclosure, draft, focus and body geometry can
  be verified in both themes without changing application data or bypassing any
  operation guard. All other required task checks must still pass before commit.
- Closure owner: task 019 must keep all title-bar controls visible/actionable at
  the evidence-based minimum (at most 1200x720), verify native theme switching
  there without widening, and remove this workaround before final task 020
  acceptance. Keep the source plan and bundle; no release is authorized.

### Approved Linux Baseline Exception

The owner explicitly approved continuing Windows implementation with these
separately tracked Linux baseline failures; this is not a passing Linux gate.
Ubuntu's existing Git is 2.34.1. No toolchain/Git upgrade or system changes made.

- Current Linux `cargo test --locked --features desktop --lib -- --quiet`:
  859 passed, 6 failed, 31 ignored.
- Isolated export of pre-upgrade commit `278be67`, same command/environment:
  858 passed, 7 failed, 31 ignored.
- Both fail `remote_management_preserves_files_index_and_other_remotes`,
  `remote_selection_removal_and_missing_tracking_never_switch_destinations`,
  `remote_selection_routes_push_fetch_and_pull_to_the_selected_local_remote`
  with Git remote rename/remove usage errors, and the browser installation
  complete-archive test with a zero-second lock timeout. Update scheduling
  clock/cache assertions also fail variably in both runs.
- Keep these in the issue list and require current Linux CI verification before
  release. Do not repair unrelated model behavior or claim Windows as Linux
  coverage. Task 020 must carry this exception forward until separately resolved.

## Approved Decisions

The complete 36-answer interview and refined behavior decisions are recorded in the [approved plan](../gpui-component-reuse.md#interview-decision-ledger), with detailed surface contracts in the component map, R1-R12, Other Matching Components, Collection And Workflow Pilots, Specific Retentions, and Verification Plan sections. These include stock substring command search; two-step Escape; actionless command confirmation with focus restoration and exactly-once dispatch; complete controlled opening while preserving user collapse preference; uniform overview table plus explicit full-content Details; immediate accepted multi-select writes with one history entry and no Escape rollback; unknown choice preservation; read-only Git navigation during operations but locked mutation with immutable request identity/stale-read rejection; deliberate page-aligned exit from Follow; changed-text clipboard bypass of repeat suppression; and normal toolkit sizes with both-theme evidence and a bounded window minimum.
