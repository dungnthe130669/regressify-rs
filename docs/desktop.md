# Regressify Desktop

The optional GPUI Kit desktop app shares the CLI's Rust library and keeps
YAML/JSON files as the source of truth. It provides configuration authoring,
isolated bundled-engine execution, and visual report review. See
[the implementation plan](desktop-plan.md) for scope and verification evidence.

## Build and Launch

The Windows GUI binary reserves an 8 MiB main-thread stack for GPUI rendering,
including debug builds. This linker setting does not apply to the CLI binary.

Use stable Rust and native build tools. Windows requires Visual Studio C++ build
tools and the Windows SDK. Other GPUI platform prerequisites are documented at
<https://gpui-kit.com/docs/installation>.

```sh
cargo build --features gui --bin regressify-desktop
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

## Appearance and Commands

The header appearance menu offers System, Light, and Dark. System follows the
operating system and responds to appearance changes while the app is open;
explicit Light or Dark overrides those changes. New installations default to
System. Existing `darkMode` preferences retain their previous light/dark choice.
The File-menu toggle remains available and switches from the currently displayed
theme. Native window decorations may still follow the OS on some platforms.

Both palettes use consistent menus, tabs, lists, input outlines, and status
colors, with visible keyboard-focus rings. Theme changes preserve open drafts,
selection, run state, and comparison images. Automated checks cover 4.5:1
contrast for the principal text/status color pairs, not full accessibility
certification.

Source wrapping, line numbers, and text size are saved across restarts. Text size
is bounded to 10-24 pixels; the reset control returns it to 14. These changes do
not edit the document. Preference writes are serialized and coalesced without
blocking editing; normal close waits for the final write. A settings-write error
is reported without locking the editor or discarding a draft.

Open Commands from the header or Ctrl+Shift+P (Cmd+Shift+P on macOS). Search by
command name or category, use arrow keys to select, Enter to execute, and Escape
to cancel. File, edit, view, navigation, and theme actions reuse the normal
workbench commands and their save/confirmation guards. Unavailable actions remain
disabled rather than bypassing the current workspace or document conditions.

## Release Pipeline

The [Release workflow](../.github/workflows/tuyen-release.yml) has a `build`
choice: `cli`, `desktop`, or `both` (default). The existing platform switches
select targets; selecting none falls back to Linux GNU x86_64.

Every application/target pair is an independent parallel job. CLI and desktop
jobs have separate caches and artifact names, and one failed job does not cancel
the others. Desktop builds enable `gui`, which includes the bundled local engine;
CLI feature flags remain unchanged. Desktop jobs have a 90-minute timeout.

Desktop targets are Linux GNU x86_64 (Ubuntu 24.04 build host), macOS x86_64 and
ARM64 (native macOS 15 runners), and Windows x86_64. Musl targets remain CLI-only;
selecting only Musl with `desktop` fails early with an actionable error. With
`both`, Musl jobs build the CLI and desktop jobs run for selected supported targets.

Packages are `regressify-cli-<target>` or `regressify-desktop-<target>` in both
ZIP and 7z formats, with SHA-256 sidecars. The existing release-branch checkout,
destination repository, and `linux`/`macos`/`windows` release tags are retained.

Release tests run before generating the matrix. To run them locally, install
`PyYAML==6.0.3` for Python and run `python ci/test-release.py`. Bash is also required;
set `BASH_PATH` to Git Bash on Windows. The tests execute all 192 application/target
selections and stub Cargo to check arguments without compiling or publishing.

## Workspace and Files

Displayed filesystem paths use forward slashes and omit Windows extended-length
prefixes: `\\?\D:\project` appears as `D:/project`, while a network share remains
`//server/share`. This is display formatting only; filesystem access, safety
checks, and saved paths keep their original semantics.

The navigator has separate, vertically resizable Test suites and Scenarios panes.
Suite rows keep aligned checkboxes and remain highlighted while their scenario
or action is edited. The lower pane lists only the open suite's scenarios, with
labels and URLs; actions are edited in the scenario's action sequence rather
than nested in the sidebar. Both panes scroll independently.

Suite search matches paths and scenario labels, IDs, or URLs across the workspace,
but displays matching suites only. Open a suite to see its scenarios below. The
lower search filters only that suite, without changing the suite search or run
selection. Selected narrows the upper list to checked suites. Ctrl+P (Cmd+P on
macOS) focuses suite search. Navigation retains Save/Discard/Cancel protection.
Suite rows also show queued/running/pass/fail totals. The refresh icon reloads
suite discovery. Scenario rows shorten web addresses to host/path and local
fixture URLs to workspace-relative paths; tooltips retain the full URL.

