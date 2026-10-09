# GPUI Kit Component Reuse Tasks

Status: Planning and tasks 001-018 committed. Task 019 is verified and awaiting
its authorized local commit, with approved contrast-only adjustments and an
unchanged 4.5:1 requirement. Task 020 reconciliation follows that commit.
The compact-header exception is closed; the approved Linux baseline exception remains.

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
- [x] 013 Collapsible groups: Fields/workspace/activity and acceptance pass; committed as `ad26ca0`, with task 019 header exception recorded.
- [x] 014 Flat collections: committed as `23d805c`; all surfaces and native acceptance verified with the approved task 019 header exception.
- [x] 015 Git Tree: committed as `55a6142`, with the approved public base adapter and task 019 header exception.
- [x] 016 Overview Table: committed as `d8e9965`; native Default/Schema pass, with the approved task 019 header exception.
- [x] 017 Command palette: committed as `6dfd54d`; native Default passes with the approved task 019 header exception.
- [x] 018 Source font size: committed as `ecbc562`; native Default passes and integrated UI has only the approved task 019 header failure.
- [x] 019 Stock theme and normal sizing: verified, awaiting commit; Windows gates/native flows pass with the approved contrast-only adjustments and unchanged 1000x640 minimum.
- [ ] 020 Final acceptance: not started; awaits 019 and the approved Linux disposition.

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
- Task 013 is committed as `ad26ca0`. Task 014 is in progress; 015-020 remain unstarted;
  task 019 owns the compact header repair and task 020 owns fresh final gates.

### Task 014 Verification

- Fresh pre-edit baselines pass with locked gui-tests/lib/one test thread and
  ephemeral `CARGO_INCREMENTAL=0`: `navigator_` (7), `workspace_files_` (3),
  `image_previews_` (3), `scenario_sequence_` (1), `sequence_removal_` (1).
- Workspace files now use retained Kit List/ListItem with stable selected paths,
  weak deferred activation through the existing open guards, native scrolling,
  no duplicate search, and retained filename/non-editable tooltips.
- The initial scrolling test assumed every offscreen row had bounds and failed
  on the 12-row virtualized case. The owner approved allowing an absent/offscreen
  last row before scrolling; all exact short-list, post-scroll, drag, Home/End,
  activation and binary assertions remain. Added 64-row cases in both themes
  and sizes; final `workspace_files_` passes all 3 tests.
- `flat_collection_workspace_` passes (1), covering reordered stable identity,
  cursor versus activation, Up/Down wrapping, binary/busy guards and input-scope
  exclusion. Toolkit deferred scrolling is followed by a layout frame before
  inspecting existing Home/End bounds. No task 014 commit or native acceptance
  is claimed by this pilot evidence; remaining surfaces are recorded below.
- Capture rows now use Kit List/ListItem virtualization while the existing
  controller preserves keyboard preview selection and approval cleanup. The
  unchanged 152-capture scroll/filter test passes (1). `flat_collection_capture_`
  passes (2): reordered refresh failed before unique-key restoration, and
  duplicate-key clicks failed before catalog-scoped identity/activation guards.
- The bounded action-sequence delegate migrated only sequence ownership/rendering
  and its tests. UI-only IDs follow known moves/duplicates and ambiguous external
  duplicates invalidate the cursor. Existing `scenario_sequence_` (1) and
  `sequence_removal_` (1) pass; `sequence_identity_` (2) and
  `flat_collection_sequence_` (3) pass (one test overlaps those last filters).
- Suite/scenario rows now use ListItem selected/secondary presentation while
  retaining named Buttons, explicit actions, managed menu scope and the existing
  checked-range controller. Final `navigator_` passes all 7 cases. No native
  task 014 acceptance or commit is claimed yet; broad gates follow integration.
- First integrated `cargo test --locked --features gui-tests --lib desktop::ui
  -- --test-threads=1 --quiet`: 250 passed, 6 failed. Production GUI Clippy,
  desktop build, rustfmt and guidance checks passed (10 validator self-tests,
  46 discoverable skills). This was not a passing UI gate.
- Isolated `git archive` of `ad26ca0`, with the same dependency lock and exact
  filters, reproduced five failures: `missing_output_has_a_recoverable_empty_results_view`,
  `compact_titlebar_preserves_editor_space_and_navigation_at_both_sizes`,
  `suite_commands_request_execution_directly_and_keep_editor_state`,
  `panel_tabs_align_with_content_at_both_window_sizes`, and
  `git_large_tree_and_status_fit_compact_and_wide_windows`. The independent
  preview-mode test passed on that baseline and failed in the candidate.
- Owner approved adjacent field-remount timing and Run-tab alignment repairs,
  retaining only the existing title-bar exception. A diagnostic preserved the
  failing immediate visibility assertion and proved the URL field becomes visible
  on the next layout frame with unchanged bounds. The three affected tests now
  render that frame before their unchanged visibility assertions; all three pass.
- Run toolbar padding now matches the existing panel/tab inset. The unchanged
  alignment test failed at 270px versus 263px before the fix and passes afterward
  in both themes/sizes. No geometry assertion was relaxed.
