# GPUI Kit Component Reuse Tasks

Status: Planning committed; task 001 is verified on Windows with the explicitly
approved Linux baseline exception. Component-adoption tasks have not started.

This bundle translates the approved decisions in the [refined plan](../gpui-component-reuse.md) into separately reviewable implementation units. The refined plan remains the behavior authority; this directory does not copy or replace it. Task IDs and dependency order are stable.

## Authorization And Status

- Task 000 is the reviewed plan and task bundle; it has no separate task file.
- The user's interview explicitly authorizes one local commit per completed, verified task, including planning and final acceptance documentation. No branch, push, release, credential change, live remote operation, toolkit edit, vendoring, fork, or plan deletion is authorized.
- Delegation of independent work is approved, but shared files must be serialized and builds/commits must be serialized. An implementation blocker still requires the decision process in the implement-plan skill.
- Keep this bundle and the refined plan. The main agent owns integration and commits.
- Task 001 is under validation after storage recovery; no component task may start until the Windows upgrade gates pass. Unavailable local Linux verification may remain explicitly pending for CI as approved.

## Baseline Evidence

Baseline revision: `be843359a593b19d155c82bdcea0ee163e015423`.

- `cargo test --locked --features gui-tests --all-targets -- --test-threads=1`: compilation failed before any tests ran due to disk exhaustion, OS error 112. No pass count or test count.
- `cargo check --locked --features gui --bin regressify-desktop`: failed during PDB link with `LNK1201` / OS error 112. No successful GUI compile.
- Storage recovery: the user authorized removal of unneeded build artifacts. Only the regenerable incremental cache was removed, recovering about 48.6 GiB of free space; binaries, fixtures, smoke artifacts, and screenshots were preserved. Use ephemeral `CARGO_INCREMENTAL=0` for this migration's build commands.
- Retried `cargo check --locked --features gui --bin regressify-desktop` with `CARGO_INCREMENTAL=0`: passed on 0.6.1. Full GUI baseline displayed a schema visibility failure; the terminal's final aggregate summary was not retained, so no aggregate pass count is claimed.
- The retained 0.6.1 test executable independently reproduced `schema_issues_have_precise_jumps_hover_help_and_keep_the_save_guard`: 0 passed, 1 failed, with diagnostic 3 offscreen. The 0.7.1 executable failed identically. The user authorized a test-only scroll repair that preserves every existing assertion.
- The approved target is frozen at exact published 0.7.1. Do not chase newer releases.
- Refined-plan lint: `rumdl check --no-exclude plan/gpui-component-reuse.md` passed with a pre-existing warning: MD004 has unknown option `after-marker`. No suppression/config change was made.

These are baseline facts, not evidence that a runtime test passed or that the planned migration compiles. Task 001 must record the remaining baseline outcomes and complete the Windows upgrade gates before any presentation migration.

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
- [x] 001 Upgrade and root API: 0.7.1 pin/lock, production/test roots and authorized compatibility fixes verified; Windows gates pass, Linux baseline exception recorded. Local commit pending.
- [ ] 002 Progress and binary checkbox: not started; blocked by 001.
- [ ] 003 Numeric fields: not started; blocked by 001.
- [ ] 004 Forms: not started; blocked by 001.
- [ ] 005 Status and content components: not started; blocked by 001.
- [ ] 006 Dialog footers: not started; blocked by 001.
- [ ] 007 Toolbars: not started; blocked by 001.
- [ ] 008 Context menus: not started; blocked by 001.
- [ ] 009 Value selectors: not started; blocked by 001.
- [ ] 010 Clipboard: not started; blocked by 001.
- [ ] 011 Log navigation: not started; blocked by 001.
- [ ] 012 Application menus: not started; blocked by 001.
- [ ] 013 Collapsible groups: not started; blocked by 001 and 004.
- [ ] 014 Flat collections: not started; blocked by 001.
- [ ] 015 Git Tree: not started; blocked by 001 and 014.
- [ ] 016 Overview Table: not started; blocked by 001.
- [ ] 017 Command palette: not started; blocked by 001.
- [ ] 018 Source font size: not started; blocked by 001.
- [ ] 019 Stock theme and normal sizing: not started; blocked by 001-018.
- [ ] 020 Final acceptance: not started; blocked by 001-019.

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
- Task 001 is ready for its authorized local commit. Linux is not claimed green.

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