The Workspace files section contains configuration and YAML/JSON under `common`.
Opening a viewport file displays its definitions in the lower pane. Open
configuration file adds other workspace-relative data files, including custom
viewport definitions.

File supports new/duplicate/rename/delete suite, Save As, reload, configuration
and viewport-file creation, file browsing, refresh, and light/dark appearance.
Save As takes a workspace-relative destination and never replaces an existing
file. Reload discards a draft after confirmation. Suite-file deletion is
confirmed but cannot be undone through document history.

Opening another document/workspace or closing with a draft prompts Save,
Discard, or Cancel. A failed save retains the prompt and draft. Recent workspaces
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

The navigator can be hidden for a wider editor or comparison preview; its
visibility is saved with appearance preferences. Editor, Runs, and Results keep
their state while switching views. Document and parent-scenario breadcrumbs
navigate without changing source or bypassing draft validation.

Find a setting searches names, keys, descriptions, and groups, including advanced
settings. Overrides only narrows the form to explicit values. Groups can be
collapsed independently or expanded together without changing the draft. Numeric
steppers start from the effective inherited value; mismatch percentages remain
within 0-100 and concurrency counts remain positive. File pickers sit beside
their input. Field-issue links reveal the affected control even under filters.

Viewport and dependency lists retain free-form input and also offer checked
choice menus. Viewports load asynchronously from the effective viewport file,
including custom names and dimensions. Inherited or empty viewport lists mean
all viewports, as in the CLI; the picker keeps at least one selected. Unknown
existing values remain editable instead of being silently removed. Dependencies
use IDs from the current unsaved suite and exclude the current scenario and
ambiguous duplicate IDs. Missing or invalid explicit viewport files display a
recoverable warning beside the field.

Scenario fields include an action sequence with step icons and target summaries.
Open a step directly, move it earlier/later, duplicate it, or remove it with
confirmation. Reorder and duplicate keep the scenario selected and participate
in document undo. The Add action menu groups navigation, interactions, page
utilities, and capture/state actions. Search settings for `actions` to isolate
the sequence. Input values are not included in the sequence summaries.

The workspace opens to an overview with suite/scenario totals, selection counts,
configuration issues, and per-suite status. Edit opens the selected suite; Overview
returns to the workspace without discarding an open draft. The header identifies
the single active workspace and links to live execution while a run is active.

Fields provides typed controls derived from the Rust schemas. Advanced reveals
optional root settings. Source exposes complete YAML/JSON with highlighting,
selection, native text editing, and search. Effective uses the real CLI resolver.

Related fields are grouped into General, Capture, Session and dependencies,
Execution, Page setup, and Behavior. Short values use a two-column layout; URLs
and lists use the full width. Document actions stay with the editor. Light and
dark appearances share restrained teal accents and consistent native typography.

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

Source has local YAML/JSON schema completion. Suggestions appear while typing
property names and supported boolean/browser values; Ctrl+Space opens the menu
explicitly (also Ctrl+Space on macOS). Arrow keys choose an item, Enter accepts,
and Escape dismisses. Keys are scoped to workspace, suite, scenario, action, or
viewport context, and existing sibling keys are excluded. Accepting a suggestion
replaces the partial token without reformatting surrounding source.

Completions use the Rust field schema and an error-tolerant syntax tree; no
network service or language-server installation is required. Arbitrary data
files, free-text values, JavaScript, and live-page CSS selectors are not completed.
JSON files and the Effective view use JSON highlighting; YAML files use YAML.

The source toolbar provides Find, Replace, and Suggest, plus word wrapping and
text sizes from 10 to 24 pixels. These presentation changes preserve source and
undo history. The status strip shows the language and cursor position. Syntax
errors underline their source line; the issue button unfolds and focuses that
line. Schema errors without a source location remain in the diagnostic panel.

