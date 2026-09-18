# Regressify Desktop Implementation Plan

## Goal

Provide a local, Rust-native GPUI Kit application in which QA can maintain
Regressify workspaces, configure suites and scenarios, execute the shared Rust engine,
and review visual results without manually constructing commands. YAML and JSON
remain the source of truth. The CLI remains usable without GUI dependencies.

## Architecture

- An optional `regressify-desktop` binary uses GPUI Kit for the desktop interface.
- A shared Rust library exposes the existing configuration, action types, and
  validation. The CLI keeps its command and exit-code contracts.
- Desktop document, workspace, execution, and result models are independent of
  GPUI so their behavior can be tested without opening a window.
- Each command executes an isolated worker from the desktop executable itself.
  No separately installed CLI is required. Process-wide `OnceLock` values
  must never leak from one desktop run or workspace to another.
- File access is scoped to the selected workspace. Writes detect external
  changes and use atomic replacement. Invalid edits never overwrite good files.
- Structured editing preserves YAML comments where supported by the document
  parser. Unsupported document constructs must produce a clear diagnostic,
  never silently discard data. Source editing remains available.

## Workbench Improvements

- [x] Bundle the engine in the desktop executable; remove the CLI picker and
  external executable dependency. Verify standalone init and repeated runs.
- [x] Emit structured live suite/scenario/viewport progress, including queued,
  running, capture, comparison, failure, and cancellation states.
- [x] Provide checkbox-based multi-suite selection with select-all/clear and
  an explicit run scope; never silently run all suites for an empty selection.
- [x] Keep one active workspace per window, clear workspace-owned state on switch,
  guard unsaved work, and prevent stale asynchronous results from crossing roots.
- [x] Redesign navigation, forms, execution, and results for a polished QA
  workbench with clear hierarchy, compact controls, and useful empty states.
- [x] Update tests, documentation, CI, and native smoke coverage; commit each
  completed unit and distinguish local verification from platform follow-ups.

Backend verification: the standalone desktop worker initializes a workspace
without a CLI executable. Typed progress tests cover parallel suites, multiple
captures per scenario, partial records, cancellation, and saturated log queues.
A real two-suite comparison integration test reports one pass and one mismatch
while preserving the zero-exit visual-failure contract.

Native redesign verification: twenty production UI tests pass. The Windows
smoke test exercises the workspace overview, grouped editor, source/results,
multi-suite checklist, and bundled-engine init at 1600x1000 and 1080x720. With
`-Capture`, two local suites and three real Chromium scenarios transition from
queued/running through capture and comparison to passed in the Activity view.

First improvement pass gates: 347 library/UI tests and two standalone-worker
integration tests pass, including execution with no CLI binary beside the app.
The default CLI gate passes 281 tests. Formatting and strict production-library
Clippy pass. Ten native window captures passed nonblank pixel checks, with the
overview, compact editor, live activity, and final scenario outcomes inspected.
The native worker completed all three local Chromium scenarios successfully;
startup stderr was empty. CI definitions are updated but were not run remotely.

## Usability Refinements

- [x] Search suite paths and scenario labels, IDs, and URLs across the workspace;
  open matching scenarios directly while preserving save/discard destinations.
- [x] Expand or collapse suite outlines and filter the navigator to selected suites.
- [x] Search settings, including advanced properties and descriptions; filter to
  overrides and collapse groups without rebuilding retained field controls.
- [x] Add inherited-value numeric steppers, inline file pickers, selection headings,
  and document/parent-scenario breadcrumbs.
- [x] Reveal and focus invalid fields, reset one override without losing other
  invalid drafts, and flush pending text before navigation and structural edits.
- [x] Save navigator visibility and add keyboard access to search, settings,
  Editor, Runs, and Results without discarding view state.
- [x] Keep result previews synchronized with filters and searches; provide
  capture navigation, failure jumps, automatic pagination, and actual-size reset.
- [x] Expand comparison previews without losing loaded images, filters, or zoom;
  verify full-width and compact layouts using native window captures.
- [x] Fix Windows debug startup stack exhaustion with a GUI-only linker reserve.
- [x] Extend native smoke coverage, regression tests, and the desktop guide;
  commit each completed unit.