- The zoom fixture reused one image path for five captures, accidentally giving
  them duplicate keys. Unique `fileName` values now express its five identities
  without changing image data, legacy scenario IDs or any zoom assertions;
  `image_previews_` passes all 3 tests. The separate duplicate-key regression now
  explicitly verifies refresh fallback, approval dismissal and distinct row clicks.
  Final `flat_collection_capture_` passes 2 tests.
- The Git long-status header failure also reproduces at `ad26ca0` and remains
  with the owner-approved task 019 title-bar exception below. The full UI gate
  will continue reporting it; do not suppress or mark that test passing.
- Final integrated rerun: 255 passed, 1 failed, 0 ignored, 903 filtered out
  (1263.53s), using the same full `desktop::ui` command without exclusions.
  The sole failure is the approved task 019 Git header assertion. GUI Clippy
  with `-D warnings`, the desktop build, rustfmt, guidance validation and all
  10 guidance self-tests pass. No broader CLI, Linux or worker gate is claimed.
- Current-binary native default, Schema, Comparison and Artifact flows pass
  through `pwsh -NoProfile -STA -File ./ci/desktop-smoke.ps1 -SkipBuild` with
  the matching flow switch. Schema's first attempt stopped at the focus guard;
  the owner confirmed readiness and the retry passed. Preferences, clipboard
  and synthetic drafts were restored; no focus or operation guard was bypassed.
- Inspected `editor.png`, `editor-compact.png`,
  `comparison-light-1600-fit.png`, `comparison-dark-1080-fit.png`,
  `run-settings-dark-compact.png`, `controlled-fields-forced-dark-1000.png` and
  `suite-context-menu.png` under `target/desktop-smoke`. Row content, selection,
  sequence controls, image panes and context menus render in the expected body
  layouts. Compact header clipping remains the explicit exception, not a pass.
- Production line net by owner: Workbench +18, navigation +150, reports +112,
  runs +1, sequence +310, total +591. Test net: reports +139, sequence +497,
  UI tests +157, total +793. Three retained Kit List entities replace two manual
  list states and one capture scroll handle; weak owners, sequence snapshots/IDs
  and capture catalog generation support identity guards. No new application
  subscriptions, dependency edits or toolkit fork. Existing suite/scenario
  selection controllers remain. Reviewed callback scope, busy/draft checks,
  weak ownership, refresh ambiguity and known action permutations.
- Task 014 is committed as `23d805c`; the post-commit worktree was clean.
  Acceptance retains the approved header exception. Source plan and bundle
  remain; no push or release occurred.

### Task 015 Verification

- `cargo test --locked --features gui-tests --lib desktop::ui::git::tests
  -- --test-threads=1` with temporary `CARGO_INCREMENTAL=0`: 17 passed, 1 failed,
  0 ignored (155.60s). The sole failure is the existing task 019 header assertion.
- Current trees own flattened rows, cursor paths, expansion and scroll handles.
  Their navigation/activation guards also block read-only movement while an
  operation is active. The approved Tree migration must separate that from
  mutation policy without changing the cloned reviewed `model::Request`.
- Public Kit TreeState provides `set_items`, `selected_item`, `index_of`,
  expansion events and scroll handles. `set_items` clears the cursor, so restore
  it by group/path ID rather than the previous numeric index.
- Added `git_tree_allows_read_only_navigation_during_operations`: failed before
  the guard repair (End retained `first.txt`), then passed. Both groups now use
  retained Kit TreeState/Tree/ListItem with weak owners, group/path IDs and two
  expansion subscriptions. Removed manual flattened row vectors, cursor paths,
  focus handles and scroll handles; checkbox/request policy remains in GitPanel.
- The first refresh pilot passed reordered/removed identities and independent
  collapsed branches before rendering changed. Scoped Enter now routes directly
  to Kit Confirm for folders and existing diff review for leaves. Forwarding
  another action from the old handler did not satisfy the existing collapse test.
- The unchanged folder-selection test exposed a missed checkbox click after
  migration. Disabled/active and clipping diagnostics ruled those causes out;
  isolating checkbox mouse-down from Tree expansion fixed it. All temporary
  probes were removed; no existing assertion was relaxed.
- `cargo test --locked --features gui-tests --lib git_tree_ -- --test-threads=1`
  passed 5 tests before the final body test was added: operation browsing,
  group/path refresh, immutable emitted requests, stale diff success/failure and
  the existing unsaved commit guard. The request fixture records a real emitted
  Request but prevents Workbench execution; no synthetic lock test performs a write.
- `git_tree_virtual_scroll_fits_both_groups_and_window_sizes` passes (1), with
  256 rows per group, both themes/sizes, wheel, Home/End and exact visible bounds.
  It checks body geometry independently because the approved header failure
  occurs before the old large-tree body's assertions. Production and native
  acceptance remain pending; no task 015 commit is claimed yet.
- First integrated candidate: the full Git UI slice ran 23 tests, 22 passed and
  only the approved header assertion failed (196.02s). Rustfmt, production GUI
  Clippy and desktop build passed. Guidance self-tests passed 10/10 and the
  validator checked 46 skills; changed-file Markdown checks passed.