- Guided YAML edits retain existing comments and formatting using a concrete syntax tree. New nested values use valid YAML flow notation.
- Generated edits are parsed and checked against the requested semantic change. Unsupported constructs leave the draft untouched; use Source instead. Multiple YAML documents are rejected.
- JSON edits are pretty-printed. Save As format conversion preserves data, not YAML comments or formatting.
- Invalid drafts cannot save. Invalid field text blocks structural commands until corrected, reset, or undone. Problem links reveal and focus invalid controls; resetting one field preserves other drafts and errors.
- Saves detect external conflicts and use same-directory atomic replacement. Save As never replaces existing files. Paths reject workspace and symlink escapes.
- Immediate saves, breadcrumbs, and structural commands read retained native input, including text whose change event has not run. Save completion acknowledges only the source written; newer edits stay dirty. This is conflict detection, not an OS-wide lock.
- Documents are limited to 4 MiB. Undo retains at most 100 exact-source snapshots with a 16 MiB budget.

Valid plain YAML strings such as `.item[aria-label='10 midpoints']` retain their
original spelling during guided edits. Adding suite fields does not require
quoting existing selectors. A YAML 1.2 span adapter protects these strings from
the lossless editor's flow-delimiter parsing, then restores them without changing
comments, Unicode text, or surrounding line endings.

Use trusted local workspaces. Configuration can reference scripts and browser
actions executed by the engine. The desktop is not a sandbox for untrusted projects
or arbitrary executables.

## Running Commands

Runs provides direct Run Test, Run Reference, Approve Results, and Compare
Results buttons. There is no command-mode selector followed by a generic Run.
More actions provides Build Report, Optimize Images, Install Chromium, and
Initialize Workspace. Suite checkboxes in the navigator and Runs support multiple
selections and Select all. No selection disables suite commands, never an
implicit all-suite run. Invalid suites remain visible for
repair and are excluded from Select all. Editor Run Test and Run Reference act
on the open suite immediately, with Approve/Compare in More suite actions.
Overview actions operate on checked suites. Command search also exposes direct
open-suite actions. All routes start execution after any draft and artifact confirmations.
Reference capture, approve, and optimize require action-named confirmations
describing the baseline or image changes. Pending option text is read before
command changes and execution; invalid option drafts are retained for correction.

Run settings is independent of the last command. Search the Environment & output,
Capture, Comparison, Report, Image optimization, and Diagnostics & network groups
before running any action. Browser download has Auto/Always/Never choices;
Compare after test controls whether a test capture is immediately compared.
Invalid settings block only commands that use them. Other drafts stay visible
and retained, and unrelated options are omitted from the execution request.

Each request freezes its command, options, and logical suite selection. The
run-specific draft prompt offers Save and continue or Run saved version, plus
Cancel. Saving cannot silently switch the requested command. Preflight resolves
those exact suites again, including any newly saved output IDs; it never substitutes
another same-sized selection or expands an empty selection to all suites.

The CLI has no single-scenario run flag. Selecting a scenario for editing does
not restrict execution to it. Suite selections use exact escaped names;
compare and approve resolve each selected suite's explicit output ID.
The selected suite catalog is refreshed immediately before execution. A removed
or newly invalid selection stops the run for review, while updated output IDs
are resolved from disk. Workspace-only commands do not require valid suites.
For duplicate names, `.tests.yaml` takes precedence over `.tests.yml`, then
`.tests.json`. Open-suite actions reject a shadowed file and name the competing path.
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

Activity headlines distinguish failed/passed captures, captures not compared,
stopped runs, and command failures. View Results opens the executed output folder
and exact run, never an older report when the requested run is missing. Compare
This Run compares the exact captures after skipped comparison or after approval.
Select suites and View Log provide direct routes from ready and completed states.

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

Approve This Run promotes successful captures from the report being viewed,
including historical runs, after an action-named confirmation. Its output folder,
run ID, and suite output identity are pinned; if the suite output has changed,
approval stops for review. Failed capture placeholders are still skipped by the
engine. This is a run-wide approval, not approval of only the visible filter.

Capture rows use scenario labels, with viewport and saved-step/target details on
the secondary line. Newly captured labels are preserved in report metadata;
older runs resolve names from the current workspace where an ID matches
unambiguously. Unresolved generated IDs fall back to the recorded page URL or an
unnamed-scenario label. Raw capture IDs remain searchable and available in
tooltips. Generated desktop run UUIDs are presented as report timestamps;
timestamp-based CLI run IDs are formatted as local dates. Custom run names remain
unchanged, and the original run ID is available in the report-picker tooltip.

