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
to isolate run settings and browser processes. Windows installation and in-app
updates are described below; installer signing is not configured.

## Windows Installation and Updates

Windows x86_64 releases include `Regressify-Desktop-Setup-N.N.N.N-x64.exe`.
Run the installer normally, without administrator elevation. It installs per user
in `%LOCALAPPDATA%\Programs\Regressify Desktop`, creates a Start-menu shortcut,
and registers an uninstaller. Workspaces and desktop preferences are preserved
by updates and uninstall. The release executable links the MSVC runtime statically.

Official Windows builds check for updates once at startup. The header's Software
updates button shows the installed version and supports manual checks. Select
Download update, then Install and restart. Downloads must match the manifest's
size and SHA-256, and are checked again before execution. Only newer numeric
versions and this app's immutable HTTPS installer URLs are accepted. A missing
manifest (404) means no release is published; network failures remain visible in
the dialog and do not interrupt editing.

Installation requires active runs to finish and uses the usual Save/Discard/Cancel
guard for a draft. The installer never forcibly closes other windows or workers;
close them normally when prompted. New app launches are blocked during setup.
After installation the initiating workspace reopens. Older installers cannot
downgrade an installed version. Unconfigured development builds and non-Windows
builds do not contact the update service.

The public feed is `https://drive.tuyen.blog/downloads/<app-slug>/latest.txt`.
Only the public app slug is embedded in the application, never the upload token.
Installers are currently unsigned, so Windows may show SmartScreen warnings.

## Git

The title bar shows the number of changed files and incoming/outgoing commits
for the workspace's repository. Click it, use Ctrl+Shift+G (Cmd+Shift+G on macOS),
or choose Git from Commands to open the change tree and read-only diff review.
The repository root is shown explicitly, including when the workspace is a
subdirectory of a larger repository. Counts and the tree cover that repository.

Local status refreshes every five seconds. The app checks the tracking remote
(otherwise origin or the only configured remote) every minute while idle.
Automatic checks fetch remote refs only: they never pull, merge, rebase, stash,
or change working files. Fetch also provides a manual remote check. Incoming
commits, divergence, merge conflicts, and failed remote checks are highlighted;
the last successful check remains visible so cached counts are not mistaken
for a successful fresh check. Network commands have deadlines and terminal
credential prompts are disabled. Configure Git credentials/SSH trust outside
the app; authentication failures can be handled in VS Code.

Changed files appear as a collapsible, virtualized folder tree. Select a file to
review its staged and unstaged changes; binary files are identified by Git.
Choose files with the checkboxes, enter a commit message, and confirm the commit.
The current contents of selected files are committed, including selected
renames/deletions and untracked files. Unselected staged files remain staged and
are not included. A failed commit may leave the selected files staged, as in
normal Git; no changes are discarded. Git hooks and signing configuration remain
in effect. Submodule commits and in-progress merge/rebase commits are delegated
to VS Code. Filenames are passed literally using NUL-delimited pathspecs.

Untracked workspaces display Not tracked and offer Initialize Git; initialization
does not stage files. Add origin accepts a credential-free HTTPS/SSH URL or local
repository path. Push uses the configured tracking branch, or publishes the
current branch to origin/the sole remote and establishes upstream tracking. It
never force-pushes. Pull requires a clean working tree and uses fast-forward-only
semantics. Diverged branches and existing conflicts show Open in VS Code for
merge/rebase resolution; refresh afterward. This avoids embedding a second merge
editor or silently resolving conflicts. VS Code must be installed and registered
for its `vscode://` URL handler.

Commit, pull, push, initialization, and remote setup use explicit confirmation
and the editor's unsaved-draft guard. Mutating operations block workspace edits
until completion and offer Stop; other Git work runs off the UI thread. Requests
revalidate repository identity and HEAD before acting. Cancelling a network
operation cannot undo changes already accepted by the remote. Git must be
installed on PATH. Git UI tests use isolated repositories and local bare remotes.

Remote/upstream configuration is fingerprinted without retaining credentials in
status messages; network actions reject a changed destination after confirmation.
Changing branches or remotes invalidates the previous remote-check timestamp and
any in-flight result. Pull reloads the editor from disk in the background and
closes a document removed by the incoming commit while retaining the Git view.