Refinement gates: 358 library/UI tests (including 30 production UI interaction
tests), two standalone-worker integration tests, and 281 default CLI tests pass.
Formatting and strict production-library Clippy pass. The final Windows native
smoke run generated 13 nonblank captures at 1600x1000 and 1080x720, exercised
layout toggles and actual-size previews, and passed all three local Chromium
scenarios across two suites. Editor, comparison, and completed activity captures
were inspected; startup stderr was empty. Native macOS/Linux, full IME, and
screen-reader end-to-end validation remain platform follow-ups. Remote CI was
not executed during this pass.

## Authoring Improvements

- [x] Add local schema-aware YAML/JSON completion using GPUI's native menu and
  background syntax parsing, including unfinished properties and containers.
- [x] Scope property suggestions to workspace, suite, scenario, action, or
  viewport; exclude duplicate sibling keys and support boolean/browser values.
- [x] Support Ctrl+Space, arrow selection, Enter acceptance, and Escape dismissal
  without inserting extra newlines or moving the caret behind the menu.
- [x] Add source Find/Replace/Suggest controls, wrap and text-size settings,
  correct language highlighting, cursor status, syntax underlines, and error jumps.
- [x] Offer checked viewport/dependency choices alongside free-form lists,
  preserving inherited values, custom entries, and draft-derived scenario IDs.
- [x] Show scenario action sequences with targets and icons; reuse document
  guards/history for reorder, duplicate, direct editing, and confirmed removal.
- [x] Extend native coverage to authoring menus and completion acceptance/save;
  verify focus and stable bounds before input without changing keyboard settings.
- [x] Handle the test app's unsaved-changes popup by discarding only its synthetic
  draft, verifying normal shutdown and unchanged saved source.

Authoring gates: 373 library/UI tests (including 36 production UI tests), two
standalone-worker integration tests, 56 headless desktop model tests, and 281
default CLI tests pass. Formatting and strict library Clippy pass. Windows native
smoke verified 18 nonblank screenshots at 1600x1000 and 1080x720, accepted and
saved a JSON completion, selected custom viewport choices, and passed three local
Chromium scenarios across two suites. Dirty-fixture cleanup exited normally
without changing the saved file. Native startup stderr was empty. Completion,
source, action-menu, and compact-editor captures were inspected. Full IME,
screen-reader, and native macOS/Linux validation remain outstanding; remote CI
was not run by the assistant during this pass.

## Navigation and Review

- [x] Replace the nested sidebar with separate resizable suite and scenario
  panes, independent searches, aligned selection controls, and workspace files.
- [x] Show scenarios only for the open suite, retain draft guards on navigation,
  and display readable page locations and live suite status.
- [x] Format displayed paths with forward slashes and remove Windows verbatim
  prefixes while preserving network shares and canonical filesystem paths.
- [x] Persist captured scenario labels in optional report metadata and resolve
  legacy names from unambiguous workspace IDs without changing matching keys.
- [x] Replace generated run UUIDs with report timestamps, retain raw IDs in
  search/tooltips, and show capture context and mismatch thresholds separately.
- [x] Add copyable capture details and guarded navigation to the owning suite.
- [x] Extend Windows native checks to suite switching, readable generated-ID
  reports, label metadata, normalized paths, and DPI-correct pointer input.

Verification: 381 library/UI tests (including 39 production UI tests), two
standalone-worker tests, 60 headless desktop model tests, and 281 default CLI
tests pass. Formatting and strict library Clippy pass. Native Windows checks
passed 19 nonblank captures at 1600x1000 and 1080x720, three local Chromium
scenarios, and fixture-only draft cleanup without changing saved source. Compact
sidebar and report screenshots were inspected. GPUI 0.6.1 can retain a tooltip
after programmatic view changes; its overlay handle is private, so the ineffective
window-refresh probe was removed and no dependency internals were modified.
Native macOS/Linux and full accessibility/IME validation remain follow-ups.

## Appearance and Efficiency

- [x] Expose System/Light/Dark in the header, migrate legacy preferences, and
  follow OS appearance changes only when System is selected.