- Native Git reached its final compact conflict Light switch, then failed to
  find the clipped theme control. The owner approved extending wide-only theme
  selection to this Git step while retaining both compact captures/assertions.
  Headless harness passed (18 modules, 49 helpers, 45 dispatch/cleanup cases).
  The next full local-repository Git smoke passed and restored preferences.
- Final ownership review added `git_tree_entities_release_after_window_close`:
  it failed because both styled Tree states remained alive after GitPanel/window
  release. The styled 0.7.1 renderer captures its own entity for context menus,
  even when none is configured. The owner approved public base Tree with the
  same ListItem/scrollbar styling; the lifecycle regression now passes. No
  toolkit/private API/dependency changes. Final gates and native smoke are being
  repeated on this repaired candidate before acceptance or commit.
- Final repaired candidate: the same complete Git UI command runs 24 tests,
  23 passed, 1 failed, 0 ignored, 1141 filtered out (179.08s). The sole failure
  remains `git_large_tree_and_status_fit_compact_and_wide_windows`, assigned to
  task 019. All seven `git_tree_` cases pass within that run. Rustfmt, production
  GUI Clippy with `-D warnings`, desktop build, editor diagnostics and guidance
  checks pass (10 self-tests, 46 skills).
- Final rebuilt binary passes `pwsh -NoProfile -STA -File
  ./ci/desktop-smoke.ps1 -SkipBuild -GitFlows`. The first final attempt stopped
  before scenarios at the Windows focus guard; the owner confirmed readiness
  and the retry passed. Local-repository init, selected ordinary/comparison
  commits, diff copying, remote forms/cancellation, push/pull and conflict
  handoff pass, and preferences are restored. No real remote was used.
- Inspected final `git-text-diff-{light,dark}-{1080,1600}.png` and
  `git-comparison-changes-selected.png` under `target/desktop-smoke`, plus the
  preceding tree and compact conflict images. The body layouts, row selection,
  disabled conflict controls and split diffs are intact. Header clipping remains
  recorded, not accepted as correct.
- Production diff: GitPanel +52/-27, tree view +279/-244, scoped key binding +5,
  total net +65. Tests +553/-3 (net +550), smoke +3. Two retained Tree entities,
  two expansion subscriptions and one callback generation replace two row vectors,
  two cursor paths, two focus handles and two scroll handles. Expansion preference
  and application selection remain owned by GitPanel. Reviewed request capture,
  locks, stale completions, pointer behavior and entity release; no unresolved
  task 015 defect outside the approved header exception. No broader CLI/Linux
  gate is claimed. Task 015 is committed as `55a6142`; the post-commit worktree
  was clean. No push or release occurred.

### Task 016 Verification

- Fresh pre-edit baselines pass with locked gui-tests/lib/one test thread and
  ephemeral `CARGO_INCREMENTAL=0`: `overview_` (3), `navigator_` (7),
  `suite_name_dialog_` (6), `source_line_commands` (12).
- The pinned uniform TableState API is rendered through `DataTable` in 0.7.1.
  One retained table snapshots suite rows and resolves callbacks against current
  workspace/file identity; row cursor and checked suites remain separate. No
  change was made to variable-height Source diagnostics.
- Added checkbox/row/Edit isolation before migration; it passed before and after.
  Existing saved-report routing passes. The header/body alignment test exposed
  Kit's sort-icon padding in unsortable headers; fixed-width header content
  restores the unchanged exact bounds assertions in both themes/sizes.
- Details opens only from its explicit control or Enter with actual Table focus.
  Its read-only editor preserves full summary/error text, CopyText and focus
  return. The long-label/150-line-error regression passes exact selection/copy,
  read-only input and Close/Escape at both sizes/themes. Refresh/filter/busy and
  owner-release coverage passes; no retained entity cycle was found.
- `overview_table_` passed 3 tests before the large-row case was added; the new
  `overview_table_virtual_rows_keep_uniform_geometry` passes (1), covering 80
  mixed valid/error rows, exact uniform heights, wheel scrolling, offscreen
  virtualization and compact action reachability in both themes/sizes.
- Default native smoke now exercises Details full text, selection/copy and close
  in both themes/sizes with clipboard restoration. Headless harness passes:
  18 modules, 49 helpers, 45 dispatch/cleanup cases.
- Final `cargo test --locked --features gui-tests --lib desktop::ui -- --test-threads=1 --quiet`:
  265 passed, 1 failed, 0 ignored, 903 filtered. The only failure is the approved
  task 019 `git_large_tree_and_status_fit_compact_and_wide_windows` header
  assertion; all four new Overview cases and existing neighbors pass.
- `cargo fmt --all -- --check`, `cargo clippy --locked --features gui --lib -- -D warnings`
  and `cargo build --locked --features gui --bin regressify-desktop` pass with
  ephemeral `CARGO_INCREMENTAL=0`.
