# GPUI Kit Component Reuse Tasks

Status: Planning and tasks 001-003 committed; task 004 Forms presentation is
verified and awaiting its local commit. The approved Linux baseline exception
remains in force.

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
- [x] 004 Forms: five form areas verified with focused tests, root lifecycle and native publishing/deployment/schema/default flows. Local commit pending.
- [ ] 005 Status and content components: not started.
- [ ] 006 Dialog footers: not started.
- [ ] 007 Toolbars: not started.
- [ ] 008 Context menus: not started.
- [ ] 009 Value selectors: not started.
- [ ] 010 Clipboard: not started.
- [ ] 011 Log navigation: not started.
- [ ] 012 Application menus: not started.
- [ ] 013 Collapsible groups: not started; form prerequisite satisfied.
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
- Task 004 is verified and ready for its authorized local commit. The Linux
  baseline exception is unchanged.

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