`./ci/desktop-smoke.ps1 -GitFlows` runs the native acceptance flow against local
repositories only. It verifies initialization, tree and diff review, selected
commits that preserve unrelated staging, push, automatic one-minute incoming
checks, fast-forward pull, and conflict handoff in light/dark compact windows.
The Git workspace is isolated under the OS temporary directory; screenshots
remain under `target/desktop-smoke`. Other native fixtures initialize an isolated
repository before opening, so periodic checks cannot use the parent project's
remote. No real project commits or remote pushes are made by these tests.

## Deploy Reports

The header's Deploy command publishes selected suites to Azure Static Web Apps.
Use Deployment targets to add, edit, or remove multiple targets for the current
workspace. Choose the target in the deployment dialog each time. A target has a
name, a deployment-token environment variable, and an optional Azure named
environment (blank means production). Define the environment variable before
launching the desktop app. Token values are never saved in project files or
target settings, passed on the command line, or included in deployment logs.
Target settings are stored in the per-user application configuration, keyed by
the canonical workspace path; removing a target does not delete the Azure app.

At least one suite must be selected. When only a subset is selected, the dialog
lists it and requires explicit confirmation that excluding the other suites is
intentional. Individual scenario checkboxes do not narrow deployment: the full
latest report of each selected suite is published. Unsaved changes use the normal
Save/Cancel guard, and a changed suite catalog requires selection review again.

Deploy validates the token, runs the bundled `snapshot` command for the selected
suites, stages only their reports and referenced images, and then publishes.
Unselected suites, old unreferenced runs, persisted authentication state, source
YAML, and unrelated files are not uploaded. Symlinked or escaping artifact paths
are rejected. No external Azure, npm, or Tuyen CLI installation is required.
The protocol follows Tuyen CLI's SWA deploy handshake: validate token, request
upload URLs, upload Azure blob blocks, update status, and poll until completion.
Transient network failures retry within bounds; the entire deployment has a
30-minute deadline. ZIP packaging is disk-backed, preserves already-compressed
images without recompressing them, and uploads at most four 4 MiB blocks at once.
Missing current/diff images fail deployment rather than publishing broken links;
missing references remain valid report failures. Non-Standard plans reject security-sensitive `auth` and
`networking` configuration rather than letting Azure silently discard it.

The last selected target is remembered per workspace, and removing a target
requires confirmation. Deploy and target management are also available from the
command palette. Progress, the deployed site URL, and Open Site appear in Runs.
Stop cancels the local worker
process tree; it cannot undo a deployment already accepted by Azure. Deployment
transport is covered by local mock-server tests; a live Azure smoke deployment
requires a configured target and is not performed by the test suite.

Native Windows deployment/layout verification uses
`./ci/desktop-smoke.ps1 -SkipBuild -DeploymentFlows` after building the GUI. It
creates isolated target settings, exercises target persistence and subset
confirmation, captures compact light/dark layouts, and verifies missing-token
failure without contacting Azure. It refuses a locked session before changing
preferences and cleans up only its own fixture settings and processes.

## Appearance and Commands

Workspace navigation and global tools share a compact custom title bar with
native window controls. Suite actions sit beside the editor tools; the navigator,
settings forms, and activity metrics use compact spacing. The main source text
size remains independently configurable. Headless layout tests cover 1080x720
and 1600x1000 windows and check that controls do not overlap window buttons.

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

Untagged/manual archive builds produce `regressify-cli-<target>` or
`regressify-desktop-<target>` in ZIP and 7z formats with SHA-256 sidecars. Their
destination repository and `linux`/`macos`/`windows` release tags are unchanged.
Every build now checks out the triggering ref, not a hard-coded release branch.

Push a version tag to publish the Windows desktop installer:

```sh
git tag v0.2.1
git push origin v0.2.1
```

Tag pushes build Windows desktop only. Tags must be `vN.N.N` or `vN.N.N.N`, with
each numeric component between 0 and 9999. The tag supplies the desktop version,
Windows executable metadata, installer name/version and Tuyen Drive version;
three components normalize to four (for example `v0.2.1` becomes `0.2.1.0`).
The Cargo/CLI package version is not changed. A manually dispatched workflow can
also publish when run on a valid version tag with Windows desktop selected.
Branch builds cannot publish to Tuyen Drive.