- Rebuilt `pwsh -NoProfile -STA -File ./ci/desktop-smoke.ps1 -SkipBuild` and
  the same command with `-SchemaFlows` pass. Reviewed all eight Overview/Details
  Light/Dark screenshots at 1080x720 and 1600x1000, plus Source diagnostic
  captures. Native full-text copy, independent controls, existing default flows,
  selectable variable-height diagnostics and external-conflict workflows pass.
  Preferences and clipboard were restored. Wide-only theme selection still
  follows the task 019 exception; compact title-bar fit is not claimed green.
- Reviewed current-root/path action guards, busy/run locks, weak ownership,
  cursor refresh identity and full-text/read-only boundaries; no new defect
  found. Source diagnostics and dependencies are unchanged.
- Production net +316 lines (Overview +301, Workbench +15); GUI tests +306;
  native smoke +35. Scoped Markdown and guidance checks pass (10 self-tests,
  46 skills); existing MD004 configuration warning remains. Linux is deferred
  under its separate exception. Committed as `d8e9965`.

### Task 017 Verification

- Fresh `cargo test --locked --features gui-tests --lib command_palette_ -- --test-threads=1`
  baseline: 3 passed. Actionless Kit Command replaces the custom ListDelegate;
  the catalog, enablement and existing guarded actions remain unchanged.
- The unchanged unsaved-dialog guard pilot passes (1). Expanded palette filter
  passes (5): approved contiguous case-insensitive title/category substrings,
  rejection of reversed words/noncontiguous characters, keyboard selection,
  two-step Escape, theme/draft preservation, focus and duplicate confirmation.
- Strengthened disabled-pointer/Enter, empty-result Enter and cancellation-focus
  test passes (1). Continuing query typing after a row click explicitly restores
  the captured search focus; the disabled action remains inert.
- Native default now checks a rejected former all-words query, first-Escape clear
  and second-Escape dismissal at both sizes/themes. Headless smoke harness passes:
  18 modules, 49 helpers, 45 dispatch/cleanup cases.
- Final `cargo test --locked --features gui-tests --lib desktop::ui -- --test-threads=1 --quiet`:
  267 passed, 1 failed, 0 ignored, 903 filtered. All five palette cases pass;
  the only failure remains task 019's unchanged compact Git header assertion.
- `cargo fmt --all -- --check`, `cargo clippy --locked --features gui --lib -- -D warnings`
  and `cargo build --locked --features gui --bin regressify-desktop` pass with
  ephemeral `CARGO_INCREMENTAL=0`.
- Rebuilt `pwsh -NoProfile -STA -File ./ci/desktop-smoke.ps1 -SkipBuild` passed on
  a complete unchanged retry. The first attempt passed all palette combinations,
  then failed in existing `results.ps1` clipboard restoration; preferences were
  restored but that clipboard restore failed. The retry completed all default
  workflows and cleanup. This does not retroactively restore the first backup.
- Reviewed eight empty/cleared palette screenshots across Light/Dark and
  1080x720/1600x1000, plus filtered native theme confirmation. Native accessible
  labels and two-step Escape pass; wide-only theme selection remains task 019's
  exception, not green compact-header evidence.
- Final review preserves the catalog, existing guarded actions, actionless
  items, close/focus/dispatch ordering and one-shot confirmation. No new palette
  defect found. Production net -78 lines; tests net +161; smoke +9. Linux remains
  deferred. Committed as `6dfd54d`; the clean resume independently verified the
  commit and its acceptance body before starting task 018.

### Task 018 Verification

- Resumed from clean `6dfd54d`; task 017's pending-commit text was stale, not
  missing implementation. Tasks 001-017 and their recorded dependencies are
  reconciled; the approved Linux and task 019 header exceptions remain.
- Fresh baselines with temporary `CARGO_INCREMENTAL=0` pass:
  `cargo test --locked --features gui-tests --lib source_line_commands -- --test-threads=1`
  (12 tests), and the same command with filter `preferences` (5 tests).
- New `source_font_size_accepts_direct_integer_entry` failed before migration
  (0 passed, 1 failed) and passes with a retained normal Kit NumberInput. The
  source-only adapter accepts integers 10..24, leaves invalid text unpersisted,
  ignores invalid/boundary steps, restores the last valid value on blur and
  preserves serialized saves and Reset. Configuration field policy is unchanged.
- `cargo test --locked --features gui-tests --lib source_font_size_ -- --test-threads=1`:
  3 passed. Coverage includes actual child values, empty/fractional/exponent/
  invalid/out-of-range text, blur, keyboard/pointer steps, saved bytes, reload,
  Reset, exact invalid Source drafts and both themes at 1000x640/1600x1000.
- The new blur fixture initially omitted window activation: GPUI suppresses
  focus events for inactive windows. Activating and waiting for the real Blur
  event preserves every assertion. Its invalid Source fixture now uses an
  unmatched closing bracket to avoid native auto-pairing; no production focus
  workaround or diagnostic trace remains.
- Neighbor checks with the same locked gui-tests/lib/one-thread flags pass:
  `preferences` (5), `preference_write_failures_do_not_lock_the_editor_and_can_be_retried`
  (1), `source_tools_preserve_text_and_jump_to_syntax_errors` (1),
  `source_line_commands` (12), `numeric_stepper_` (2), and
  `toolbar_editor_controls_fit_supported_window_sizes` (1). Obsolete button
  targets now address built-in steps and the actual editable child. Normal size
  is checked on the 28px frame; its step buttons have exact 1px inner insets.
