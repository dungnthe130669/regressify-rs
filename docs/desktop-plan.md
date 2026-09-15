# Regressify Desktop Implementation Plan

## Goal

Provide a local, Rust-native GPUI Kit application in which QA can maintain
Regressify workspaces, configure suites and scenarios, execute the existing CLI,
and review visual results without manually constructing commands. YAML and JSON
remain the source of truth. The CLI remains usable without GUI dependencies.

## Architecture

- An optional `regressify-desktop` binary uses GPUI Kit for the desktop interface.
- A shared Rust library exposes the existing configuration, action types, and
  validation. The CLI keeps its command and exit-code contracts.
- Desktop document, workspace, execution, and result models are independent of
  GPUI so their behavior can be tested without opening a window.
- Each command executes a fresh CLI child process. Process-wide `OnceLock` values
  must never leak from one desktop run or workspace to another.
- File access is scoped to the selected workspace. Writes detect external
  changes and use atomic replacement. Invalid edits never overwrite good files.
- Structured editing preserves YAML comments where supported by the document
  parser. Unsupported document constructs must produce a clear diagnostic,
  never silently discard data. Source editing remains available.

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

- [ ] Add focused unit tests for documents, editing, schema coverage, validation,
  paths, persistence, command arguments, subprocesses, and result discovery.
- [ ] Add GPUI interaction/render tests where supported by the framework.
- [ ] Run the existing CLI regression tests and both default/desktop build gates.
- [x] Exercise the desktop application on Windows, recording any GUI checks
  that cannot be automated in the available environment.
- [ ] Document installation, launching, editing, inheritance, running, result
  semantics, keyboard shortcuts, limitations, and contributor test commands.
- [ ] Update this checklist as work is completed; distinguish implemented,
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