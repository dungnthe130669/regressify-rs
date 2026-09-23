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

### Desktop Icon

The icon source is [assets/icon.svg](../assets/icon.svg). Windows GUI builds embed
[assets/icon.ico](../assets/icon.ico) as resource `1`, which GPUI uses for the
window and taskbar. The custom title bar embeds [assets/icon.png](../assets/icon.png)
without tinting its gradient. Shortcuts, Jump Lists, and Explorer context menus
use the executable's icon; the Windows installer uses the same artwork.

The installed executable keeps the `Regressify.Desktop` taskbar identity.
Local and portable copies use a stable identity derived from their executable
path, so Windows does not reuse an installed copy's shortcut or cached icon.
Their window metadata explicitly supplies the running executable's icon,
display name, and relaunch command. Moving a portable copy changes its identity.

The ICO contains transparent 32-bit images at 16, 20, 24, 32, 40, 48, 64, 128,
and 256 pixels; the title-bar PNG is 256 pixels. The SVG viewBox tightly frames
the letter to avoid oversized transparent margins. After editing the SVG,
regenerate both exports with Node.js:

```sh
npm install --prefix target/icon-tools --no-save --package-lock=false --ignore-scripts @resvg/resvg-js@2.6.2 png-to-ico@2.1.8
node ci/generate-icon.mjs
```

Commit the SVG and both exports together. Normal Rust and installer builds use
the checked-in images and do not require Node.js or the export tools. On an
unlocked Windows desktop with no other app instances running, verify the native
window icons and rendered title-bar/taskbar pixels with:

```powershell
./ci/desktop-smoke.ps1 -IconFlows
```

## Windows Installation and Updates

Windows x86_64 releases include `Regressify-Desktop-Setup-N.N.N.N-x64.exe`.
Run the installer normally, without administrator elevation. It installs per user
in `%LOCALAPPDATA%\Programs\Regressify Desktop`, creates desktop and Start-menu
shortcuts, and adds **Open with Regressify Desktop** to folder and folder-background
context menus. On Windows 11, the command is under **Show more options**.
Uninstall removes these shortcuts and context-menu entries. Workspaces and desktop
preferences are preserved by updates and uninstall. The release executable links
the MSVC runtime statically.

Setup also prepares the pinned Chromium build in
`%LOCALAPPDATA%\regressify\chromium` before launching the app. It checks the same
shared cache used by CLI, Reference, and Test and downloads only when that build
is missing. Reinstallation or an app update does not download Chromium again
unless the required browser revision changes or the cache was removed. Installed
Chrome/Edge are not substitutes for this managed build. The browser cache remains
available after app uninstall.