- `cargo fmt --all -- --check`, `cargo clippy --locked --features gui --lib -- -D warnings`
  and `cargo build --locked --features gui --bin regressify-desktop` pass.
  Final `cargo test --locked --features gui-tests --lib desktop::ui -- --test-threads=1 --quiet`:
  270 passed, 1 failed, 0 ignored, 903 filtered (2466.31s). The sole failure is
  the unchanged task 019 `git_large_tree_and_status_fit_compact_and_wide_windows`
  header assertion. This is not a fully passing gate despite the shell wrapper's
  reported zero exit status; the test aggregate is authoritative.
- Native Default now checks exact entry, steps, invalid drafts, blur, Reset,
  persisted JSON, restart and unchanged Source undo/file bytes. Headless harness
  passes: 18 modules, 49 helpers, 45 dispatch/cleanup cases. Scoped Markdown,
  guidance validation (46 skills) and all 10 validator self-tests pass using
  the existing `target/agent-guidance-venv` environment. The earlier plain-Python
  attempt lacked PyYAML; no global dependency was installed.
- Rebuilt `pwsh -NoProfile -STA -File ./ci/desktop-smoke.ps1 -SkipBuild` stopped
  in the existing Overview Details copy check: `default.ps1:20`, `GetText`,
  requested Clipboard operation did not succeed. One complete unchanged retry
  passed the Overview/Command size/theme cases but failed at `default.ps1:40`,
  restoring its clipboard backup with `SetDataObject`. Both attempts restored
  preferences; the second attempt's clipboard backup was not restored.
- Neither initial attempt reached task 018's font-size checks. The owner then
  explicitly authorized bounded adjacent clipboard-harness diagnosis/repair
  with regression tests and unchanged exact-copy/cleanup guards.
- The new headless regression failed before replacing Overview's immediate
  reads with the existing sequence-before-read helper, then passed. Unrecovered
  clipboard failures now report only elapsed time, exception type and HRESULT;
  the same two-second busy-only retry policy and original exception remain.
  This does not claim that every historical clipboard-restoration failure is
  solved or that a previously lost backup can be recovered.
- Native focus tracing identified the new test's wrong observation target:
  NumberInput reports focus on its Spinner, with the value on its Edit child.
  Final checks require exactly one Spinner owning exactly one Edit, actual
  pointer/keyboard focus and exact input/persisted values. Temporary focus
  tracing was removed; no production focus adapter was needed.
- Final rebuilt `pwsh -NoProfile -STA -File ./ci/desktop-smoke.ps1 -SkipBuild`
  passes all Default workflows, including entry at 10/24/17, keyboard/pointer
  steps and limits, invalid/fractional/out-of-range drafts, blur, Reset, exact
  configuration bytes/undo, saved preferences and restart. Clipboard cleanup,
  dirty-fixture cleanup and preference restoration complete on this run.
- Inspected final `source.png`, `source-dark.png`, `source-compact.png` and
  `source-dark-compact.png` under `target/desktop-smoke`: Source controls fit at
  1600x1000 and 1080x720 in both themes. Native compact-header fit remains the
  task 019 exception, not a pass. Final harness and guidance checks pass.
- Production +94/-44 (net +50): one retained InputState and two subscriptions;
  GUI tests +313/-11, native Default +105/-4, shared helper +4/-1 and headless
  regression +9. Reviewed strict parsing, no-op/rejected steps, serialized
  saves, weak subscription ownership and unchanged configuration/history.
  No new dependency or toolkit change. Task 018 is committed as `ecbc562`;
  its post-commit worktree was clean. The Linux exception is unchanged.

### Task 019 Verification

- Started from clean task 018 commit `ecbc562`. Baseline commands use temporary
  `CARGO_INCREMENTAL=0`. `cargo test --locked --features gui-tests --lib
  theme_choices_persist_without_changing_drafts_and_explicit_modes_ignore_system_changes
  -- --test-threads=1` passes (1 test), as does the same command with filter
  `both_palettes_keep_text_readable_and_keyboard_focus_visible` (1 test).
- A small baseline capture probe in existing Titlebar smoke passes headless
  checks (18 modules, 49 helpers, 45 dispatch/cleanup cases). Rebuilt task 018
  binary `pwsh -NoProfile -STA -File ./ci/desktop-smoke.ps1 -SkipBuild -TitlebarFlows`
  passes existing pointer, window controls, drag and unsaved-close checks, with
  preferences and dirty fixture restored.
- Inspected all four `stock-sizing-before-{light,dark}-{1000,1600}.png` images.
  Requested 1000x640 produces 1016x648 outer bounds on Windows; large captures
  are 1600x1000. The minimum-size header clips right-side controls. These are
  pre-change captures, not completed header-fit evidence. Minimum remains
  1000x640; no increase or workaround removal has been made.
- The first production pilot removes custom palette, UI font and radius
  overrides while retaining visible focus rings and existing scrollbar policy.
  An initially misapplied patch caused a compile error before tests; the exact
  committed helpers were restored, and the final diff changes only apply_theme.
