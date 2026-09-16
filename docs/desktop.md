# Regressify Desktop

The optional GPUI Kit desktop app shares the CLI's Rust library and keeps
YAML/JSON files as the source of truth. It provides configuration authoring,
isolated CLI execution, and visual report review. See
[the implementation plan](desktop-plan.md) for scope and verification evidence.

## Build and Launch

Use stable Rust and native build tools. Windows requires Visual Studio C++ build
tools and the Windows SDK. Other GPUI platform prerequisites are documented at
<https://gpui-kit.com/docs/installation>.

```sh
cargo build --features gui --bins
cargo run --features gui --bin regressify-desktop
cargo run --features gui --bin regressify-desktop -- /path/to/workspace
```

Omit the workspace argument to use the native folder picker. The default
`cargo build` and `cargo run -- test` remain CLI-only. Feature `desktop` builds
headless models; `gui` adds native widgets; `gui-tests` adds UI instrumentation.

For distribution, use `cargo build --release --features gui --bin regressify-desktop`.
The desktop executable contains the engine; no separate `regressify` binary or
executable picker is needed. It launches itself in a private headless worker mode
to isolate run settings and browser processes. Signed installers and auto-update
are not included.

## Workspace and Files

The navigator lists workspace configuration, suites under `visual_tests`, and
YAML/JSON under `common`. Select a suite, then a scenario to reveal its actions.
Search filters labels. Open configuration file adds other workspace-relative
data files, including custom viewport definitions.

File supports new/duplicate/rename/delete suite, Save As, reload, configuration
and viewport-file creation, file browsing, refresh, and light/dark appearance.
Save As takes a workspace-relative destination and never replaces an existing
file. Reload discards a draft after confirmation. Suite-file deletion is
confirmed but cannot be undone through document history.

Opening another document/workspace or closing with a draft prompts Save,
Discard, or Cancel. A failed save retains the prompt and draft. Recent workspaces,
and appearance are stored outside the project in the
operating system's per-user Regressify Desktop configuration directory.

Only one desktop window and one workspace are active at a time. Close workspace
unloads documents, run options, suite selections, logs, and report previews.
Switching replaces the workspace rather than adding another root. Active runs
must finish or be cancelled before switching. A per-user instance lock prevents
a second GUI process; bundled worker processes do not take that lock.

Choose Initialize workspace in Runs to invoke the existing non-destructive
`init` command, which only creates missing starter files.

## Editing

Fields provides typed controls derived from the Rust schemas. Advanced reveals
optional root settings. Source exposes complete YAML/JSON with highlighting,
selection, native text editing, and search. Effective uses the real CLI resolver.

The Add menu supports scenarios, viewports, and all 15 action types. Selected
items can be duplicated, reordered, and removed. Removal is confirmed and
undoable. Compound changes form one undo step. Scenario copies omit explicit IDs
and receive unique labels; suite copies omit the suite ID to avoid sharing output
artifacts. Renaming preserves IDs.

All serialized configuration and action fields are represented by generated
controls. Scenarios/actions have dedicated collection editors; complex URL
replacement objects use structured YAML/JSON input. Generated runtime `testDate`
metadata remains available in Source. Frame chains, selector lists, dependencies,
restore state, and uploads retain the existing serialization shapes.

File fields offer native pickers. Configuration paths are workspace-relative;
input uploads are relative to `visual_tests`, including multi-file selection.
Out-of-base files are rejected. If the selected document/item changes while a
picker is open, its result is discarded rather than applied to another field.

### Inheritance

Absent properties inherit. Explicit `false`, zero, an empty string, or an empty
list is an override. Inherited fields show their effective value and source
(workspace, suite, or default); explicit values show Override. Reset removes the
property. Optional action fields do not inherit unrelated scenario settings.

Configuration cascades from scenario to suite to workspace to defaults. State
and restore use the CLI's merge behavior rather than simple replacement. Live
effective values update with guided edits. Unresolvable configuration produces
a diagnostic rather than a guessed value.

### Source and Save Safety

- Guided YAML edits retain existing comments and formatting using a concrete syntax tree. New nested values use valid YAML flow notation.
- Generated edits are parsed and checked against the requested semantic change. Unsupported constructs leave the draft untouched; use Source instead. Multiple YAML documents are rejected.
- JSON edits are pretty-printed. Save As format conversion preserves data, not YAML comments or formatting.
- Invalid drafts cannot save. Invalid field text blocks structural commands until corrected, reset, or undone.
- Saves detect external conflicts and use same-directory atomic replacement. Save As never replaces existing files. Paths reject workspace and symlink escapes.
- Immediate saves read retained native input. Completion acknowledges only the source written; newer edits stay dirty. This is conflict detection, not an OS-wide lock.
- Documents are limited to 4 MiB. Undo retains at most 100 exact-source snapshots with a 16 MiB budget.

Valid plain YAML strings such as `.item[aria-label='10 midpoints']` retain their
original spelling during guided edits. Adding suite fields does not require
quoting existing selectors. A YAML 1.2 span adapter protects these strings from
the lossless editor's flow-delimiter parsing, then restores them without changing
comments, Unicode text, or surrounding line endings.

Use trusted local workspaces. Configuration can reference scripts and browser
actions executed by the CLI. The desktop is not a sandbox for untrusted projects
or arbitrary executables.