- [x] Unify component palettes and restore keyboard-focus rings; test principal
  text/status color pairs at a minimum 4.5:1 contrast in both themes.
- [x] Persist source wrapping, line numbers, and bounded text size, with a reset
  control that does not alter source or undo history.
- [x] Serialize/coalesce preference writes independently from document busy
  state, report write failures, support retries, and flush settings on close.
- [x] Add a searchable native command palette with keyboard activation,
  contextual disabled states, and existing draft/confirmation guards.
- [x] Verify native dark/light menus, source, results, command rows, and restart
  persistence while restoring the original user preference file after testing.

Verification: 390 library/UI tests (including 45 production UI tests), two
standalone-worker tests, 62 headless desktop tests, and 281 default CLI tests
pass. Formatting and strict library Clippy pass. The Windows smoke workflow
passed 28 nonblank captures at 1600x1000 and 1080x720, three local Chromium
scenarios, appearance/display restoration after restart, and fixture-only draft
cleanup. Dark command-menu, compact source, and result captures were inspected.
Original user preferences were restored. System-change behavior is covered by
the preference model and simulated native appearance events; no OS-wide theme
settings were changed. Native macOS/Linux and full screen-reader/IME validation
remain outstanding, along with the previously documented GPUI tooltip issue.

## Direct Command Workflow Review

- [x] Replace command modes and generic Run with Run Test, Run Reference,
  Approve Results, and Compare Results; keep secondary commands in More actions.
- [x] Run the open suite directly from Editor and command search, and checked
  suites directly from Overview. Empty selection never means every suite.
- [x] Keep Run settings available before any command, searchable by group, and
  retain invalid drafts without blocking unrelated commands.
- [x] Freeze command, options, and exact suite names through confirmation and
  Save and continue / Run saved version; revalidate those names against disk.
- [x] Distinguish visual results, capture-only completion, infrastructure
  failure, and cancellation. Label the executed command independently of the
  next request and provide direct results, comparison, and log actions.
- [x] Pin View Results and report approval to the executed or reviewed run.
  Reject changed suite output identities instead of approving different images.
- [x] Default followup approve/compare to the last matching run while keeping
  subsequent test/reference captures fresh.
- [x] Flush pending field/source text before switching editor views; preserve
  invalid source for repair. Share contextual file-action availability between
  File and command search.
- [x] Extend native acceptance with reviewed-run checksum validation, exact
  comparison, direct reference capture, Stop/rerun, and compact themed settings.
- [ ] Complete the expanded native workflow through final results, workspace
  initialization, and restart on an available interactive Windows desktop.

Verification: all 400 library/UI tests and two standalone-worker tests pass,
including regressions for direct commands, immutable run intent, independent
settings, exact report targeting, authoring transitions, and stopped-run state.
The 281 default CLI tests and 62 headless desktop tests also pass. Formatting,
strict library Clippy, and the standalone Windows build pass.

Native execution reached historical approval despite a newer capture directory,
checksum equality with the reviewed image, exact-run comparison, three reference
captures, cancellation, and three passing rerun scenarios. The first extended
attempt stopped on an incorrect assumption that previous reports were retained.
The corrected check validates reference and test reports at their respective
completion points; repeat attempts stopped safely because Windows refused
foreground input. No input was sent to an unverified window, and original user
preferences were restored. Reviewed light/dark and compact screenshots informed
the final prior-run labels and cancellation colors; those final adjustments have
automated coverage but still need the complete native rerun. Prior appearance
and restart validation remains recorded above, not claimed as a fresh full pass.

## Scenario Controls and Logs

- [x] Show persistent native overflow scrollbars across the workbench and native
  text editors; retain independent image scrolling and correct Fit width sizing.
- [x] Support exact repeated scenario filters through CLI and bundled workers
  for Test, Reference, Compare, and Approve. Keep prerequisite definitions while
  filtering capture jobs and report manifests.
- [x] Add scenario checkboxes, mixed suite states, direct scenario actions, and
  scope counts across the sidebar, editor, Runs, overview, and command search.