Configure the repository secrets `TUYEN_DRIVE_APP` (app slug) and
`TUYEN_DRIVE_TOKEN` (that app's upload token). Only the publish step receives the
token. Tagged Windows jobs do not require the legacy GitHub archive-upload
credentials. The version must be higher than the latest published version.
Releases are serialized without cancelling an active publish. CI installs Tuyen
CLI with `tuyen-at-work/tuyen-cli-action@master` and checks that
`tuyen drive upload-app` is available before building. The publish step passes the
app, token, tagged version, installer file, and `<filename>=installer` role to
that command. Tuyen CLI owns checksums, upload-size selection, and retries; there
is no custom Python uploader. The setup action must provide a CLI release that
includes `drive upload-app`.

After a successful CLI upload, a separate credential-free check requires the
public manifest to match the exact version, immutable URL, size, and SHA-256 of
the local installer. It retries transient errors and propagation delays within
a bound, and uses an explicit client User-Agent rather than Python's default.
Tuyen Drive generates the manifest; it is never uploaded separately. Published
versions cannot be overwritten, including with a different rebuild of the same
tag. CI retains the installer as an Actions artifact before publishing.

The installer is compiled with Inno Setup 6. Release CI verifies static-runtime
dependencies and exercises installation, reinstallation, active-process blocking,
downgrade rejection and uninstall before upload. Local packaging of a desktop
build whose executable version matches `0.2.0.0`:

```powershell
./ci/build-desktop-installer.ps1 -Version 0.2.0.0 -BinaryDirectory target/debug -Compiler 'C:/Tools/Inno Setup 6/ISCC.exe' -SmokeTest
./ci/desktop-smoke.ps1 -SkipBuild -UpdateFlows
```

The installer smoke test refuses to run over an existing per-user installation;
it installs only under a generated `target/installer-smoke-*` directory, suppresses
shortcuts/relaunch, and removes its temporary installation registration. The
native UI smoke requires an unconfigured development build and temporarily backs
up/restores preferences, like the other desktop smoke modes.

Release tests run before generating the matrix. To run them locally, install
`PyYAML==6.0.3` for Python 3.11+ and run `python ci/test-release.py`. Bash and
PowerShell 7 (`pwsh`) are also required; set `BASH_PATH` to Git Bash on Windows.
The tests execute all 192 application/target selections, stub Cargo and Tuyen CLI
to check the real workflow arguments and failure handling, and verify manifests
against offline fixtures without publishing.

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

In Results, the lower pane becomes Captures for the active suite. Selecting a
suite there changes the report being reviewed without opening another editor
document or disturbing an unsaved draft.

Scenario checkboxes select execution scope independently of the scenario being
edited. A suite checkbox shows a mixed state for partial selection; clicking it
selects the whole suite, then clears it on the next click. The sidebar's play
button runs only that row's scenario. Runs > Suites also expands each suite into
scenario checkboxes with direct Test and Reference buttons. Clearing the last
scenario disables suite commands rather than reverting to an all-scenario run.

Overflowing areas show persistent, draggable scrollbars in both themes, including
navigation, settings, action sequences, activity, source/log editors, and result
lists. Image previews have horizontal and vertical scrollbars when the selected
zoom exceeds the available space. Side-by-side previews share both scroll offsets;
scrolling or dragging a thumb in either pane moves both images together.

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

Multiple desktop instances can run at once, each with its own workspace. Empty
windows are also independent. Opening a workspace already owned by another
instance restores and focuses that existing window instead of opening a duplicate.
This applies to startup paths, the folder chooser, and recent workspaces. A
duplicate startup exits successfully after handoff; an existing requesting window
keeps its current workspace, draft, selection, and run. Reopening the current
workspace also preserves its draft rather than reloading it.

Each window still has one active workspace. Close workspace unloads documents,
run options, selections, logs, and previews and releases its ownership without
closing the window. Switching to a different unowned workspace retains the normal
save/discard guard and requires active runs to finish or be cancelled. The old
workspace remains owned until the new workspace loads successfully.

Ownership uses the canonical workspace path, resolving relative and symlink
aliases; Windows paths are also case-normalized. Per-workspace file locks and a
token-checked, loopback-only activation endpoint live in the user's configuration
directory, not the project. Closing or crashing releases the OS lock, and the next
owner replaces stale activation data. An unresponsive owner is reported rather
than bypassing the lock. Bundled engine workers do not acquire GUI ownership.
Use the updated executable for all windows; an older globally locked version
cannot receive the new activation requests.

Preferences remain per-user. Concurrent writes are serialized and recent-workspace
entries are merged so separate windows retain each other's history. Appearance
and editor defaults use the last saved settings; existing windows keep their
current appearance until changed or reopened.

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

Switching Fields, Source, and Effective reads pending input before changing the
view. Invalid source stays in Source when Fields cannot represent it, with its
diagnostic retained. File-menu actions use the same availability rules as command
search, including disabling suite deletion for unsaved documents.

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
repair and are excluded from Select all. Editor Run Suite Test and Run Suite
Reference act on the whole open suite, with Approve/Compare in More suite actions.
The scenario editor separately provides Run Scenario Test and Run Scenario
Reference, also available while editing one of its actions. Command search
exposes scenario-scoped Test, Reference, Compare, and Approve actions.
Overview actions operate on checked suites. Command search also exposes direct
open-suite actions. All routes start execution after any draft and artifact confirmations.
Reference capture, approve, and optimize require action-named confirmations
describing the baseline or image changes. Pending option text is read before
command changes and execution; invalid option drafts are retained for correction.

Opening or selecting a suite automatically discovers its reference images and
saved test runs in the active output folder, using the suite's output ID. The
editor, Overview, and Runs show the detected counts. Discovery runs in the
background and refreshes after commands, workspace refreshes, and output-folder
or environment changes. Refresh saved captures also checks changes made outside
the app. Results from an older workspace or output-folder scan cannot replace
the current inventory.

Run Test is disabled until every selected suite has at least one successful
reference image. The same requirement applies to suite/scenario buttons and
command search; Run Reference remains available for initial setup. Empty folders,
diff images, invalid PNGs, and failed-capture placeholders do not establish a
baseline. References are rechecked immediately before execution, including after
saving a draft. This is a suite-level existence check, not a guarantee that every
scenario, viewport, or saved step has a reference. Missing individual references
remain comparison results. Standalone CLI behavior is unchanged.

When references and test data exist, Open Report is available beside the open
suite, on its Runs/Overview row, and for a compatible checked-suite selection.
An existing report opens directly without starting a worker or changing captures.
If saved screenshots have no usable report, the app compares those screenshots
against the references and opens the result automatically, without launching a
browser or creating a capture run. The latest saved run is chosen by screenshot
modification time, not UUID ordering; an explicit Run name selects that exact
run. Suite output IDs and run IDs are pinned together, so another suite with the
same run ID cannot appear accidentally. Existing reports from multiple suites
can be viewed together even when their run IDs differ. If report generation is
needed for different run IDs, use each suite's Open Report control separately.

Run settings is independent of the last command. Search the Environment & output,
Capture, Comparison, Report, Image optimization, and Diagnostics & network groups
before running any action. Browser download has Auto/Always/Never choices;
Compare after test controls whether a test capture is immediately compared.
Invalid settings block only commands that use them. Other drafts stay visible
and retained, and unrelated options are omitted from the execution request.

Each request freezes its command, options, logical suites, and exact scenario
IDs when scenarios are selected. The
run-specific draft prompt offers Save and continue or Run saved version, plus
Cancel. Saving cannot silently switch the requested command. Preflight resolves
those exact suites again, including any newly saved output IDs; it never substitutes
another same-sized selection or expands an empty selection to all suites.

Selecting a scenario for editing does not change run checkboxes. Use its direct
scenario actions or the checkbox selection to restrict execution. Scenario runs
include all configured viewports and saved steps, and still execute `needs`
setup actions without separately capturing dependency scenarios. Unsaved new
scenarios use the normal save guard; choosing Run saved version cannot run a new
scenario absent from that saved suite. Removed or renamed IDs stop preflight for
review rather than selecting another row. Whole-suite selections use exact
escaped names; compare and approve resolve explicit output IDs and use recorded
scenario metadata, never screenshot filename prefixes.
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

With no explicit Run name, Approve Results and Compare Results reuse the last
executed run when its output folder and suite scope match the current selection.
An explicit Run name takes precedence. Subsequent Run Test and Run Reference
actions still create fresh captures rather than reusing that previous run ID.

Stop terminates the process group, including children. Closing during a run
requests cancellation; close again after it stops to handle unsaved work.
The live log is searchable, read-only, and capped at 2 MiB. Its bounded queue may
omit chunks under heavy output; the omission count is displayed. Completion is
tracked separately. Proxy input is masked and command lines are not echoed;
CLI/script logs can still contain project-provided data.

Click a scenario/viewport row in Activity to open its timestamped run log. The
terminal button beside a scenario in the sidebar or Runs checklist opens that
scenario's combined viewport log. These routes work while running, after
completion, and after cancellation. Logs include navigation, action targets,
waits, captures, browser cleanup, and comparison outcomes; typed `input.value`
contents are omitted. Search with Ctrl+F, select/copy text, or use Copy log. Back
to Activity and All run logs retain direct access to the other views.

Scenario messages are attributed to job IDs before concurrent output interleaves.
Their shared retained history is capped at 2 MiB or 4,096 entries, with 16 KiB
per message and an explicit omission notice. It is independent of the console
queue, so heavy console output cannot mix or discard scenario telemetry. The
current run's history remains available until another run starts or the workspace
closes; historical reports do not contain these session logs. Custom scripts,
URLs, selectors, and error messages may still contain project-provided data.

**Command completed means exit zero, not visual success.** Scenario failures,
missing references, and mismatches are report outcomes. Configuration,
infrastructure, and artifact-write errors are command failures.

Activity headlines distinguish failed/passed captures, captures not compared,
stopped runs, and command failures. The Running or Last run label identifies the
executed command even while another command awaits confirmation. Stopped runs
use amber status indicators and offer View Log, not a link to uncompleted results.
After Run Test, Open Report stays available in the Runs header, including while
viewing Log or Run settings. It opens the executed output folder and exact run
with Reference and Current images side by side, never an older report when the
requested run is missing. If Compare after test was off, Open Report compares
that captured run first and opens its report automatically without recapturing
or using the next run's changed settings/selection. Normal draft guards still
apply to that comparison request. Compare This Run remains available after
capture-only execution or approval. Select suites and View Log provide direct
routes from ready and completed states.

Once comparison results are available, the headline and Passed/Failed totals
count **screenshot comparisons**, matching the HTML report. Scenario/job progress
is labeled separately: four scenarios across three viewports can produce twelve
comparison results. A row passes only when all its recorded screenshots pass.
Browser-cleanup or other infrastructure errors remain explicit execution errors
and can make the command fail, but they no longer turn passing image comparisons
into visual failures. Open Report remains available when comparisons were
produced even if the command also reported an infrastructure failure.

The engine emits versioned progress events independently of console logs. Each
scenario/viewport job reports queued, running, capture completion, comparison,
and failure states. Screenshot keys connect visual comparisons to their original
jobs; a scenario is visually passed only after all of its screenshots pass.
Cancellation resolves queued/running jobs explicitly. The worker writes events
to a private temporary file so an overflowing log queue cannot drop status updates.

Activity is the default run view. It shows overall progress, active/pass/fail
counts, expandable suites, and each scenario/viewport's status, elapsed time,
and failure details. Failures only narrows the activity list. Captured is distinct
from Passed, which requires comparison. Suites, Run settings, and Log are separate
tabs; live running/finished counts remain visible in the workspace navigator.

## Reviewing Results

Results uses one left navigator with Test suites above Captures and one detail
area on the right. Only the active suite's captures are shown. A suite without a
report shows an empty capture list, never another suite's results. The report
title is read-only; there is no separate report dropdown.

Approve This Run promotes successful captures from the report being viewed,
including historical runs, after an action-named confirmation. Its output folder,
run ID, and suite output identity are pinned; if the suite output has changed,
approval stops for review. For reports with scenario metadata, approval is pinned
to all scenarios recorded in that report, not just the visible search/filter.
This preserves the scope of scenario-only comparison reports even when their
run folder contains other captures. Older reports without scenario metadata
retain whole-run approval. Failed capture placeholders are still skipped by the
engine.

Capture rows use scenario labels, with viewport and saved-step/target details on
the secondary line. Newly captured labels are preserved in report metadata;
older runs resolve names from the current workspace where an ID matches
unambiguously. Unresolved generated IDs fall back to the recorded page URL or an
unnamed-scenario label. Raw capture IDs remain searchable and available in
tooltips. Generated desktop run UUIDs are presented as report timestamps;
timestamp-based CLI run IDs are formatted as local dates. Custom run names remain
unchanged.

The comparison heading shows capture context and the allowed mismatch threshold
beside the measured difference. Copy capture details includes the human label,
outcome, page, threshold, failure details, and raw capture ID. Edit suite opens
the owning workspace file through the same guarded navigation used by the
sidebar; it is unavailable when that association cannot be resolved safely.

Results refreshes on opening and after commands finish using current output
options. It loads each suite's `bitmaps_test/<run>/report.json`, newest first,
and shows the latest report for the active suite. Open Report pins its requested
runs, including on refresh and while switching between their suites. Selecting a
suite outside that request returns to latest-report browsing. A selected capture is
retained when its report is unchanged. Filter
All/Failed/Passed and search labels or errors. The preview follows the filtered
selection and clears when nothing matches. Capture lists use 50-item pages.
Previous/next controls traverse matching captures; double arrows jump directly
to failures. Navigation automatically reveals the selected capture's page.
The navigator toggle hides both left panes for a full-width preview without
resetting selection, filters, images, or zoom.

Pass, mismatch, engine/capture error, and unknown status are distinct. Unknown
statuses never count as passes. Malformed reports warn without hiding valid
reports. Reports whose suite ID disagrees with their output directory are
rejected. Ordinary browsing reads only the newest valid report per suite;
explicit run requests exclude unrelated reports before applying the discovery
limit of 1,000 report reads, limited to 16 MiB each.

Reference, Current, Diff, and Side by side display background-loaded PNGs.
Fit width, zoom, actual-size reset (100%), and scrollbars support tall pages.
Side-by-side images use a common canvas and scale, including when their dimensions
differ. Both horizontal and vertical scrolling stay synchronized from either pane.
Moving the pointer over an image draws horizontal and vertical guides in both
panes at the same image coordinate. Guides disappear when the pointer leaves the
images, and selecting another capture resets the shared scroll position.
Image dimensions are bounded
with the CLI's PNG helper before decoding. Missing images are recoverable errors.
Paths resolve relative to `html_report` and must remain inside the output root.

External-open controls launch the HTML summary, artifact folder, or image in its
default application. Report HTML is not embedded in GPUI. Historical reports
refer to current baseline files; later approval can change their reference image.

## Shortcuts

Use Cmd instead of Ctrl on macOS. Native inputs retain their editing shortcuts;
toolbar Undo/Redo always use complete document history.

| Shortcut                             | Command                                                |
| ------------------------------------ | ------------------------------------------------------ |
| Ctrl+O                               | Open workspace                                         |
| Ctrl+P                               | Search suites and scenarios; reopen a hidden navigator |
| Ctrl+B                               | Toggle navigator                                       |
| Ctrl+1 / Ctrl+2 / Ctrl+3             | Editor / Runs / Results                                |
| Ctrl+Shift+F                         | Find a setting                                         |
| Ctrl+N                               | New suite                                              |
| Ctrl+S                               | Save                                                   |
| Ctrl+Shift+S                         | Save As                                                |
| Ctrl+R                               | Reload with draft guard                                |
| Ctrl+Z / Ctrl+Shift+Z                | Undo / redo                                            |
| Ctrl+F in Source or Log              | Find                                                   |
| Ctrl+Space in Source (all platforms) | Show schema suggestions                                |
| F2                                   | Rename suite                                           |
| F5                                   | Refresh files                                          |

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

Add `-Capture` to run direct reference capture, stop a test, rerun the test, and
open the completed results for three local fixture scenarios across two suites.
An installed Chromium browser is required; the script asserts live pass counts
and capture metadata. This uses local HTML only and does not contact an external
test website or download a browser.

Use `./ci/desktop-smoke.ps1 -ScenarioFlows` for focused scenario acceptance. It
implies capture, creates suite references, selects and runs only Checkout,
opens live logs from Activity and the sidebar, reviews results, approves and
compares that exact scenario, and checks sibling reference hashes. It captures
11 light/dark windows at 1600x1000 and 1080x720, including overflowing settings,
logs, and fit-width images. This mode does not run the source-authoring sequence.

Use `./ci/desktop-smoke.ps1 -ArtifactFlows` to verify first-open artifact detection
without Chromium or network access. It checks disabled Test actions with no
baseline, multi-suite gating, direct saved-report opening, comparison of existing
capture-only data, exact suite identity when run IDs match, and detection after
restart. Screenshot and reference hashes remain unchanged. It captures seven
light/dark windows at 1600x1000 and 1080x720 and restores user preferences.

Use `./ci/desktop-smoke.ps1 -InstanceFlows` for multi-instance acceptance. It opens
two different workspaces and multiple empty windows, verifies duplicate startup
and recent-workspace handoff, restores a minimized owner, and checks its unsaved
draft and saved-file hash remain unchanged. It also verifies ownership release
after Close workspace and recovery after terminating only its own synthetic
fixture. Seven native window captures are retained; all fixture processes are
tracked for cleanup and the original preference file is restored.

Use `./ci/desktop-smoke.ps1 -TitlebarFlows` for native window input acceptance.
It uses physical pointer events for Commands, view tabs, navigator visibility,
and Deploy, then drags the workspace-title area and verifies the native
maximize/restore, minimize, and close controls. Closing a dirty document must
show the save guard; cancelling preserves the draft and the saved-file hash.
Seven screenshots include the maximized window and close prompt. Title-bar
controls consume their pointer press so Windows cannot mistake them for a
caption drag; only the surrounding title area remains draggable.

The script builds the standalone desktop executable, creates an isolated synthetic workspace under
`target/desktop-smoke`, navigates fields/source/results with Windows UI Automation,
and runs the bundled engine's init command from the GUI. It captures the target window
directly, checks nonblank pixels, and leaves screenshots for review. By default it runs
no browser scenarios or downloads. `-SkipBuild` uses the existing debug
binaries; `-KeepOpen` leaves the fixture open for inspection.

The native workflow also exercises navigator visibility, expanded comparison,
compact side-by-side previews, actual-size reset, action menus, checked viewport
and dependency choices, and real source-completion acceptance and saving. It
approves a viewed historical run despite a newer capture directory, checks the
promoted image's checksum, compares that exact run, and opens Run settings in
both themes. Input steps verify foreground focus and stable control bounds;
literal fixture text uses Unicode input packets without changing the user's
keyboard settings.
Close existing desktop windows before starting the smoke check because the
fixture temporarily changes shared preferences, not because the application
restricts the number of windows.

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

The latest automated gates pass 438 library/UI tests, five executable integration
tests, 72 headless desktop tests, and 293 default CLI tests, plus formatting and
strict library Clippy. The standalone Windows desktop builds successfully.

The multi-instance native flow passes all seven captures and foreground/restore,
draft preservation, independent-window, close/reopen, and crash-recovery checks.
Transport tests cover simultaneous launches, canonical aliases, stale endpoints,
invalid tokens, and oversized requests. The executable integration test verifies
duplicate launchers exit successfully and internal workers bypass GUI ownership.
Window switching and duplicate-open draft guards have production-view coverage;
concurrent preference tests verify both windows' recent workspaces are retained.
Native focus/restore behavior has been checked on Windows, not yet macOS/Linux.

The saved-artifact native flow passes all seven captures, no-baseline action
checks, compare-only generation, unchanged-image assertions, and restart checks.
The screenshots were inspected in both themes and window sizes. GPUI's Windows
accessibility bridge removes disabled buttons' Invoke action but does not expose
`IsEnabled=false`; the native check verifies action availability instead. An
empty Activity view's duplicate Select suites accessibility ID was also fixed.

The preceding focused scenario native flow passed all 11 captures and artifact/log checks.
The screenshots were inspected for visible overflow bars, compact controls,
scenario scope, and log readability; an image Fit width regression found during
inspection was fixed and now has a production-view bounds test.

The report-consistency update adds a real-worker fixture proving that HTML,
saved report JSON, desktop loading, and live comparison totals all agree on nine
passes and three failed captures across twelve viewport screenshots. UI tests
cover report access after execution errors, side-by-side exact-run opening, and
capture-only compare-and-open through the draft guard. The Windows smoke locator
uses Open Report; the saved-artifact native flow above now also verifies the
comparison handoff from the suite editor and after restarting the app.

The earlier appearance pass completed 28 captures and restart/cleanup checks.
The broader authoring smoke was attempted again: its first attempt stopped at a
viewport menu, and its next reached source completion but synthetic input left
an invalid draft without a suggestion. That full sequence still needs an
uninterrupted pass; the focused scenario flow is not a claim that the unrelated
source/restart checks were rerun successfully. All attempts restored original
preferences and closed only their synthetic fixture.

Full IME composition, screen-reader usability, and native macOS/Linux behavior still require testing
on those environments. The Linux CI model job is not a claim of native GUI
validation there. Signed installers, cloud accounts, collaborative editing,
browser recording, and non-Chromium execution are outside this implementation.
GPUI 0.6.1 has a remaining managed-tooltip issue after some programmatic view
changes; this does not affect document or result data.