- Immediate unchanged `cargo test --locked --features gui-tests --lib
  both_palettes_keep_text_readable_and_keyboard_focus_visible -- --test-threads=1`:
  compiles, then 0 passed, 1 failed: `secondary contrast 4.35 with dark=false`.
  The pre-edit palette passed the same test. Remaining color pairs are not
  claimed verified because the test stops at this first failure.
- This triggers task 019's explicit insufficient-contrast reopen condition.
  Palette policy needs an owner decision before continuing. No threshold,
  assertion, toolkit source or dependency has been changed; no task 019 commit,
  stock-theme native acceptance or final feature-gate pass is claimed.
  Header repair, normal-size sweep and task 020 remain pending.
- Owner decision: allow minimal contrast-only color adjustments to satisfy the
  existing accessibility gate. Preserve stock typography, sizing and all other
  styling; no brand palette, lowered threshold or toolkit/dependency change.
  The same test now aggregates all failing pairs without removing a pair or
  changing its threshold, so the actual adjustment scope can be measured.
- Aggregate stock failures were light secondary/success/warning/error/info and
  danger-button pairs in both themes. The approved correction preserves hue and
  saturation, minimally moves failing foreground lightness, and resolves stock
  danger tints over the stock background. The original 4.5:1 gate plus danger
  hover/active pairs now passes (1); no brand colors or font/radius overrides.
- The normal header packs name/version, retains full app menus and accessible
  actions, bounds the visible Git badge and permits content shrinkage. Deploy
  remains an accessible normal-sized icon button. The unchanged
  `git_large_tree_and_status_fit_compact_and_wide_windows` passes (1). Native
  minimum-size theme switching and workaround removal still need verification.
- Editing/navigation sweep removes 30 compact overrides with measured list
  heights, wrapping action groups and a 288px default sidebar (minimum window
  unchanged). With `cargo test --locked --features gui-tests --lib <filter>
  -- --test-threads=1`, focused checks pass: `navigator_` (7), `workspace_files_`
  (3), `setting_search_` (1), `numeric_stepper_` (2), `source_font_size_` (3),
  `source_line_commands` (12), `scenario_sequence_` (1), `flat_collection_sequence_`
  (3), `sequence_removal_` (1), `external_reload_keeps_` (2), `toolbar_editor_`
  (2), `suite_name_dialog_` (6), and
  `problem_links_focus_drafts_and_resets_preserve_other_invalid_fields` (1).
- Diagnostic jump buttons now use normal 32px controls; only their obsolete
  exact 20px size expectation changed. The complete
  `schema_issue_panel_scrolls_and_keeps_text_inside_compact_and_large_windows`
  passes (1), retaining message, centering, scrolling and jump assertions.
- Results/Run sweep removes 34 compact overrides and four fixed heights.
  Focused filters pass with the same command flags: `flat_collection_capture_`
  (2), `result_capture_list_scrolls_` (1), `results_filter_errors_` (1),
  `scenario_approval_` (1), `approval_completion_` (1), `report_approval_` (2),
  `comparison_completion_` (4), `run_log_` (7), `capture_log_` (3),
  `log_pagination_` (2), `log_health_warning_` (1), `scenario_checklists_` (1),
  `multiple_suites_can_be_checked` (1), `run_settings_are_available` (1),
  `direct_commands_flush_pending` (1), `live_dashboard_tracks` (1),
  `runs_open_site_` (1), `deployment_activity_` (2), and `preview_loading_` (1).
- Metadata initially failed the old 160px height and 8px text-gap assertions.
  Paired URLs now share a stock two-column DescriptionList row. The owner
  approved a bounded label-cell repair and then stock-aware geometry checks:
  keep the 8px gaps between control bounds and separately assert the stock 10px
  text inset. Exact metadata, copy, height and vertical alignment remain;
  `scenario_information_hover` passes (1).
- Reducing image command-band padding preserved normal controls and passed the
  unchanged 110px header-space limit. Fit-width then exposed a fractional-pane
  rounding mismatch: 300x1600 canvas, 259px viewport, 1379.5px painted height
  versus 1381.33px expected. The owner approved a bounded public measurement
  repair with the one-pixel tolerance and all zoom policies retained.
- Pixel-aligned pane widths now apply only to side-by-side FitWidth through
  the existing public canvas layout API. The previously failing fit regression
  passes (1); final neighboring `image_previews_` (3), `side_by_side_scrolls_`
  (1), `image_context_menus_` (1), `results_normal_` (2) and
  `runs_normal_controls_` (1) all pass. No toolkit edit or dependency change.
- The remaining Overview/Git/publishing/update control sweep is implemented.
  Full Git UI passes 24 tests after bounded short-tree space sharing, a 480px
  default review column within the existing resize range, footer spacing and
  paired Fetch/Push URL rows. No Git assertions or operation guards changed.
- Native harness work removes the wide-only theme workarounds and adds chosen
  minimum-size checks; headless checks pass (18 modules, 50 helpers, 45
  dispatch/cleanup cases, 41 sizing cases). Current-binary native execution is
  still pending; removing a workaround is not proof that its closure check passes.