- [x] Preserve exact scenario IDs through drafts, confirmation, and fresh disk
  validation. Empty, removed, ambiguous, and renamed selections never widen.
- [x] Scope artifact operations by recorded metadata and preflight every selected
  output before approval; preserve sibling references and legacy whole-run usage.
- [x] Attribute navigation/action/capture messages to job IDs before output
  interleaves. Bound retained history and omit typed input values.
- [x] Open searchable/copyable logs from Activity rows and scenario log buttons,
  aggregate viewports safely, and preserve comparison-only scenario identity.
- [x] Validate native scenario-only capture, live log content, results, scoped
  approval/comparison, unchanged sibling hashes, and both theme/layout sizes.

Verification: 417 library/UI tests (60 production UI tests), three bundled-worker
integration tests, 290 default CLI tests, and 65 headless desktop tests pass.
Strict library Clippy, formatting, and the standalone Windows build pass.
`ci/desktop-smoke.ps1 -ScenarioFlows` completes with 11 nonblank window captures
at 1600x1000 and 1080x720. Captured images were inspected; a Fit width regression
was corrected and tested against actual preview/content bounds.

The full authoring smoke remains incomplete: it reached viewport/dependency
menus and then stopped when synthetic source input produced an invalid draft
without a completion popup. The focused feature flow avoids those unrelated
typing steps and does not replace their outstanding full native check. Original
user settings were restored and fixture processes exited. Native macOS/Linux,
full screen-reader/IME validation, and the prior GPUI tooltip limitation remain
as documented above.

## Report Consistency

- [x] Expose Open Report after testing from every Runs tab and open the executed
  run's reference/current comparison side by side.
- [x] Compare capture-only runs before opening their report, preserving the
  original output, run ID, suite/scenario scope, and draft guard.
- [x] Count individual screenshot comparisons in the final report summary while
  keeping scenario/job progress distinct.
- [x] Derive visual row status from comparison outcomes rather than retaining a
  previous capture/cleanup failure forever. Preserve infrastructure errors and
  nonzero command semantics independently; keep generated reports accessible.
- [x] Reproduce and fix the passing-comparison/failed-live-row transition, and
  verify HTML, saved JSON, desktop reading, and worker telemetry agree for nine
  passes and three failed captures across four scenarios and three viewports.

Verification: 423 library/UI tests (63 production UI tests), four worker tests,
293 default CLI tests, and 65 headless desktop tests pass, together with strict
library Clippy and formatting. The standalone Windows desktop builds. Supplied
report artifacts were inspected read-only; no captures or references were
changed. The Windows smoke action is updated to Open Report, but native input
checks were not rerun while the user's desktop window remained open.

## Saved Artifact Discovery

- [x] Discover baseline images and saved capture runs by suite output ID when
  opening/selecting suites, after commands, and after output/environment changes.
- [x] Scan asynchronously with generation checks; retain same-folder results
  during refresh and reject stale results after changing folders/workspaces.
- [x] Disable all desktop Test routes without a successful baseline in every
  selected suite, keep Reference available, and recheck disk during preflight.
- [x] Exclude empty folders, diff files, invalid PNGs, and failed placeholders
  from baseline availability without changing standalone CLI semantics.
- [x] Open existing reports directly from Editor, Overview, and Runs, including
  reports discovered after restart. Compare capture-only data without recapture.
- [x] Pin output and run identities, select newest captures by timestamp, and
  support existing multi-suite reports with independent run IDs.
- [x] Exercise the native no-baseline, discovered-baseline, existing-report,
  compare-only, shared-run-ID, and restart paths with unchanged image hashes.

Verification: 432 library/UI tests, four worker tests, 293 default CLI tests,
and 68 headless desktop tests pass. Strict library Clippy, formatting, and the
normal Windows desktop build pass. `ci/desktop-smoke.ps1 -ArtifactFlows` passes
with seven inspected light/dark captures at 1600x1000 and 1080x720, without browser
capture or network access. Original preferences were restored and no fixture
processes remain. Native testing found and fixed duplicate accessibility IDs in
empty Activity and a compare-and-open handoff incorrectly tied to unrelated
checkbox selections. Disabled buttons are checked through Invoke availability
because GPUI does not expose their disabled state through Windows IsEnabled.