Interactive setup and visible `/SILENT` updates show Chromium preparation progress.
During downloads, the text reports transferred size, percentage when available,
speed, and elapsed time. Connection waits, downloads, and extraction report activity
every five seconds. Fully silent `/VERYSILENT` setup stays hidden; all modes write
detailed messages to the setup log. If browser preparation fails, the app installation
still succeeds; interactive setup reports the problem, and Reference/Test can retry later.
Silent setup records the failure without opening a dialog. Pass `/SKIPCHROMIUM=1`
to defer preparation explicitly. Setup uses the HTTP client's default proxy
configuration, not a workspace's saved Proxy setting; for a workspace-specific
proxy, finish setup and choose Install Chromium from the app after configuring
Run settings. The same network time limits and diagnostics described under
[Chromium installation diagnostics](commands.md#chromium-installation-diagnostics)
apply during setup.

Right-click the app's taskbar icon to open a workspace from **Recent Workspaces**.
The Jump List uses saved workspace history, refreshes as workspaces are opened,
and skips missing folders and duplicates. Windows removals are remembered until
the workspace is explicitly reopened. Windows recent-item privacy settings and
the available Jump List space determine whether these entries are shown.

Official Windows builds check for updates at startup when due, then every eight
hours while the app remains open, and automatically download and verify newer
releases in the background. The check timestamp is shared across instances and
survives restarts, so opening another workspace does not issue a duplicate check.
After sleep, an overdue check runs on the next local poll. No upgrade banner,
highlighted update button, or installation prompt appears until the entire
installer is saved locally and matches the manifest's size and SHA-256. Network
outages and failed preparation do not interrupt editing or announce an upgrade.

Only one instance per user update cache owns checking and downloading at a time.
Other instances poll the shared cache every five seconds without contacting the
feed. Once the owner finishes, each instance verifies the same cached installer
and shows its own notification; it does not download another copy. Notification
normally follows the next local poll plus verification time. Closing the owner
releases the process lock and allows another instance to resume any partial
download. Abnormal process termination also releases ownership automatically.

Once preparation succeeds, a banner stays visible below the title bar in every
view with an Install update button, and the header's Software updates button is
highlighted. Select Install update, then Install and restart. Installation uses
the verified local installer without contacting the update service; its size and
SHA-256 are checked again immediately before execution. Closing the dialog does
not dismiss the ready banner or remove the installer. Only newer numeric versions
and this app's immutable HTTPS installer URLs are accepted. A missing manifest
(404) means no release is published.

Release metadata, partial downloads, and checksum retry counts are retained in
the `updates` directory alongside desktop preferences. Automatic feed checks use
the persisted eight-hour schedule; manual checks contact the feed immediately
when no other instance owns preparation. Cache notifications and checksum retries
within that interval reuse validated metadata without issuing another feed
request. Feed failures fall back to the newest cached release if available.
Discovering a newer release discards older partial and complete cached
installers before preparing the replacement. Unrelated cache files are left alone.
Closing the app or losing
power does not intentionally discard download progress: the next launch resumes
from the saved bytes using HTTP range requests. If the server ignores ranges, the
transfer restarts from byte zero. An interrupted verification rechecks the retained
file, and an already downloaded installer can be verified and offered even while
the update service is unavailable. Cached release URLs are validated again before
use. An outer process lock covers the entire check/preparation cycle, in addition
to the existing lock around cache writes. Idle windows keep their ready banners
visible during automatic polls and continue to check for newer releases.

If SHA-256 verification fails, the invalid payload is discarded and downloaded
again, with at most three retries after the initial attempt (four attempts total).
Retries use exponential backoff: wait 2 hours after the first failure, 4 hours after
the second, and 8 hours after the third. The failure count and next retry deadline
survive restarts; reopening the app does not bypass the remaining wait. Scheduled
retries run automatically while the app stays open, without holding the cache lock
during the wait. If all attempts fail, automatic checks skip that version until a
strictly newer release is published. Network interruptions do not consume checksum
retries.

Manual Check for updates resets the checksum retry count, clears any scheduled
delay or skipped-version marker, and checks immediately when no peer owns the
update cycle. If another instance owns it, the dialog reports that preparation
is underway and observes its result instead of starting duplicate work. This is also available
when an installer is already ready, so a newer release can replace it. Automatic
startup checks and scheduled retries never reset the budget for the same version.

The Software updates dialog supports manual checks and shows preparation status:
connecting, receiving data, SHA-256 verification,
and saving the verified installer to disk. During transfer it shows received and
total MiB, percentage, average speed, and elapsed seconds. After five seconds
without data it also shows the inactivity time. Connecting or receiving no data
for 30 seconds pauses the attempt; each download attempt has a separate 15-minute
limit. HTTP errors, incomplete transfers, and disk-write failures retain the
partial download and defer automatic preparation for eight hours; manual Check
for updates retries immediately. Pause download also retains progress and applies
the same shared delay, so another instance does not immediately undo the pause.
Closing the owner without pausing allows another instance or the next launch to
resume instead. Closing only the dialog lets
preparation continue. Diagnostics are shown only in the dialog, where Copy details
copies the message for troubleshooting. A ready installer still requires an
explicit Install and restart; there is no automatic installation.

Installation requires active runs to finish and uses the usual Save/Discard/Cancel
guard for a draft. No open workspace or document is required. After the initiating
app instance closes and the running-app checks pass, the installer requests
foreground focus for its progress window. Fully silent installs remain hidden.
The installer never
forcibly closes other windows or workers; close them normally when prompted. New
app launches are blocked during setup. After installation the initiating workspace
reopens, if one was open. Older installers cannot
downgrade an installed version. Local preparation removes the network dependency
from installation, but disk errors, security software, or running app instances
can still prevent setup from completing. Unconfigured development builds and non-Windows
builds do not contact the update service.

The public feed is `https://drive.tuyen.blog/downloads/<app-slug>/latest.txt`.
Only the public app slug is embedded in the application, never the upload token.
Installers are currently unsigned, so Windows may show SmartScreen warnings.

## Git

The title bar shows the number of changed files and incoming/outgoing commits
for the workspace's repository. Click it, use Ctrl+Shift+G (Cmd+Shift+G on macOS),
or choose Git from Commands to open the change trees and read-only change review.
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

Changed files appear as collapsible, virtualized folder trees. **Changes** contains
ordinary project files. **Comparison Changes**, beneath it, contains files under
`.regressify` and `.regressify__<environment>` directories, including nested
workspaces. Renames into or out of these directories are comparison changes too.
Comparison Changes starts collapsed and unselected, even for files already staged
in Git. Expanding or reviewing a file does not select it. The ordinary All and
folder checkboxes never select comparison files; select those explicitly in their
own panel to include them in a commit. Collapsing the panel preserves explicit
selections, and the confirmation calls out included comparison files. Git-ignored
results stay ignored; this does not edit `.gitignore` or force-add ignored files.

Selecting YAML or JSON opens **Changes**, a structured Before/After review of the
last commit and the current saved file. It groups settings, scenarios, actions,
and viewport definitions, using readable labels, units, and Added, Removed,
Changed, or Moved indicators. Unchanged items are counted and omitted; changed
groups can be collapsed. Scenario matching prefers explicit IDs, then unique
unchanged content, labels, or URLs. Actions use unique content or action type.
Ambiguous matches remain additions/removals instead of guessed replacements.
Inserting an item does not mark every subsequent item as moved. Values are
explicit configuration, not an inferred comparison of inherited runtime settings;
removed overrides are shown as Not set.

**Source Diff** remains available for exact staged and unstaged patches, including
comments and formatting. Formatting-only edits are identified in Changes. Invalid
YAML/JSON, unsupported/binary files, conflicts, and submodules fall back to Source
Diff with a reason. File sizes, nesting, collection matching, and displayed values
are bounded; oversized reviews use the existing raw-diff fallback. Structured
review does not execute scenarios or predict visual website changes, and does not
include unsaved editor drafts. Existing save/discard/cancel guards still apply
before commits. Neither review mode modifies files or the Git index.

Choose files or folders with the checkboxes. A folder checkbox selects its group's
eligible changed files recursively, including files in collapsed subfolders, and
shows a partial state when only some are selected. Conflicts and submodules are
excluded from selection. Enter a commit message and confirm the commit.
The current contents of selected files are committed, including selected
renames/deletions and untracked files. Unselected staged files remain staged and
are not included. A failed commit may leave the selected files staged, as in
normal Git; no changes are discarded. Git hooks and signing configuration remain
in effect. Submodule commits and in-progress merge/rebase commits are delegated
to VS Code. Filenames are passed literally using NUL-delimited pathspecs.

Discard selected files is available beside Commit selected files and does not
require a commit message. Its confirmation lists the selected files and warns
that discarding cannot be undone. Tracked files lose both staged and unstaged
changes and are restored to the last commit, including selected deletions and
renames. New files, whether staged or untracked, are permanently deleted;
unselected changes and unrelated staged files remain untouched. Folder selection
does not include unchanged or ignored files. Discard refuses conflicts,
in-progress Git operations, submodules, directory replacements, and renames that
would overwrite an unselected file. Changed Git status requires another review.

Untracked workspaces display Not tracked and offer Initialize Git; initialization
does not stage files. Add origin accepts a credential-free HTTPS/SSH URL or local
repository path. Push uses the configured tracking branch, or publishes the
current branch to origin/the sole remote and establishes upstream tracking. It
never force-pushes. Pull requires a clean working tree and uses fast-forward-only
semantics. Diverged branches and existing conflicts show Open in VS Code for
merge/rebase resolution; refresh afterward. This avoids embedding a second merge
editor or silently resolving conflicts. VS Code must be installed and registered
for its `vscode://` URL handler.

Commit, discard, pull, push, initialization, and remote setup use explicit
confirmation and the editor's unsaved-draft guard. Mutating operations block
workspace edits until completion and offer Stop; other Git work runs off the UI
thread. Requests
revalidate repository identity and HEAD before acting. Cancelling a network
operation cannot undo changes already accepted by the remote; stopping discard
cannot undo changes already discarded. Git must be installed on PATH. Git UI
tests use isolated repositories and local bare remotes.

Remote/upstream configuration is fingerprinted without retaining credentials in
status messages; network actions reject a changed destination after confirmation.
Changing branches or remotes invalidates the previous remote-check timestamp and
any in-flight result. Pull and discard reload the editor from disk in the
background and close a document removed by the operation while retaining the
Git view. Discard retains the draft commit message.

`./ci/desktop-smoke.ps1 -GitFlows` runs the native acceptance flow against local
repositories only. It verifies initialization, structured and source review,
comparison-file opt-in, selected
commits that preserve unrelated staging, push, automatic one-minute incoming
checks, fast-forward pull, and conflict handoff in light/dark compact windows.
The Git workspace is isolated under the OS temporary directory; screenshots
remain under `target/desktop-smoke`. Other native fixtures initialize an isolated
repository before opening, so periodic checks cannot use the parent project's
remote. No real project commits or remote pushes are made by these tests.

## Deploy Reports

The header's Deploy command publishes selected suites to Azure Static Web Apps
or Netlify.
Use Deployment targets to add, edit, or remove multiple targets for the current
workspace. Choose the target in the deployment dialog each time. A target has a
name, a provider, and a securely saved token. Azure targets also have an optional
named environment (blank means production); Netlify targets have a Site ID.
Paste the provider's token directly into its masked token field and select
Save target. No environment variable
or app restart is needed. When editing a target, leave that field blank to keep
the saved token, or paste a replacement token to rotate it. Saved tokens are
never displayed in the editor; cancelling or closing clears newly entered text.
Changing providers clears the token draft and requires a new token before saving.
Existing targets without a provider setting continue to use Azure.

For **Azure Static Web Apps**, choose that provider and enter the app's deployment
token. The Azure environment field remains optional.

For **Netlify**, create a dedicated Netlify project/site first, choose Netlify
as the provider, and enter its Site ID and a Netlify personal access token.
Netlify now labels the Site ID **Project ID**, under Project configuration >
General > Project details > Project information. Generate the token under your
Netlify user settings > Applications > Personal access tokens; it must have
access to the project's team, including SSO authorization when required.
The app publishes to the existing site's production deployment, replacing its
contents with the selected reports. It does not create or delete Netlify sites,
change domains, run builds, or require the Netlify CLI.

Tokens are stored in the operating system's secure credential store, outside the
repository: Windows Credential Manager (current user, this machine only), macOS
Keychain, or a Linux Secret Service such as GNOME Keyring or KWallet. Linux needs
an available, unlocked Secret Service. A locked or unavailable store produces an
error, never a plaintext fallback. Credentials are scoped to the canonical
workspace path and target, so moving a workspace or changing computers requires
entering the token again.

Target settings contain only credential references in the per-user application
configuration. Token values are never saved in project files or settings JSON,
passed in worker requests or command-line arguments, or included in deployment
logs. Removing a target also removes its saved token, but does not delete the
hosted site. Targets created with the old environment-variable setting retain
their names and Azure environments; edit each target and enter its token once
to enable deployment.

At least one suite must be selected. When only a subset is selected, the dialog
lists it and requires explicit confirmation that excluding the other suites is
intentional. Individual scenario selections do not narrow deployment: the full
latest report of each selected suite is published. Unsaved changes use the normal
Save/Cancel guard, and a changed suite catalog requires selection review again.

Deploy validates the token, runs the bundled `snapshot` command for the selected
suites, and stages the snapshot entry page, versioned JSON, referenced images,
frontend assets, and hosting policies before publishing. Staging follows the
generated manifest rather than walking capture folders. Snapshot generation reads
the run referenced by each suite's `config.js`, including older selected runs,
without modifying comparison data or deleting capture history.

Snapshot and Deploy use `snapshotTitle`, `snapshotHeading`, and
`snapshotSubheading` from the workspace configuration unless an explicit override
was entered in Runs > Settings > Report. The subheading supports Markdown links.
Untouched report fields inherit workspace values, then the built-in defaults;
deliberately entered empty strings remain explicit overrides.

Saved activity from v0.32.0 and earlier prefilled these overrides with built-in
text. On loading that legacy activity, exact built-in strings become inherited
values while custom text is retained. Newly saved explicit overrides, including
values equal to the built-in defaults, are preserved when reopening the workspace.

Unselected suites, unreferenced captures and diffs from any run (including the
current run), capture metadata, unused TTF fonts, source maps, persisted
authentication state, source YAML, and unrelated files are not uploaded. Skipped
files remain unchanged on disk. Referenced web fonts, icons, scripts, and styles
are retained. Symlinked or escaping artifact paths are rejected. No external
Azure, npm, or Tuyen CLI installation is required.
Azure uses Tuyen CLI's SWA deploy handshake: validate token, request
upload URLs, upload Azure blob blocks, update status, and poll until completion.
Transient network failures retry within bounds; the entire deployment has a
30-minute deadline. ZIP packaging is disk-backed, preserves already-compressed
images without recompressing them, and uploads at most four 4 MiB blocks at once.
Missing current/diff images fail deployment rather than publishing broken links;
missing references remain valid report failures. Non-Standard plans reject security-sensitive `auth` and
`networking` configuration rather than letting Azure silently discard it.

Azure token validation also detects the app's actual Free or Standard tier.
Deployment budgets are 250 MiB for Free and 500 MiB for Standard per environment
(Azure labels these limits in MB), measured from all uncompressed report files,
not the ZIP size. An unrecognized tier is logged and conservatively uses the Free
budget. Reports already within budget are uploaded without image changes.

Oversized Azure reports are optimized only in a separate OS-temporary directory.
Local screenshots, references, report metadata, and files used by future
comparisons are never overwritten. Lossless PNG recompression comes first. If
more savings are needed, perceptual palette reduction tries progressively lower
quality floors for passed comparisons (99 down to 50), then failed Reference/Test
images (99 down to 80). Larger images are tried first at each quality level;
processing stops when the report fits, with a higher-quality search for the last
image when possible. Every attempt starts from the original, and replacements
are kept only if smaller. Dimensions, transparency support, and test results
remain intact; failed diff images and error placeholders stay lossless. Any changed
image bytes receive a new `?v=...` value from `get_asset_version`, and suite JSON
plus the entry manifest receive updated versions as well. Filenames remain stable,
and only the temporary staging copy is rewritten.
The deployment log reports the budget, savings, reduced images, and skipped
images. If the report cannot fit within these safeguards, deployment stops before
upload with sizes and recovery steps. Choose fewer suites or a larger plan;
Azure may also reject a fitting report if other environments exhaust total app
storage. Optimization shares the deployment deadline and does not affect Netlify.

Netlify verifies site access, hashes the staged files, and creates an asynchronous
file-digest deployment; it does not build or upload a ZIP. The manifest includes
every selected report path and its SHA-1, as required by Netlify's API. Only
content Netlify requests is uploaded, with at most four file requests in flight.
Identical content is uploaded once even when used at multiple paths. Existing
reference images and shared assets can therefore be reused across deployments.
Netlify still publishes the complete report atomically, after every required
file is accepted and the deployment becomes ready.

The client limits Netlify deployments to 25,000 paths, 2 GiB of staged content,
and a 16 MiB manifest; it bounds API response bodies to 2 MiB. File paths and
content are revalidated when uploading. Authentication, permanent HTTP errors,
and changed or unreadable staged files fail without automatic retries. File
uploads and read/poll requests retry transient errors up to six attempts with
bounded backoff and numeric Retry-After support. Site validation and status reads
also retry timed-out or interrupted response bodies after successful HTTP headers;
malformed JSON and oversized bodies still fail immediately. Retry logs distinguish
response-body timeouts from interruptions, report received byte counts without
response content, and identify when all six attempts are exhausted. A failure
during `Validating Netlify deployment target` occurs before any report upload;
if it persists, check the connection, proxy/firewall, and Netlify's service status.
Deployment creation is not
automatically retried because a failed response may still have created a deploy.
Check Netlify's deploy history before retrying a failed or canceled deployment.
Locked production deploys and Git-only production restrictions are reported
before upload. Azure authentication, network restrictions, and restricted role
rules are rejected instead of silently publishing unprotected reports on Netlify.
Configure access protection in Netlify itself for private reports. Netlify allows
up to one hour for hashing, uploads, retries, and publication polling. Each file
request uses that timeout but cannot outlive the overall deadline. Connections
retain a 10-second timeout; manifest creation and read/poll requests use 30 seconds.
Azure retains its ZIP/block-upload protocol and 30-minute deployment deadline.
Both providers keep tokens out of URLs and logs.

Provider failures include available structured error details, HTTP status,
request/deployment identifiers, and recovery guidance for access, storage,
billing, throttling, and provider outages. This includes Azure's `errorMessage`
and `errorDetails` and Netlify's `error_message`. Details are bounded and
credential/signed-URL redacted; unparseable responses fall back to status-specific
guidance. The full error is shown in deployment Activity and View Log.

Both providers send `X-Robots-Tag: noindex,nofollow` for report URLs. Snapshots
include matching Azure/Netlify security and cache policies, plus clean-URL SPA
fallback rules. HTML revalidates; `/_snapshot/*` uses immutable one-year caching.
All resource URLs include `?v=...`; any additional CDN must include that parameter
in its cache key. Stable origin files contain only the current report version.
These directives ask search engines not to index reports, but do not restrict
access. See [snapshot site hosting](commands.md#snapshot-site) for other hosts.

The last selected target is remembered per workspace, and removing a target
requires confirmation. Deploy and target management are also available from the
command palette. Deploy immediately shows a loading state, then opens Runs >
Activity with a highlighted deployment band, animated spinner, target, current
stage, and elapsed time. Deployment activity replaces scenario counters with
deployment-specific feedback. Completion, failure, and cancellation have distinct
status icons and colors; View Log remains available, and successful deployments
offer Open Site.
Activity also shows the 12 most recent nonempty worker log lines, bounded for
responsive rendering; View Log retains the full bounded run log. Staging and
packaging report file/byte counts, optimization reports actual savings, and
Netlify reports reused paths, per-file bytes sent, and accepted file/byte counts;
Azure reports acknowledged bytes. Azure block commits,
retry reasons/delays, and each provider polling state are logged. A five-second
heartbeat continues during long asynchronous operations, without claiming
unmeasured completion percentages or provider acceptance.
Netlify samples each active file upload every 60 seconds. If its byte count has
not advanced since the previous sample, Activity and View Log show a warning
each minute identifying the file number, elapsed time without observed progress,
and sent/total bytes. The upload keeps running, so users can keep waiting or
choose Stop. Once a file's bytes have been sent, warnings describe waiting for
Netlify's acceptance instead of a stalled transfer. Retry attempts restart that
file's sent counter; accepted totals count each required content only once.
Stop cancels the local worker process tree; it cannot undo a deployment already
accepted by Azure or Netlify. Check the provider's deploy history before retrying
after cancellation. Deployment
transport is covered by local mock-server tests; a live Azure smoke deployment
requires a configured target and is not performed by the test suite.

Optional non-publishing API checks use `AZURE_STATIC_WEB_APPS_API_TOKEN`,
`NETLIFY_PAT`, and `NETLIFY_SITE_ID` from the process environment:
`cargo test --locked --features desktop --lib live_deployment_ -- --ignored --nocapture --test-threads=1`.
These tests validate the Azure tier and Netlify site access without creating a deploy.

An explicitly publishing Netlify smoke test is also available after setting
`NETLIFY_PAT` and `NETLIFY_SITE_ID` in the process environment:
`cargo test --locked --features desktop --lib live_publish_netlify_digest -- --ignored --nocapture --test-threads=1`.
Use a dedicated test site: this replaces its current content with a tiny
synthetic report, publishes twice to exercise content reuse, and checks the
public page and stylesheet. It is excluded from the `live_deployment_` preflight
filter and all ordinary test runs. Load credentials privately from a local
environment file when needed; never print or commit their values.

Native Windows deployment/layout verification uses
`./ci/desktop-smoke.ps1 -SkipBuild -DeploymentFlows` after building the GUI. It
creates isolated target settings, exercises target persistence and subset
confirmation, checks masked token entry and machine-local credential persistence
across edits and restarts, and verifies credential deletion for Azure and Netlify.
It checks provider switching and Netlify Site ID persistence, captures desktop
and compact light/dark layouts, and verifies missing-token failures without
contacting either provider.
It refuses a locked session before changing preferences and cleans up only its
own fixture credentials, settings, and processes.

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
dependencies and exercises installation, visible-update foreground requests,
fully silent setup, active-process blocking, downgrade rejection, shortcuts,
folder context menus and uninstall cleanup before upload. Windows may deny
foreground activation on a noninteractive runner; the smoke test reports that
as a warning while still requiring the activation request. Local packaging of a
desktop build whose executable version matches `0.2.0.0`:

```powershell
./ci/build-desktop-installer.ps1 -Version 0.2.0.0 -BinaryDirectory target/debug -Compiler 'C:/Tools/Inno Setup 6/ISCC.exe' -SmokeTest
./ci/desktop-smoke.ps1 -SkipBuild -UpdateFlows
```

The installer smoke test refuses to run over an existing per-user installation,
shortcut group or context-menu registration. It installs only under a generated
`target/installer-smoke-*` directory, suppresses relaunch, verifies both shortcuts
and their Jump List identity, and checks the folder context-menu commands.
Chromium requests in the smoke child processes use an unreachable loopback proxy:
setup must reuse an existing cached build or log a non-fatal network failure,
without downloading a browser. The fixture also checks `/SKIPCHROMIUM=1`.
Uninstall must remove the fixture shortcuts, context menus and installation
registration. The native UI smoke requires an unconfigured development build and
temporarily backs up/restores preferences, like the other desktop smoke modes.

The opt-in native Jump List test uses a unique application ID, temporary workspace
history and automatic cleanup without modifying the installed app's Jump List:

```powershell
cargo test --features gui-tests --lib desktop::jump_list::tests -- --include-ignored --test-threads=1
```

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
Selected rows are highlighted without checkboxes; a left-edge marker identifies
the suite or scenario currently open. The lower pane lists only the open suite's scenarios, with
labels and URLs; actions are edited in the scenario's action sequence rather
than nested in the sidebar. Both panes scroll independently.

Click a row to select only that item in its list. Ctrl-click toggles individual
items (Cmd-click also works on macOS), and Shift-click selects a range from the
last clicked item in visible list order. Ctrl+Shift-click adds a range to the
existing selection. Hidden filtered rows are excluded from ranges.
Click empty list space or None to clear selection; All selects every valid suite
or every scenario in the open suite, including filtered-out items. Scenario
selection changes only the open suite's scope, preserving other selected suites.
Clearing selection does not close the editor or discard its draft.

In Results, the lower pane becomes Captures for the active suite. Selecting a
suite there changes the report being reviewed without opening another editor
document or disturbing an unsaved draft.

Selecting a suite selects all of its scenarios; clicking an individual scenario
narrows that suite's execution scope and opens its editor. The sidebar's play
button runs only that row's scenario. Runs > Suites still expands each suite into
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
selection. Selected narrows the upper list to selected suites. Ctrl+P (Cmd+P on
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
Open in VS Code opens the current workspace in an external VS Code window.
Open in File Explorer (Windows), Open in Finder (macOS), or Open in File Manager
(Linux) opens the workspace folder in the operating system's default folder viewer.
These actions require an open workspace and do not save or discard editor drafts.
Save As takes a workspace-relative destination and never replaces an existing
file. Reload discards a draft after confirmation. Suite-file deletion is
confirmed but cannot be undone through document history.

The title bar displays the package version beside Regressify. Help > About shows
Regressify Desktop, its version, author Tuyen Pham, and the clickable website
`https://tuyen.blog/`, which opens in the external default browser.
There is no bottom application or Results status bar. Application status and
Report status icons in their respective headers retain messages in tooltips and
accessibility labels, leaving the bottom edge available for content.

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
controls. Scenarios/actions have dedicated collection editors; Basic Auth,
URL replacements, and other object collections use structured YAML/JSON input. Generated runtime `testDate`
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

Editor tabs are ordered Source, Fields, and Effective. Selecting a test suite in
the editor opens Source, including reselecting an already-open suite. Selecting
a scenario or action opens its Fields. Suite selection while reviewing Results
stays in Results and switches the displayed suite.

Switching Source, Fields, and Effective reads pending input before changing the
view. Invalid source stays in Source when Fields cannot represent it, with its
diagnostic retained. File-menu actions use the same availability rules as command
search, including disabling suite deletion for unsaved documents.

Source has local YAML/JSON schema completion. Suggestions appear while typing
property names, supported values, and documented examples; Ctrl+Space opens the menu
explicitly (also Ctrl+Space on macOS). Arrow keys choose an item, Enter accepts,
and Escape dismisses. Keys follow the schema through nested objects and lists,
including Basic Auth, URL replacements, iframe settings, and navigation options.
Required fields appear first; existing sibling keys and unrelated action fields
are excluded. Suggestions include the expected type, descriptions, units, and
safe examples. Structured examples insert valid YAML flow notation or JSON.
Accepting a suggestion replaces only the partial token, preserving surrounding source.

Hover over a field or value for its schema description, required/optional status,
expected type, and examples. These hints contain schema metadata, not entered
credential values. Arbitrary data files, JavaScript, and live-page CSS selectors
are not completed. JSON files and Effective use JSON highlighting; YAML uses YAML.

Schemas are generated automatically from the Rust types with `schemars`.
Field documentation, examples, and constraints live beside those types and feed
both the desktop editor and schemas exported by `regressify init` for VS Code.
There is no separately maintained desktop schema, network schema lookup, or
language-server installation. Validation uses an offline, cached JSON Schema
validator plus the existing semantic checks. Tests check field-help coverage and
validate documented examples against the generated schemas.

For example, a Basic Auth entry without `origin` reports
`basicAuth[0].origin: Missing required field`, followed by an origin example and
an explanation. A complete entry is:

```yaml
basicAuth:
  - origin: https://example.com
    username: "${BASIC_AUTH_USERNAME}"
    password: "${BASIC_AUTH_PASSWORD}"
```

Set those environment variables before launching Regressify. The origin includes
the scheme and optional port, not a page path. Each entry needs all three fields;
the single-object form is also supported. In the editor, indices such as `[0]`
refer to the first list item. Generated-schema diagnostics never echo entered
values. Runtime checks still determine whether files, environment variables,
websites, and elements actually exist or work. `init` remains non-destructive:
it does not overwrite existing exported schemas or VS Code settings. The desktop
uses the schema built into its current version, independent of those files.

The source toolbar provides Find, Replace, and Suggest, plus word wrapping and
text sizes from 10 to 24 pixels. These presentation changes preserve source and
undo history. The status strip shows the language, cursor position, and issue
count. Syntax errors underline their source line; schema errors identify the
field and offer corrections for missing fields, misspellings, types, supported
choices, and numeric limits. Up to 50 issues are reported together in a scrollable
panel. Each line link unfolds and focuses the affected field, or its enclosing
object when a required field is missing. The issue button moves to the next
issue, wrapping to the first. Fixing the draft clears resolved diagnostics.

`./ci/desktop-smoke.ps1 -SchemaFlows` verifies missing-origin guidance, credential-safe
messages, issue jumps, nested completion acceptance, save guards, and compact/large
light/dark layouts in a synthetic workspace on an unlocked Windows desktop.

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
Initialize Workspace. Navigator rows and the checkboxes in Runs support multiple
selections and All. No selection disables suite commands, never an
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
suite, on its Runs/Overview row, and for a compatible suite selection.
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
Auto (the default) and Always use the managed Chromium revision shipped with
Regressify, downloading it only when missing from the shared per-user browser
cache. On Windows this is `%LOCALAPPDATA%\regressify\chromium`; all workspaces and
CLI runs reuse it. Concurrent installations are serialized with a file lock.
Never uses that same cached build without downloading; run Install Chromium once
if the shared cache is empty. Captures never auto-detect installed Chrome or Edge,
and missing or invalid managed executables fail the run instead of switching
browsers. Use the same Regressify version across QA machines and review baselines
again when changing browser revisions.
During installation, Activity distinguishes cache-lock waits, connecting,
downloading (MiB, percentage when known, speed, elapsed time, and idle time),
extraction, and readiness. Status refreshes every five seconds during long steps;
View Log includes the download URL, cache/staging paths, limits, and error details.
The Proxy run setting also applies to Chromium downloads. Network stalls fail
with recovery guidance instead of waiting indefinitely. See
[Chromium installation diagnostics](commands.md#chromium-installation-diagnostics)
for time limits and QA troubleshooting checks.
Invalid settings block only commands that use them. Other drafts stay visible
and retained, and unrelated options are omitted from the execution request.

Each request freezes its command, options, logical suites, and exact scenario
IDs when scenarios are selected. The
run-specific draft prompt offers Save and continue or Run saved version, plus
Cancel. Saving cannot silently switch the requested command. Preflight resolves
those exact suites again, including any newly saved output IDs; it never substitutes
another same-sized selection or expands an empty selection to all suites.

Selecting a scenario in the navigator also selects it for execution. Use Ctrl or
Shift to select multiple scenarios, the direct scenario actions to run one, or
the checkboxes in Runs to adjust scope without navigating. Scenario runs
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
latest completed run's retained logs are saved with its activity and restored
when the workspace reopens. Starting another command replaces the displayed
activity; its completion replaces the saved snapshot. Historical comparison
reports do not contain execution logs. Custom scripts, URLs, selectors, and
error messages may still contain project-provided data in the local snapshot.

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

The Test suites panel retains each suite's latest counts when another suite runs
and after restarting the app. These per-suite summaries are saved separately from
the current Activity totals and scoped to the output directory/environment.
Missing summaries are loaded from each suite's latest saved comparison report at
startup, including suites outside the most recent run. Capture-only and cancelled
statuses remain distinct from passed comparisons.

Completed, failed, and cancelled run activity persists across app launches.
Reopening the same workspace restores its last command, suite/scenario selection,
pass/fail counts, viewport rows, failure details, timings, bounded logs, and output
directory settings. It does not restart a worker. The recorded run ID remains
available for report review; the next capture gets a fresh ID. Proxy credentials
are not included in the saved settings.

Activity is saved atomically under `runs/<workspace-key>.json` alongside the
desktop preferences, separately for each workspace. Completion waits for the save;
save/load failures remain visible. This is the latest activity snapshot, not an
unbounded execution history. Screenshots and reports remain in their existing
output directories. If there is no readable snapshot, the app recovers the newest
matching comparison run from the default `.regressify` output. These older results
are labeled Saved comparison results; original execution logs and timings cannot
be reconstructed from reports and are not fabricated.

## Reviewing Results

Results uses one left navigator with Test suites above Captures and one detail
area on the right. Only the active suite's captures are shown. A suite without a
report shows an empty capture list, never another suite's results. The report
title is read-only; there is no separate report dropdown.
While reviewing Results, suite pass/fail badges use the loaded reports' scenario
and viewport totals, not older saved Activity totals. Saved steps and targets do
not inflate these totals. Live execution counts remain visible during a run.

Approve This Scenario promotes the current scenario's successful captures for
the displayed viewport only, including its saved steps and targets in this report.
Other viewports and scenarios keep their existing baselines; approve each viewport
separately when needed. The confirmation identifies the scenario, viewport, and
exact reviewed run, including historical runs, directly in the current comparison.
Cancel dismisses it without changing baselines; switching captures or refreshing
also dismisses the pending confirmation. Approval stays in Results rather than
switching to Runs, and failures appear in the comparison. Its output folder and suite output
identity are pinned; changes to the suite output or capture metadata stop approval
for review. Failed or missing test captures and reports without scenario metadata
cannot use this action. Valid test captures with missing references or changed
dimensions can be approved; failed sibling captures remain unapproved.

After approval, the same report refreshes in place with recalculated mismatch
percentages, dimensions, suite pass/fail badges, and diff images. The selected
capture, tab, and zoom are retained when the capture still matches the filter.
Recalculation preserves the original
report scope, order, labels, URLs, and thresholds, including all unapproved failures.
It does not capture the website again. Results has no whole-run approval button;
use Approve Results in Runs for suite-wide or all-viewport approval. Ordinary Runs
approval retains its existing behavior and does not automatically recompare.

Capture rows use scenario labels, with viewport and saved-step/target details on
the secondary line. Newly captured labels are preserved in report metadata;
older runs resolve names from the current workspace where an ID matches
unambiguously. Unresolved generated IDs fall back to the recorded page URL or an
unnamed-scenario label. Raw capture IDs remain searchable and available in
tooltips. Generated desktop run UUIDs are presented as report timestamps;
timestamp-based CLI run IDs are formatted as local dates. Custom run names remain
unchanged.

The compact comparison heading shows the scenario name and measured difference.
Hover over the information icon beside the name to see Label, Selector,
Reference URL, Test URL, Diff %, Diff X, and Diff Y. Selector is the recorded
capture target, such as `document` or `body .carousel`. Each recorded URL can
be opened in the external default browser or copied using its adjacent button.
When both URLs match, Test URL displays Same URL but still opens and copies the
actual URL. Diff X and Diff Y are the signed width and height differences in
pixels (test minus reference). Missing metadata displays Unavailable rather
than an inferred value; unavailable URL controls are disabled.
Capture context, allowed mismatch, page, failure details, and raw capture ID are
available in the Copy capture details tooltip and copied summary, without extra
metadata rows above the images. Image panes have no repeated headings or
external-open buttons; image actions are in the context menu. Report totals are
on Refresh reports. Edit suite opens
the owning workspace file through the same guarded navigation used by the
sidebar; it is unavailable when that association cannot be resolved safely.

Results refreshes on opening and after commands finish using current output
options. It loads each suite's `bitmaps_test/<run>/report.json`, newest first,
and shows the latest report for the active suite. Open Report pins its requested
runs, including on refresh and while switching between their suites. Selecting a
suite outside that request returns to latest-report browsing. A selected capture is
retained when its report is unchanged. Filter
All/Failed/Passed and search labels or errors. The preview follows the filtered
selection and clears when nothing matches. Captures use a continuously scrolling,
virtualized list without pagination. Select a capture directly from the list;
filtering and refresh keep the selected capture visible when it still matches.
The navigator toggle hides both left panes for a full-width preview without
resetting selection, filters, images, or zoom.

Pass, mismatch, engine/capture error, and unknown status are distinct. Unknown
statuses never count as passes. Malformed reports warn without hiding valid
reports. Reports whose suite ID disagrees with their output directory are
rejected. Ordinary browsing reads only the newest valid report per suite;
explicit run requests exclude unrelated reports before applying the discovery
limit of 1,000 report reads, limited to 16 MiB each.

Reference, Test, Diff, and Side by side display background-loaded PNGs.
Side by side is selected by default and whenever a different scenario is selected.
Switching between captures of the same scenario or refreshing its report preserves
the selected tab. Legacy reports without scenario IDs return to Side by side for
each different capture.
Fit and zoom controls sit to the right of these tabs on the same toolbar.
Fit area is enabled by default and uses the smaller width/height scale so the
entire image is visible, capped at 100% to keep small images at their original size.
Fit width scales to the viewport width, also capped at 100%, with vertical
scrolling for tall images. The two fit modes are mutually exclusive.
Preview canvases are horizontally centered when narrower than their viewport.
These defaults apply to Reference, Test, Diff, and Side by side.
Each tab independently remembers its fit mode and manual zoom value while viewing
the same scenario, including across report refreshes and viewport captures of that
scenario. Selecting a different scenario, leaving Results, or restarting the app
resets every tab to Fit area. Legacy reports without scenario IDs reset on each
different capture.
Zoom buttons and Ctrl+mouse wheel use [Chrome's zoom presets](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/common/page/page_zoom.cc):
25%, 33%, 50%, 67%, 75%, 80%, 90%, 100%, 110%, 125%, 150%, 175%, 200%, 250%,
300%, 400%, and 500%. Each action selects the nearest preset in the requested
direction, starting from the displayed scale. Fit area remains exact and can
shrink below 25%; zooming out at that size leaves the image fitted.
Ctrl+mouse wheel keeps the image point under the pointer fixed. Left-button
dragging pans images that exceed the viewport; ordinary wheel scrolling,
scrollbars, and actual-size reset remain available.
Side by side shows Reference and Test, followed by Diff on the right when a
diff image can be loaded. Clicking an image opens its corresponding tab and
restores that tab's own fit/zoom choice; dragging does not switch tabs.
All comparison panes use a common canvas and scale, including when their
dimensions differ. Zoom, dragging, and both scroll axes stay synchronized from
any pane. Moving the pointer over an image draws horizontal and vertical guides
in all comparison panes at the same image coordinate. Guides disappear when the pointer leaves the
images, and selecting another capture resets the shared scroll position.
Image dimensions are bounded
with the CLI's PNG helper before decoding. When a reference capture failed and
no baseline image exists, Reference displays the shared rounded report error
image. Failed Test captures use the same artwork. Older Reference and Test images
with capture-failure markers also display this rounded image in memory, without
rewriting their files. These previews never create or promote a baseline.
Successful reference images and the capture's error message remain intact.
Other missing images, invalid PNGs, and unsafe paths retain their diagnostics.
Paths resolve relative to `html_report` and must remain inside the output root.

Right-click any saved Reference, Test, or Diff image, including in Side by side, to open
it in the default browser, open it in the default image viewer, or reveal it in
the OS file manager. In-memory error placeholders have no file context menu.
Reference also offers Open Reference URL and Copy Reference
URL; Test offers Open Test URL and Copy Test URL. These actions use that capture's
recorded initial scenario URL: Reference uses `referenceUrl`, while Test uses
`url`, which includes the replacements applied when the test ran. These URLs do
not use the final URL after actions or recompute from today's replacement profile.
URL actions are disabled when the corresponding metadata is missing or unsupported;
they are not offered for Diff. HTTP, HTTPS, and local file URLs are supported.

External-open controls also launch the HTML summary and artifact folder.
The app does not embed a browser or webview; browser actions launch the installed
default browser as a separate application. Historical reports refer to current baseline
files; later approval can change their reference image.

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

The [runner](../ci/desktop-smoke.ps1) keeps the public switches, safety guards,
build, and flow dispatch. Its dot-sourced modules share the runner's script scope:

| Module                                           | Responsibility                                                              |
| ------------------------------------------------ | --------------------------------------------------------------------------- |
| [native.ps1](../ci/desktop-smoke/native.ps1)     | Windows interop, window state, screenshots, and native buttons              |
| [controls.ps1](../ci/desktop-smoke/controls.ps1) | Accessible controls, input, themes, and application menu assertions         |
| [fixture.ps1](../ci/desktop-smoke/fixture.ps1)   | Synthetic workspace, process tracking, settings backup, and fixture helpers |
| [cleanup.ps1](../ci/desktop-smoke/cleanup.ps1)   | Process teardown and restoration of preferences and fixture state           |
| [flows](../ci/desktop-smoke/flows)               | Feature-specific acceptance flows and shared Results assertions             |

Keep shared cleanup state script-scoped when moving code into flow functions.
Run [the headless harness checks](../ci/test-desktop-smoke.ps1) with
`./ci/test-desktop-smoke.ps1` after changing the harness. These checks need only
PowerShell 7: they parse and load helpers, exercise switch dispatch and cleanup
with mocked side effects, compile and check native keyboard event sequences
without sending input, and verify draft cleanup state. They do not build the
app, open windows, or touch preferences; native acceptance still needs Windows.
After editing the inline Windows interop code, run native checks in a fresh
PowerShell session because `Add-Type` definitions are cached for the process.

Add `-Capture` to run direct reference capture, stop a test, rerun the test, and
open the completed results for three local fixture scenarios across two suites.
The fixture reuses the shared managed Chromium cache, downloading the browser only
if needed. The script asserts live pass counts and capture metadata. Test pages
use local HTML only and do not contact an external test website.

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

Use `./ci/desktop-smoke.ps1 -ComparisonFlows` for native Results acceptance without
browser capture. It verifies the information popup and URL clipboard contents,
including distinct URLs and Same URL, image context menus, header-only controls,
fit/zoom, drag panning, linked scrolling, and crosshair pixels in both themes at
1600x1000 and 1080x720. Image navigation uses the continuous capture list.
All smoke modes also check the title-bar version, File workspace-opening items,
and Help > About. External launch items are checked without opening other apps;
URL copy tests restore the clipboard afterward.

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
keyboard settings. Shortcuts use `Send-FixtureKeys` with a `ConsoleKey` and
explicit `-Control` / `-Shift` switches, not SendKeys strings. Native `SendInput`
batches press and release modifiers in order and mark navigation keys as extended;
failed batches attempt to release their keys before reporting the error.
Source-completion setup verifies the caret position between navigation keys
before entering a new property. Native menu items use enabled
state checks and physical clicks rather than assuming an Invoke automation pattern.
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

The full authoring smoke now passes with native keyboard shortcuts, including
viewport and dependency controls, document-start navigation, source completion,
saving, historical-run approval, appearance restoration, and dirty-draft cleanup.
The schema flow also passes with the shared input helper, including clipboard
paste, diagnostic-text copying, completion, and corrected saves in compact
light/dark windows. Both runs restored original preferences and closed only
their synthetic fixture.

Full IME composition, screen-reader usability, and native macOS/Linux behavior still require testing
on those environments. The Linux CI model job is not a claim of native GUI
validation there. Signed installers, cloud accounts, collaborative editing,
browser recording, and non-Chromium execution are outside this implementation.
GPUI 0.6.1 has a remaining managed-tooltip issue after some programmatic view
changes; this does not affect document or result data.