- The authoritative full `cargo test --locked --features gui-tests --lib desktop::ui -- --test-threads=1 --quiet`
  run reported 271 passed, 3 failed, 0 ignored, 903 filtered in 2872.10s.
  A preceding delegate's claimed 24-test full-UI pass was inconsistent and is
  not accepted as full-gate evidence. The three failures clicked offscreen
  controls in normal-sized scrollable Fields and Run settings.
- On 2026-10-09, resumed with only those failures after the owner raised the
  cost of repeated broad runs. `deployment_preserves_inherited_and_explicit_report_options`
  reproduced alone (0 passed, 1 failed), then passed (1) after measured native
  scrolling. The other two fixtures now use the same real scroll-to-target
  interaction; production code and all behavioral assertions are unchanged.
- One shared rebuild and the exact focused command below passed 3 tests,
  0 failed, 0 ignored, 1174 filtered in 16.69s. The rebuild took 2m08s; the
  earlier single-test rebuild took 2m53s including build-directory lock waiting.

  ```powershell
  cargo test --locked --features gui-tests --lib -- --test-threads=1 controlled_group_search_and_error_restore_preference_without_losing_input deployment_preserves_inherited_and_explicit_report_options editing_real_suite_fields_preserves_plain_css_selectors
  ```

- The focused checks retain invalid drafts, collapse preference/focus return,
  inherited versus explicit report options, exact CSS selectors, comments and
  saved content. `cargo fmt --all -- --check` and editor diagnostics pass.
  No broad suite was rerun for these repairs. Further broad runs require owner
  approval; the 3-test pass does not replace outstanding full/native gates.
- Remaining: final candidate review/maintenance, required feature/native gates
  and minimum-size evidence. Task 019 remains uncommitted; task 020 has not
  started. Plans are retained and no release is authorized by this work.

### Task 019 Native Acceptance

- Native Titlebar initially reproduced long workspace-name clipping at the
  1000x640 minimum. The existing path-header test now uses a long name and
  failed before the viewport/native-caption bound, then passed (1). Neighboring
  compact-header and long-Git-status checks passed (2). Full paths remain
  accessible; neither control sizes nor the application minimum were reduced.
- The new harness incorrectly expected an Edit application menu; the approved
  catalog is File/Publish/Help. A headless regression failed before correcting
  that list. The owner approved native Tab/Enter/Escape testing for the
  appearance dropdown, which is enabled/focusable but lacks InvokePattern.
  Commands, Deploy and Switch workspace retain Invoke checks. Native Titlebar
  now passes four minimum/large theme combinations and window/close guards.
- Git tooltip checks exposed frozen elapsed-time text and missing explicit
  accessible tooltip content. The bounded Kit tooltip now uses a plain Label
  and reads live status through a weak Git owner. The extended long-status
  regression passes exact text, updates while open and viewport bounds (1).
  Full native Git passes the unchanged exact tooltip check and long-conflict
  theme switching at minimum/large sizes, plus local-repository workflows.
- Native Schema passes minimum/large/legacy-compact Fields disclosure, exact
  source, diagnostics, keyboard completion, line commands, saves and external
  conflicts. Default passes Overview/Details, commands, Source font preference,
  Results/log/approval and Run checks. Add dropdown focus uses real bounded Tab
  traversal; its exact Add-to-More Left-wrap assertion is unchanged. The added
  minimum-size Actions menu uses the existing settings search before clicking.
- Native Comparison passes pixel/fit/pan/zoom/crosshair checks in both themes
  at 1000x640, 1080x720 and 1600x1000, plus empty suites, scoped approval and
  failed-image artwork. Its return to the second suite now scrolls that row
  into view; exact report identity and all pixel assertions remain.
- Artifact exposed suite rows hidden after large-to-minimum resize. The new
  `navigator_suite_rows_remain_clickable_after_minimum_resize` failed before
  the internal pane minimum repair. The suite pane reserves 256px when workspace
  files are collapsed and retains its existing 150px allowance when expanded,
  preserving the eight-row workspace-files budget. Initial neighbors: 10 passed,
  1 failed; final resize plus workspace-file scrolling check: 2 passed in 266.87s.
  No window minimum increase. Native Artifact passes create/rename/delete,
  scope guards, restart and exact artifact preservation after a user-requested
  retry of a focus-prerequisite failure. The added Rename size matrix uses its
  real suite context menu; the original F2/Escape tests remain.
- Publishing also needed a user-requested retry of the focus prerequisite.
  Its retained landing editor is scrolled to the toolbar before local removal.
  Final Publishing passes token masking/reuse, landing/draft/target workflows
  and all sizes/themes with synthetic credential/catalog cleanup and no remote
  publication. Final Deployment passes saved URLs, subset/provider guards,
  selectable exact logs, persistence and missing-token outcomes at all sizes.