The preceding full authoring-smoke limitation remains separate from this focused
pass; native macOS/Linux and full screen-reader/IME checks remain outstanding.

## Workspace Instances

- [x] Replace the global GUI lock with per-workspace ownership; permit multiple
  distinct workspace windows and multiple empty instances.
- [x] Resolve canonical path aliases and Windows case variants to the same owner.
  Serialize simultaneous launch contenders using an OS-held file lock.
- [x] Forward duplicate startup, folder-choice, and recent-workspace requests to
  a token-checked local activation endpoint; restore/focus the existing owner.
- [x] Preserve the requester's drafts and running work during duplicate handoff.
  Keep normal save/discard and running-job guards for an actual workspace switch.
- [x] Retain old ownership until a new workspace successfully loads; release on
  Close workspace and process exit, and recover stale endpoint data after crashes.
- [x] Keep internal workers independent of GUI ownership and coordinate shared
  preference writes without dropping other windows' recent workspaces.
- [x] Verify two workspaces, multiple empty instances, minimized-owner activation,
  dirty-draft preservation, recent-workspace handoff, and close/crash recovery in
  native Windows acceptance with fixture-only cleanup.

Verification: 438 library/UI tests, five executable integration tests, 293 default
CLI tests, and 72 headless desktop tests pass. Strict library Clippy, formatting,
and the normal Windows GUI build pass. `ci/desktop-smoke.ps1 -InstanceFlows` passes
with seven inspected 1600x1000 captures and actual foreground/minimized-state
assertions. Original preferences are restored, saved fixture hashes are unchanged,
and only test-owned processes are closed or terminated.

Native test setup initially hit Windows foreground restrictions; duplicate
activation itself then passed and was used to activate the recovered fixture
before its pointer-driven Close workspace action. Missing cached rlib archives
were rebuilt for three affected dependencies before the full test gate passed.
Native macOS/Linux activation and the previously recorded full authoring-smoke,
screen-reader, and IME follow-ups remain unverified.

## Delivery Checklist

### 1. Foundation

- [x] Extract a shared library without changing CLI behavior.
- [x] Add an optional GPUI Kit desktop target and bundled icon assets.
- [x] Build a headless document model with validation and undo/redo.
- [x] Establish focused model tests and a desktop build check.

Acceptance: the default CLI builds without GUI dependencies; the optional desktop
target compiles; document operations are testable without a display.

### 2. Workspace and Document Management

- [x] Open a workspace using a native folder chooser or startup path.
- [x] Discover supported suite files and workspace/viewport configuration.
- [x] Create missing starter files using the existing non-destructive init flow.
- [x] Create, duplicate, rename, and delete suite files with confirmation.
- [x] Track unsaved changes; guard document/workspace changes and window close.
- [x] Save, save as, reload, and detect externally modified files.
- [x] Persist recent workspaces and desktop preferences outside test data.
- [x] Reject writes outside the workspace, including symlink escapes.

Acceptance: existing project files are not overwritten on initialization;
cancellation, unsaved changes, invalid paths, and conflicting saves are tested.

### 3. Guided Editing

- [x] Edit all workspace settings using labeled, typed fields.
- [x] Edit suite settings and create, duplicate, rename, reorder, and remove
      scenarios.
- [x] Edit scenario settings, dependencies, restore state, URLs, viewports,
      capture selectors, waits, and overrides.
- [x] Add, duplicate, reorder, and remove every supported action type: check,
      click, focus, goto, hide, hover, input, persist, press, remove, save, select,
      scroll, uncheck, and wait.
- [x] Support frame chains, file uploads, optional values, and action-specific
      fields without changing the existing serialization format.
- [x] Distinguish inherited values from explicit false, zero, and empty values;
      show resolved values and offer reset-to-inherited controls.
- [x] Edit viewport definitions.
- [x] Provide YAML/JSON source editing, parse/type diagnostics, and undo/redo.
- [x] Keep guided editing and source editing synchronized through one document.

