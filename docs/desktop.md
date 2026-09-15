# Regressify Desktop

Regressify Desktop is an optional Rust-native GPUI Kit application. It shares
the CLI's Rust library and keeps YAML/JSON files as the workspace source of truth.
See [the implementation plan](desktop-plan.md) for delivery and verification status.

## Build

Use the stable Rust toolchain and the native build tools for your platform. On
Windows, install the Visual Studio C++ build tools and Windows SDK.

```sh
cargo build --features gui --bins
cargo run --features gui --bin regressify-desktop
cargo run --features gui --bin regressify-desktop -- /path/to/workspace
```

The `gui` feature is opt-in. `cargo build` and `cargo run -- test` continue to
build/run the CLI without GPUI dependencies. Keep `regressify` alongside the
desktop executable when distributing the application; commands execute in a
separate CLI process so run-specific state is isolated.

## Contributor Checks

```sh
cargo test --all-targets
cargo test --features desktop --lib desktop::
cargo check --features gui --bin regressify-desktop
cargo test --features gui-tests --lib desktop::ui::tests -- --test-threads=1
```

A successful compile does not replace a native smoke test. Verify keyboard
focus, file dialogs, text input/IME, resizing, and image rendering on the target
platform before shipping. Detailed editing and execution documentation will
accompany the corresponding implementation units.

## Document Guarantees

The headless desktop model can be built/tested with `--features desktop` without
compiling GPUI. Guided YAML changes use a concrete syntax tree to retain comments;
JSON edits use pretty-printed JSON. Every guided change is parsed again and
compared to the requested semantic edit. If a YAML construct cannot be handled
losslessly, the operation fails and the original source remains untouched.

Source drafts may be temporarily invalid. Validation is a mandatory save gate.
Undo/redo stores exact source snapshots, limited to 100 entries and a 16 MiB undo
budget. Individual documents are limited to 4 MiB for interactive editing.

Saves compare the file against its last loaded version and refuse to overwrite
external edits. Reload explicitly discards the draft; Save As preserves it in a
new file and never replaces an existing destination. Changing format with Save As
converts the data and does not preserve YAML comments in JSON.

Suite copies drop the suite-level `id` to prevent sharing output artifacts with
the original. Renaming keeps IDs unchanged and requires saved content. File
deletion is restricted to suites and requires a confirmation in the interface.
Recent workspaces and the selected CLI executable are stored in the operating
system's per-user application configuration directory, outside the test project.

## Guided Fields and Inheritance

The field catalog is generated from the existing Rust types with `schemars`.
Collections of scenarios and actions have dedicated editors; generated runtime
`testDate` metadata remains available in Source. A new serialized config or
action property therefore appears in the field catalog without a parallel
handwritten schema.

An absent property inherits from its parent. Explicit `false`, zero, an empty
string, or an empty list is an override, not a request to inherit. Resetting a
field removes the property. Effective configuration uses the CLI resolver with
the scenario, suite, workspace, and built-in defaults in that order. State and
restore names use the existing merge behavior rather than simple replacement.

## Execution

Each command runs a fresh CLI process in the selected workspace. Arguments are
validated by the real clap parser and passed as an argument array, never through
a shell. Capture commands generate a unique run ID when one is not supplied.
Only one command may run at a time in a desktop session.

Test, reference capture, compare, and approve support suite filters. The current
CLI has no single-scenario execution flag: selecting a scenario for editing does
not imply that only that scenario will run. Reference capture and approval
promote baselines and require confirmation, as does image optimization.

Cancel terminates the process group, including its child processes. Closing the
application cancels an active command before disposing of its execution worker.
Logs are capped at 2 MiB; a bounded producer queue may omit chunks if output
outpaces the UI. Completion is tracked separately so a full log queue cannot
hide the command's final state.

`Completed` means the process exited zero. It does **not** mean visual tests
passed. Scenario failures and mismatches are read from generated reports.

## Results

The results browser reads `bitmaps_test/<run>/report.json` under each suite's
output directory. It keeps historical runs distinct and lists the newest first.
An invalid report produces a warning without hiding valid reports. Discovery
loads at most 1,000 reports, with a 16 MiB limit per report.

Pass, visual mismatch, and engine/capture error are separate outcomes. Unknown
future statuses are never shown as passed. Reference, test, and diff paths are
resolved relative to `html_report`, matching the report generator, and must stay
inside the selected output directory. Missing images remain recoverable errors.
PNG dimensions are bounded before decoding with the same limits as the CLI.

## Structural Editing

Scenarios, actions, and viewports support insertion, duplication, reordering, and
removal. Compound operations form one undo step. Duplicated scenarios omit their
explicit ID and receive a unique label; their original actions are retained.
Selections are repaired after deletions and undo so a stale list index cannot
silently edit a different item.

New nested values use YAML flow notation to remain valid at any indentation.
Existing document nodes and their comments are retained. JSON flow notation is
valid YAML, so these files remain compatible with the existing CLI parser.

## Editor Workflow

Open a workspace, then select a configuration file or test suite in the left
navigator. Select a scenario to edit its fields and reveal its actions. The Add
menu offers scenarios, viewports, and every supported action. Duplicate and
move controls operate on the selected item.

Fields presents guided controls; Advanced reveals all optional root settings.
Source exposes the complete YAML/JSON with syntax highlighting and built-in
search. Effective displays the resolved scenario configuration. Invalid source
or field values are retained as drafts and block saving.

Use Ctrl+S to save, Ctrl+O to open a workspace, Ctrl+Z to undo, and Ctrl+Shift+Z
to redo (Cmd on macOS). Text inputs also retain their native editing shortcuts.
The toolbar undo/redo commands operate on the complete document history.

The `gui-tests` feature enables GPUI's headless interaction instrumentation.
These tests operate on the production view, not a duplicate test-only UI. They
verify events, state, and layout bounds, but are not pixel screenshots or full
operating-system IME tests.

The File menu provides suite creation, duplication, renaming, Save As, reload,
deletion, configuration creation, opening additional workspace files, refresh,
and light/dark appearance. Save As expects a workspace-relative destination.
Opening another file/workspace or closing with a draft prompts Save, Discard,
or Cancel. A failed save keeps the dialog and draft available for recovery.
Removing an item requires confirmation and remains undoable; deleting a suite
file is a filesystem operation and cannot be undone through document history.

## Run Panel

Runs provides the supported command menu, an explicit suite scope, applicable
typed options, and a searchable read-only log. Run suite selects the current
suite and opens Runs; the Run button starts execution after any required draft
save/discard and artifact-change confirmation. An empty suite filter means all
suites. Select Initialize workspace to run the existing non-destructive init.

The CLI picker persists a trusted executable path in per-user preferences.
Proxy input is masked in the option editor and command lines are not echoed.
Cancellation stays available while execution is active. Switching workspaces is
blocked during a command. Closing during a run requests cancellation; close again
after the command has stopped to handle any unsaved document prompt.

## Reviewing Captures

Results refreshes from the current environment/output options on opening and
after commands finish. Choose a suite/run from the report menu, filter All,
Failed, or Passed, and search capture labels or error messages. Capture lists
are paginated in groups of 50 to keep large reports responsive.

Reference, Current, Diff, and Side by side views use background-loaded PNGs.
Fit width and zoom controls support tall-page inspection with scrollbars.
Engine errors are shown separately from mismatch percentages. The external-open
controls open the generated HTML summary, output directory, or individual image
with the operating system's default application. These are trusted local
artifacts; no report HTML is embedded in the desktop process.