- Native commands were `pwsh -NoProfile -STA -File ./ci/desktop-smoke.ps1 -SkipBuild`
  with no switch (Default), or `-TitlebarFlows`, `-GitFlows`, `-SchemaFlows`,
  `-ComparisonFlows`, `-ArtifactFlows`, `-PublishingFlows`, `-DeploymentFlows`.
  All completed passing runs restored preferences; no failed prerequisite or
  intermediate failure is counted as passing. The last sidebar-only change
  was verified by focused tests and rebuilt Artifact/Publishing/Deployment;
  earlier successful flows are not misrepresented as reruns of that binary.
- Inspected minimum/large Source/titlebar, command and deployment-dialog images;
  minimum Git text diff and live conflict tooltips in both themes; minimum
  Artifact rename/delete; and minimum landing-site/token-editor captures under
  `target/desktop-smoke`. Other recorded captures remain available for final
  review. Headless harness passes: 18 modules, 50 helpers, 45 dispatch/cleanup
  and 41 sizing cases. Final Rust/guidance/review gates remain; no task commit.
- Final read-only review found no concrete defect in the header/tooltip,
  navigation, normal-control, metadata and fit-width ownership paths. This is
  review evidence, not a substitute for the pending full test gate.
- Current `cargo fmt --all -- --check` and
  `cargo clippy --locked --features gui --lib -- -D warnings` pass (Clippy 25.30s),
  with temporary `CARGO_INCREMENTAL=0`. Guidance validation passes for 46 skills;
  all 10 self-tests and scoped Markdown checks pass. Existing MD004 configuration
  warning is unchanged. Latest desktop build passed in 1m24s.
- Task 019 accounting against `ecbc562`: production UI net +52 lines; GUI tests
  net +528 (including 371 lines in inline test modules); native flows net +302;
  headless harness tests net +187. No new application subscriptions/domain state,
  toolkit dependency, vendoring or fork. Public layout/contrast helpers and the
  weak tooltip wrapper account for the retained adapters; test additions are
  not counted as production reduction.
- Required broad Rust verification remains approval-gated because the previous
  full UI run took 47m52s. A fresh consolidated final pass has not yet run; no
  task 019 commit or task 020 completion is claimed from the focused/native
  evidence alone.
- Owner approved one consolidated final Rust pass on this unchanged candidate,
  reusable for tasks 019 and 020 to avoid duplicate long runs. Keep separate
  CLI, desktop-model and GUI/worker results, local per-task commits and all plan
  files. Only documentation/status integration may follow without invalidating
  that reuse; implementation changes require affected checks again.
- Consolidated final pass started on runtime-diff SHA-256
  `7E24A2E7C60C9EEC9956ECBC8C1973BDEF27B062DDC4BFF752971CE9AD399EA0`.
  `cargo test --locked --all-targets -- --quiet` passes: 524 passed, 23 ignored
  (7.94s tests; 52.74s build). Windows
  `cargo test --locked --features desktop --lib -- --quiet` passes: 864 passed,
  31 ignored (164.46s tests; 1m07s build). Neither ignored tests nor Windows
  results establish Linux coverage.
- The single approved `cargo test --locked --features gui-tests --all-targets -- --test-threads=1 --quiet`
  passes: 1,146 library tests passed, 0 failed, 32 ignored (3153.40s), and 11
  worker tests passed, 0 failed, 4 ignored (6.56s). Build took 2m43s. This is
  the authoritative aggregate, including all repaired UI cases and the closed
  header assertion, not a sum of earlier focused runs.
- Runtime-diff fingerprint matched exactly after all gates. Final production
  `cargo build --locked --features gui --bin regressify-desktop` passes (26.03s),
  restoring the production feature binary after all-target tests. Formatting,
  production GUI Clippy, guidance/self-tests and scoped Markdown already passed
  on this unchanged implementation. Task 019 is verified and awaiting its local
  commit; task 020 may reuse these results under the explicit owner approval.

### Approved Compact Header Exception

Closure evidence: task 019's focused header tests and native Titlebar/Git now
pass at the unchanged 1000x640 minimum in both themes, including long workspace
and conflict names. Wide-only theme workarounds are removed. The historical
exception below is closed for header fit, not an exemption from pending final
Rust verification.

- Origin: task 013 native Schema retry passed disclosure/focus/source checks in
  large and compact light layouts, then failed `Native control 'Dark' was not
  found`. The compact screenshot (requested 1000x720, rendered 1016x720) shows
  the appearance control clipped beyond the title bar's right edge.
- Owner decision: defer compact header fit to existing task 019 and select themes
  at a wide size before exercising compact/large body checks. Schema/default
  smoke retain their original assertions and capture dimensions. This is not
  passing compact-titlebar evidence and does not authorize an increased minimum.
- Task 014 integrated testing also reproduces
  `git_large_tree_and_status_fit_compact_and_wide_windows` at `ad26ca0`: its long
  Git status pushes `open-workspace` beyond `width - 95`. The owner retained this
  header exception while approving adjacent non-header repairs. Task 019 must
  rerun this unchanged assertion as well as native theme switching.
- Task 015 native Git confirms the same clipping at the final compact conflict
  Light switch. The owner approved wide-only theme selection in that step;
  both 1080x720 conflict captures remain. Task 019 owns removing this additional
  workaround and native switching with long Git/conflict status at minimum size.
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