Acceptance: every serialized property has a guided or structured field; unknown
properties and invalid types surface as diagnostics; editing does not introduce
unrequested overrides; action ordering and YAML/JSON round trips are tested.

### 4. Run Management

- [x] Run test, ref, compare, approve, snapshot, optimize, install, and init.
- [x] Offer applicable suite filters and global options, including
      environment, output directory, concurrency, ramp interval, and debug mode.
- [x] Launch with argument arrays, never shell-interpolated commands.
- [x] Keep execution off the UI thread and show bounded stdout/stderr logs.
- [x] Support cancellation and clean up the child process tree.
- [x] Prevent overlapping mutating commands in the same desktop session.
- [x] Confirm baseline promotion and other destructive operations.
- [x] Distinguish command failure, cancellation, and visual/scenario failure.

Acceptance: CLI argument generation is checked against the actual clap parser;
spaces and special characters remain single arguments; process completion and
cancellation are tested; exit zero is never treated as proof of a visual pass.

### 5. Results and Workbench

- [x] Discover existing comparison reports and refresh after commands finish.
- [x] List scenario/viewport outcomes with pass, mismatch, and capture-error
      states and filtering.
- [x] Display reference, current, and diff images with fit/zoom controls.
- [x] Open generated HTML reports and artifact folders externally.
- [x] Provide a searchable suite/scenario navigator, resizable panels, clear
      empty/error/loading states, and keyboard-accessible editing controls.
- [x] Provide keyboard shortcuts for save, undo, redo, and document operations.
- [x] Use consistent labels, concise tooltips, and inline validation.

Acceptance: missing/malformed reports and images are recoverable UI states;
artifact paths cannot escape the selected output directory; large images/logs
do not block the UI thread indefinitely.

### 6. Verification and Documentation

- [x] Add focused unit tests for documents, editing, schema coverage, validation,
      paths, persistence, command arguments, subprocesses, and result discovery.
- [x] Add GPUI interaction/render tests where supported by the framework.
- [x] Run the existing CLI regression tests and both default/desktop build gates.
- [x] Exercise the desktop application on Windows, recording any GUI checks
      that cannot be automated in the available environment.
- [x] Document installation, launching, editing, inheritance, running, result
      semantics, keyboard shortcuts, limitations, and contributor test commands.
- [x] Update this checklist as work is completed; distinguish implemented,
      verified, and environment-blocked work.

## Non-Goals

This implementation does not add a browser interaction recorder, cloud accounts,
collaborative editing, non-Chromium automation, or new test execution semantics.
Those require separate designs. It also does not replace the existing HTML
report generator or rewrite browser actions.

## Verification Strategy

Start with the narrowest document/model test after each change. Validate the
CLI/library extraction with existing tests before adding rendering. Compile a
minimal GPUI window before expanding its interface. Add regression cases for
every discovered defect. Finish with formatting, the full local test suite,
desktop compilation, and a native smoke test. Do not mark a manual or visual
check complete merely because compilation passed.

## Verification Record

- Shared-library extraction: `cargo test --all-targets` passed all 276 existing
  tests on Windows. The CLI entry point still owns process initialization and
  exit handling; the library exposes the existing modules without changing them.
- GPUI bootstrap: `cargo check --features gui --bin regressify-desktop` passed on
  Windows with GPUI Kit 0.6.1. This checks compilation, not visual interaction.
- Document model: nine focused tests pass for YAML/JSON edits, nested comment
  preservation, empty and indentationless lists, history limits, validation,
  multi-document rejection, and all 15 action templates. The model rejects a
  generated edit if its parsed meaning differs from the requested change.
- Workspace persistence: six Windows tests pass for create/save/reload,
  external-change conflicts, duplicate-key rejection, suite copy/rename/delete,
  portable path containment, and bounded recent-workspace preferences. A Unix
  symlink-escape regression is platform-gated.
- Guided schema: four tests verify exact serialized field coverage, all action
  properties, explicit empty/false values, and the production configuration
  cascade including combined state/restore values. Schema derives are enabled
  only with the headless desktop feature.