## Running Commands

Runs offers test, reference capture, compare, approve, snapshot, optimize,
install, and init with applicable options. Suite checkboxes in the navigator and
Runs support multiple selections and Select all. No selection means Run is
disabled, never an implicit all-suite run. Invalid suites remain visible for
repair and are excluded from Select all. Run suite verifies the selected file,
sets its scope, and opens Runs;
Run starts execution after draft and artifact confirmations. Reference capture,
approve, and optimize require confirmation.

The CLI has no single-scenario run flag. Selecting a scenario for editing does
not restrict execution to it. Suite selections use exact escaped names;
compare and approve resolve each selected suite's explicit output ID.
For duplicate names, `.tests.yaml` takes precedence over `.tests.yml`, then
`.tests.json`. Run suite rejects a shadowed file and names the competing path.
Same-priority duplicates and reserved `all` selected-file runs are rejected.

Each command starts a fresh copy of the desktop executable as a headless engine
worker. Validated settings travel over stdin as structured JSON, never through
a shell or an external CLI executable. Process-wide state cannot leak between runs.
Captures generate unique run IDs unless supplied. One command runs per desktop
session; workspace switching is blocked during execution.

Cancel terminates the process group, including children. Closing during a run
requests cancellation; close again after it stops to handle unsaved work.
The live log is searchable, read-only, and capped at 2 MiB. Its bounded queue may
omit chunks under heavy output; the omission count is displayed. Completion is
tracked separately. Proxy input is masked and command lines are not echoed;
CLI/script logs can still contain project-provided data.

**Command completed means exit zero, not visual success.** Scenario failures,
missing references, and mismatches are report outcomes. Configuration,
infrastructure, and artifact-write errors are command failures.

The engine emits versioned progress events independently of console logs. Each
scenario/viewport job reports queued, running, capture completion, comparison,
and failure states. Screenshot keys connect visual comparisons to their original
jobs; a scenario is visually passed only after all of its screenshots pass.
Cancellation resolves queued/running jobs explicitly. The worker writes events
to a private temporary file so an overflowing log queue cannot drop status updates.

Activity is the default run view. It shows overall progress, active/pass/fail
counts, expandable suites, and each scenario/viewport's status, elapsed time,
and failure details. Failures only narrows the activity list. Captured is distinct
from Passed, which requires comparison. Suites, Options, and Log are separate
tabs; live running/finished counts remain visible in the workspace navigator.

## Reviewing Results

Results refreshes on opening and after commands finish using current output
options. It loads each suite's `bitmaps_test/<run>/report.json`, newest first,
retaining a selected report/capture on refresh when it still exists. Filter
All/Failed/Passed and search labels or errors. Capture lists use 50-item pages.

Pass, mismatch, engine/capture error, and unknown status are distinct. Unknown
statuses never count as passes. Malformed reports warn without hiding valid
reports. Discovery loads at most 1,000 reports, limited to 16 MiB each.

Reference, Current, Diff, and Side by side display background-loaded PNGs.
Fit width, zoom, and scrollbars support tall pages. Image dimensions are bounded
with the CLI's PNG helper before decoding. Missing images are recoverable errors.
Paths resolve relative to `html_report` and must remain inside the output root.

External-open controls launch the HTML summary, artifact folder, or image in its
default application. Report HTML is not embedded in GPUI. Historical reports
refer to current baseline files; later approval can change their reference image.

## Shortcuts

Use Cmd instead of Ctrl on macOS. Native inputs retain their editing shortcuts;
toolbar Undo/Redo always use complete document history.

| Shortcut | Command |
| --- | --- |
| Ctrl+O | Open workspace |
| Ctrl+N | New suite |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save As |
| Ctrl+R | Reload with draft guard |
| Ctrl+Z / Ctrl+Shift+Z | Undo / redo |
| Ctrl+F in Source or Log | Find |
| F2 | Rename suite |
| F5 | Refresh files |

## Verification

```sh
cargo fmt --check
cargo test --all-targets
cargo test --features desktop --lib desktop::
cargo test --features gui-tests --all-targets -- --test-threads=1
cargo clippy --features gui --lib -- -D warnings
cargo build --features gui --bins
```

[Desktop Checks](../.github/workflows/desktop-checks.yml) defines Windows CLI,
native interaction, build, and lint gates plus Linux headless model tests.
GPUI tests use production views and real clicks/typing; they verify events,
state, and layout bounds, not full OS IME behavior or pixels.

### Native Windows Smoke

In PowerShell 7 on an interactive Windows desktop:

```powershell
./ci/desktop-smoke.ps1
```

The script builds both binaries, creates an isolated synthetic workspace under
`target/desktop-smoke`, navigates fields/source/results with Windows UI Automation,
and runs the actual CLI init command from the GUI. It captures the target window
directly, checks nonblank pixels, and leaves six screenshots for review. It runs
no website scenarios or browser downloads. `-SkipBuild` uses existing debug
binaries; `-KeepOpen` leaves the fixture open for inspection.

Windows native validation covers 1600x1000 and 1080x720. Full IME composition,
screen-reader usability, and native macOS/Linux behavior still require testing
on those environments. The Linux CI model job is not a claim of native GUI
validation there. Signed installers, cloud accounts, collaborative editing,
browser recording, and non-Chromium execution are outside this implementation.