The comparison heading shows capture context and the allowed mismatch threshold
beside the measured difference. Copy capture details includes the human label,
outcome, page, threshold, failure details, and raw capture ID. Edit suite opens
the owning workspace file through the same guarded navigation used by the
sidebar; it is unavailable when that association cannot be resolved safely.

Results refreshes on opening and after commands finish using current output
options. It loads each suite's `bitmaps_test/<run>/report.json`, newest first,
retaining a selected report/capture on refresh when it still exists. Filter
All/Failed/Passed and search labels or errors. The preview follows the filtered
selection and clears when nothing matches. Capture lists use 50-item pages.
Previous/next controls traverse matching captures; double arrows jump directly
to failures. Navigation automatically reveals the selected capture's page.
Expand comparison hides the capture list without resetting selection, filters,
images, or zoom. Combine it with the navigator toggle for a full-width preview.

Pass, mismatch, engine/capture error, and unknown status are distinct. Unknown
statuses never count as passes. Malformed reports warn without hiding valid
reports. Discovery loads at most 1,000 reports, limited to 16 MiB each.

Reference, Current, Diff, and Side by side display background-loaded PNGs.
Fit width, zoom, actual-size reset (100%), and scrollbars support tall pages. Image dimensions are bounded
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
| Ctrl+P | Search suites and scenarios; reopen a hidden navigator |
| Ctrl+B | Toggle navigator |
| Ctrl+1 / Ctrl+2 / Ctrl+3 | Editor / Runs / Results |
| Ctrl+Shift+F | Find a setting |
| Ctrl+N | New suite |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save As |
| Ctrl+R | Reload with draft guard |
| Ctrl+Z / Ctrl+Shift+Z | Undo / redo |
| Ctrl+F in Source or Log | Find |
| Ctrl+Space in Source (all platforms) | Show schema suggestions |
| F2 | Rename suite |
| F5 | Refresh files |

## Verification

```sh
cargo fmt --check
cargo test --all-targets
cargo test --features desktop --lib desktop::
cargo test --features gui-tests --all-targets -- --test-threads=1
cargo clippy --features gui --lib -- -D warnings
cargo build --features gui --bin regressify-desktop
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

Add `-Capture` to run three local fixture scenarios across two suites with an
installed Chromium browser and assert their live pass counts. This uses local
HTML only and does not contact an external test website or download a browser.

The script builds the standalone desktop executable, creates an isolated synthetic workspace under
`target/desktop-smoke`, navigates fields/source/results with Windows UI Automation,
and runs the bundled engine's init command from the GUI. It captures the target window
directly, checks nonblank pixels, and leaves screenshots for review. By default it runs
no browser scenarios or downloads. `-SkipBuild` uses the existing debug
binaries; `-KeepOpen` leaves the fixture open for inspection.

The native workflow also exercises navigator visibility, expanded comparison,
compact side-by-side previews, actual-size reset, action menus, checked viewport
and dependency choices, and real source-completion acceptance and saving. Input
steps verify foreground focus and stable control bounds; literal fixture text
uses Unicode input packets without changing the user's keyboard settings.
Close any existing desktop window before starting the smoke check; the
single-instance guard still applies.

Unless `-KeepOpen` is set, cleanup automatically discards unsaved changes in the
smoke runner's own synthetic fixture. The test deliberately closes a dirty draft
and verifies normal exit and an unchanged saved file. Normal application windows
continue to show Save/Discard/Cancel for unsaved work.

The native checks also verify selected-suite-only scenario lists, readable names
for generated capture IDs, persisted scenario-label metadata, forward-slash
workspace paths, and live suite totals. Pointer input uses physical screen
coordinates to avoid DPI virtualization errors.

The native theme checks switch Light/Dark/System, exercise the command palette,
capture source and result views in both palettes, and restart the app to verify
saved appearance and editor display settings. They back up and restore the
original preferences file, including recent workspaces. With `-KeepOpen`, the
remaining fixture window retains its test appearance until closed; the original
file is restored when the script returns.

Windows native validation covers 1600x1000 and 1080x720 with 28 window captures.
Full IME composition, screen-reader usability, and native macOS/Linux behavior still require testing
on those environments. The Linux CI model job is not a claim of native GUI
validation there. Signed installers, cloud accounts, collaborative editing,
browser recording, and non-Chromium execution are outside this implementation.
GPUI 0.6.1 has a remaining managed-tooltip issue after some programmatic view
changes; this does not affect document or result data.