- Execution model: seven tests pass for all CLI command arguments, invalid
  option rejection, output-directory precedence, real stdout/stderr capture,
  command failure, cancellation, and bounded log memory. The current CLI supports
  suite execution filters, not a per-scenario execution flag; scenario filtering
  in the desktop navigator does not silently change run scope.
- Result model: three tests pass for pass/mismatch/engine-error classification,
  malformed-report recovery, historical run discovery, artifact containment,
  missing files, and PNG dimension validation using the existing CLI helper.
- Editor session: four integration-style model tests cover all 15 action types,
  scenario duplication without ID collisions, selection repair, grouped undo,
  unsaved navigation guards, and rollback of failed compound edits. All 33
  desktop model tests pass together.
- First native editor: desktop compilation and three GPUI production-view tests
  pass on Windows. Tests type into fields, save asynchronously, duplicate/undo
  scenarios, and verify that invalid source exposes an alert and cannot save.
  GPUI input subscriptions are deferred; tests wait outside window-update borrows.
- Native file workflows: six production-view tests pass, including cancel and
  discard/reload, JSON suite creation through the name dialog, and confirmed
  item removal with undo. Dialog status uses a separate retained entity to
  avoid reading the workbench during its render/update borrow. Animation tests
  use the same test-thread wall-clock wait as GPUI Kit's own overlay suite.
- Native runs: eight UI tests pass, including baseline confirmation before any
  process starts, command-specific option visibility, and switching between run
  and editor views without changing the draft. Model tests cover the actual
  subprocess lifecycle. Save-and-run re-enables execution after saving completes.
- Native results: ten workbench UI tests pass, including real PNG/report fixture
  loading, reference/current side-by-side mode, zoom, failed-capture filtering,
  engine error details, and recovery from missing output directories. These are
  layout/state checks; native pixels still require the final smoke test.
- Usability pass: twelve native UI tests and seven workspace tests pass. Added
  inherited source/value labels, live effective configuration, file pickers with
  upload-relative paths, saved appearance, file-operation shortcuts, and stable
  report/capture selection across refreshes. Both binaries build successfully;
  the preceding combined regression gate passed all 319 tests.
- Windows native smoke: `ci/desktop-smoke.ps1` builds both executables, opens a
  synthetic workspace with UI Automation, navigates fields/source/results,
  renders comparison PNGs, and runs the real CLI init command from the GUI.
  Six target-window screenshots passed nonblank pixel sampling and were visually
  inspected at 1600x1000 and 1080x720. Startup stderr was empty. Direct
  `PrintWindow` capture excludes other applications even when they overlap.
- Draft safety: 49 desktop tests pass after adding regressions for invalid field
  retention during structural commands, immediate saves before deferred input
  notifications, source-buffer synchronization, and older async save completion
  retaining newer edits as dirty.
- Run scope: eight workspace tests and fifteen UI tests pass after reusing the
  CLI's discovery routine to reject shadowed selected files. Filename extension
  handling is case-insensitive and the reserved `all` name cannot accidentally
  turn a selected-file run into an all-suite run.
- Final local gates: `cargo test --locked --all-targets` passed 276 CLI tests;
  `cargo test --locked --features gui-tests --all-targets -- --test-threads=1`
  passed 327 tests (including 15 production GPUI interactions). Locked GUI/CLI
  builds, `cargo fmt --check`, and strict production-library Clippy passed.
- Added Windows GUI/CLI and Linux headless-model CI jobs. These jobs have not
  been executed on GitHub in this session. Native macOS/Linux rendering, full
  IME composition, screen-reader usability, signing, and installer packaging
  remain platform/release follow-up checks, not locally verified deliverables.
- Suite-edit regression: reproduced the color-blender error where a root field
  was inserted inside an unquoted CSS attribute selector. Plain scalar spans are
  now protected before CST editing and restored exactly afterward. Added tests
  for both reported fields, nested edits, all shipped suites, Unicode, LF/CRLF,
  inline comments, anchors, undo, and real GPUI typing/save behavior.
  Verification: all 333 feature-enabled tests and 276 default CLI tests pass;
  strict production Clippy, formatting, and locked GUI/CLI builds also pass.
