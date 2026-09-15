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

- [ ] Open a workspace using a native folder chooser or startup path.
- [ ] Discover supported suite files and workspace/viewport configuration.
- [ ] Create missing starter files using the existing non-destructive init flow.
- [ ] Create, duplicate, rename, and delete suite files with confirmation.
- [ ] Track unsaved changes; guard document/workspace changes and window close.
- [ ] Save, save as, reload, and detect externally modified files.
- [ ] Persist recent workspaces and desktop preferences outside test data.
- [ ] Reject writes outside the workspace, including symlink escapes.

Acceptance: existing project files are not overwritten on initialization;
cancellation, unsaved changes, invalid paths, and conflicting saves are tested.

### 3. Guided Editing

- [ ] Edit all workspace settings using labeled, typed fields.
- [ ] Edit suite settings and create, duplicate, rename, reorder, and remove
  scenarios.
- [ ] Edit scenario settings, dependencies, restore state, URLs, viewports,
  capture selectors, waits, and overrides.
- [ ] Add, duplicate, reorder, and remove every supported action type: check,
  click, focus, goto, hide, hover, input, persist, press, remove, save, select,
  scroll, uncheck, and wait.
- [ ] Support frame chains, file uploads, optional values, and action-specific
  fields without changing the existing serialization format.
- [ ] Distinguish inherited values from explicit false, zero, and empty values;
  show resolved values and offer reset-to-inherited controls.
- [ ] Edit viewport definitions.
- [ ] Provide YAML/JSON source editing, parse/type diagnostics, and undo/redo.
- [ ] Keep guided editing and source editing synchronized through one document.

Acceptance: every serialized property has a guided or structured field; unknown
properties and invalid types surface as diagnostics; editing does not introduce
unrequested overrides; action ordering and YAML/JSON round trips are tested.

### 4. Run Management

- [ ] Run test, ref, compare, approve, snapshot, optimize, install, and init.
- [ ] Offer applicable suite/scenario filters and global options, including
  environment, output directory, concurrency, ramp interval, and debug mode.
- [ ] Launch with argument arrays, never shell-interpolated commands.
- [ ] Keep execution off the UI thread and show bounded stdout/stderr logs.
- [ ] Support cancellation and clean up the child process tree.
- [ ] Prevent overlapping mutating commands in the same desktop session.
- [ ] Confirm baseline promotion and other destructive operations.
- [ ] Distinguish command failure, cancellation, and visual/scenario failure.

Acceptance: CLI argument generation is checked against the actual clap parser;
spaces and special characters remain single arguments; process completion and
cancellation are tested; exit zero is never treated as proof of a visual pass.

### 5. Results and Workbench

- [ ] Discover existing comparison reports and refresh after commands finish.
- [ ] List scenario/viewport outcomes with pass, mismatch, and capture-error
  states and filtering.
- [ ] Display reference, current, and diff images with fit/zoom controls.
- [ ] Open generated HTML reports and artifact folders externally.
- [ ] Provide a searchable suite/scenario navigator, resizable panels, clear
  empty/error/loading states, and keyboard-accessible editing controls.
- [ ] Provide keyboard shortcuts for save, undo, redo, and document operations.
- [ ] Use consistent labels, concise tooltips, and inline validation.

Acceptance: missing/malformed reports and images are recoverable UI states;
artifact paths cannot escape the selected output directory; large images/logs
do not block the UI thread indefinitely.

### 6. Verification and Documentation

- [ ] Add focused unit tests for documents, editing, schema coverage, validation,
  paths, persistence, command arguments, subprocesses, and result discovery.
- [ ] Add GPUI interaction/render tests where supported by the framework.
- [ ] Run the existing CLI regression tests and both default/desktop build gates.
- [ ] Exercise the desktop application on Windows, recording any GUI checks
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