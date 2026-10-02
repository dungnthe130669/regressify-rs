# Outstanding Issues

Issues identified during adversarial review that remain after the September 2026 hardening pass.

## Diagnostic Retention Follow-up (2026-10-02)

- Resolved: command failures were retained at informational severity; non-capture
  commands lacked operation journals and desktop saved-log selection. Startup,
  command, and capture/deployment completion records now preserve outcome severity.
- Resolved: optimizer results, approval decisions, and snapshot cleanup warnings
  were console-only. Typed records now preserve safe details, including worker-thread
  optimization results and Windows cleanup OS codes without changing command outcomes.
- Resolved: the desktop GUI did not install a subscriber or retain startup/native
  warnings. An entry-point-owned, nonblocking application journal now covers them.
- Resolved: out-of-range log timestamps could crash the frontend timeline. The
  schema rejects them and desktop/mobile tests verify a recoverable loading error.
- The [follow-up audit](docs/logging-audit.md#follow-up-review-2026-10-02) records the
  remaining print classifications and review boundaries. This change does not
  resolve the separately tracked worker-pipe timeout or Git discard findings.

## Frontend Formatting Baseline (2026-10-01)

- `bun run --cwd frontend lint` initially reported seven existing formatting
  errors during logging schema work. Five remain in untouched files:
  [fixture generator](frontend/scripts/create-fixture.ts), [App](frontend/src/App.tsx),
  [shared UI](frontend/src/components/ui.tsx), [overview](frontend/src/pages/OverviewPage.tsx),
  and [suite page](frontend/src/pages/SuitePage.tsx). Logging source/test files pass
  scoped Biome checks; frontend tests, typecheck, and production build pass.
- Keep formatting fixes scoped to files changed by the logging tasks rather than
  reformatting unrelated frontend code. Report the full-lint failure separately
  until the remaining baseline files are intentionally normalized.

## Worker Snapshot Inventory Assertion (2026-10-01)

- The worker integration test `filtered_snapshot_stages_only_selected_suites_and_their_public_dependencies`
  expects two staged images but observes five. Its count assertion does not account
  for the generated preview dependencies described below.
- The exact test fails identically with the new logging subscriber hooks removed
  from the worker. The other ten non-ignored worker integration cases passed.
  This is a pre-existing test gap, not a subscriber-induced asset change.
- Resolved by [logging packaging Task 027](docs/plan/log-revamp-tasks/027-snapshot-log-packaging-inventory.md):
  the assertion now compares the exact referenced original/preview image set
  rather than assuming two files. A separate exact ZIP inventory test covers
  selected-only log chunks and excludes raw journals and unselected captures.

## Large-Suite Preview Performance (2026-10-01)

- Confirmed: a read-only browser probe of 44 full-page captures downloaded about
  104 MiB of PNGs during scrolling. Their combined uncompressed pixel size was
  about 1.9 GiB, not a measurement of resident GPU memory; frame stalls reached
  517 ms in that run. The JavaScript transfer was only about 180 KiB.
- Resolved in generation/frontend: bounded small previews replace original-image
  requests while browsing, and distant preview sources are released. Original
  comparison/download images remain unchanged. Existing sites need regeneration
  and redeployment; the live report was not modified.
- Preview generation uses fast fixed PNG encoding, no expensive optimization,
  and byte-for-byte repeatability tests. A 300-capture browser regression covers
  bounded resources, filtering, view changes, distant failure jumps, and review.
  Desktop tests cover exact staging inventories, version refresh, and cleanup.

## Deployment UI And Suite Links (2026-10-01)

- Resolved: deployment dialogs now use a left-aligned native link with adjacent
  Copy and omit suite listings while retaining subset confirmation. Deployment
  activity uses a single-line suite summary, omits capture statistics, and fills
  the remaining height with a selectable, read-only log editor.
- Runs adds Open Site beside Open Report, using persisted publication membership
  and completion order to find the latest site containing the selected suites.
  Subset redeployments replace membership; failures preserve previous links.
  Legacy target URLs gain suite membership on the next successful deployment.
- Focused model and GPUI checks cover reloads, target changes, private publication,
  safe deep links, full-height layouts, and exact copying. The deployment slice
  passes 103 local tests with six live-provider checks ignored; no site was published.

## Popup Captures (2026-09-30)

- Resolved: actions and captures previously stayed on the original scenario page.
  The CLI and desktop now support explicit `switchTab: new | previous | original`
  with an optional new-tab URL filter. A URL wait alone still does not switch tabs.
- Local Chromium regressions cover `_blank`/`noopener`, JavaScript and iframe
  popups, redirects, URL replacements, ambiguous/missing/closed tabs, first-load
  viewport/auth/storage, and checkpoint/final screenshot pixels. Native desktop
  coverage verifies adding the action from the navigation menu and undoing it.
- Default CLI, desktop-model, and GUI/worker gates pass, as do production GUI
  Clippy and the desktop build. Interactive schema smoke now passes after fixing
  its selected-line endpoint setup; see the source-line smoke note below.
- See [action syntax](docs/create-action.md) and [tab ownership](src/test/tabs.rs).

## Source-Line Smoke Selection (2026-09-30)

- The `Ln 3, Col 1` timeout was caused by assuming two `Shift+Down` presses after
  `Ctrl+Home` ended at column one. A keyboard-driven GUI regression reproduced
  selection `0..28` instead of `0..27`: the first character of the third line was
  selected. The earlier regression set the selection directly and missed this.
- Fixed the smoke setup by finishing with `Shift+Home` before checking the caret.
  Exact selected-range, status-label, and resulting-text assertions remain. This
  makes the line-command fixture deterministic; production caret code is unchanged.
- `./ci/test-desktop-smoke.ps1`, the focused keyboard GUI regression, and
  `./ci/desktop-smoke.ps1 -SchemaFlows` pass. Native coverage includes all line
  commands, undo/redo, menu editing, corrected saves, completion after Alt+Tab,
  and compact/large light/dark layouts. Cleanup restored desktop preferences.

## Windows Cloudflare Access Denied (2026-09-30)

- QA's final log identifies error 5 when removing a confirmed-empty generated
  `_snapshot/images/<suite>/diff` directory in a OneDrive workspace. Snapshot
  publication had completed; optional directory cleanup incorrectly aborted
  deployment before upload. No additional log is needed to identify this blocker.
- Fixed: failure to remove a confirmed-empty generated directory now warns and
  leaves it in place; an already-removed directory needs no warning. Directory
  reads, artifact writes, stale-file deletion, and symlink checks remain strict.
  Empty directories are excluded from the deployment inventory; captures and
  permissions are unchanged.
- Windows model and native-worker regressions block directory deletion with a
  sharing handle, verify successful snapshots and warnings, preserve captures and
  manifest paths, and confirm cleanup on a later unblocked run. Existing locked
  artifact tests still fail as required. The simulated OS error is 32, not the
  exact OneDrive/ACL error 5; both use the fixed removal-error branch.
- QA should retry deployment with this fix. The particular Windows component
  denying deletion is not established. No live deployment or change to user
  credentials/security settings was performed.

## Open Review Findings (2026-09-23)

These are unresolved findings, not implemented fixes. The review followed CLI,
browser, desktop editor/worker, Git/deployment, and update/release boundaries;
it was risk-focused, not an exhaustive security certification.

### P1: Discard Does Not Detect Content-Only Changes After Review

- Evidence: [Change](src/desktop/git.rs#L36) stores paths and Git status flags, not
  reviewed content identities. [Discard validation](src/desktop/git.rs#L530)
  compares only those records before restoring/deleting selected files.
- Trigger: review an already modified or untracked file, open discard confirmation,
  then have another editor rewrite that file without changing its status flags.
  The current/expected comparison still succeeds and the newer edits are removed.
- Verification: traced through the [confirmation callback](src/desktop/ui/git.rs#L659)
  and model. Existing stale-discard coverage changes staging status; it does not
  cover different bytes with the same status. No destructive live-workspace probe ran.
- Repair/test: retain and revalidate reviewed content/index identities for selected
  paths, reject changes before any mutation, and preserve all files on rejection.
  Add tracked, untracked, and same-status staged-content regression cases in the
  existing Git test module using temporary repositories.

### P2: Desktop Worker Pipe Joins Escape Timeout and Cancellation

- Evidence: the [desktop worker](src/desktop/runner.rs#L693) has an unbounded
  reader-join boundary after the parent exits.
- Trigger: an engine descendant outlives its parent and retains
  an inherited pipe. The reader waits for EOF; the group handle is not released,
  and subsequent cancellation no longer reaches the finished parent wait loop.
  Worker close can hang.
- Verification: static control-flow review plus the pinned command-group 5.0.1
  parent `try_wait` behavior. Existing cancellation tests keep the parent alive;
  the parent-exits-first case was not executed during this review.
- Repair/test: bound writer/reader completion and preserve cancellation/process-tree
  cleanup until all streams close. Add a fixture whose parent exits while a child
  retains stdout/stderr; assert bounded completion, cancellation, and no descendants.
- Git counterpart resolved on 2026-09-24: its process wrapper keeps checking the
  deadline/cancellation until readers and the stdin writer complete. A local
  parent-exits-first helper regression verifies bounded completion.

## Review Checks (2026-09-23)

- `cargo test --features desktop --lib`: 516 passed, 9 ignored, no failures.
- Ignored Chromium/Node fixtures, GUI interaction, installer execution, and live
  provider/update services were not run. Test success does not resolve the gaps above.

## Asset Version Resolution (2026-09-28)

- The recurring generated JavaScript version failure was Git CRLF conversion,
  not a Rust release-mode hash difference. Normalizing checkout bytes to LF
  reproduced the version embedded in the HTML.
- [.gitattributes](.gitattributes) now preserves all generated snapshot bytes.
  Regenerate existing converted checkouts with `bun run --cwd frontend build`.
  The existing Rust resource-version test passes in release mode; the
  [asset-sync regression](frontend/scripts/sync-assets.test.ts) covers LF and
  CRLF inputs with Git autocrlf enabled.

## Pixel Threshold Validation (2026-09-29)

- The default CLI all-targets gate passes with 376 tests and 9 optional checks
  ignored. The GUI all-targets gate passes with 863 library tests and 18 ignored,
  plus 9 worker integration tests and 2 ignored. Production GUI Clippy with
  warnings denied, the desktop build, Rust formatting, and guidance checks pass.
- Native `./ci/desktop-smoke.ps1 -SchemaFlows -SkipBuild` passed through
  Source/Fields diagnostics, issue selection/copy, compact light/dark layouts,
  and Enter/Tab/Escape completion after reactivation. It then stopped in the
  existing source-line command check because `Ln 3, Col 1` was not found.
  Root cause is not established; this is not a full schema-smoke pass. Cleanup
  discarded the fixture draft and restored original desktop preferences.
  The isolated native `pixel_match_threshold_controls` test passes.
- The source-line smoke blocker was subsequently reproduced and resolved on
  2026-09-30; see Source-Line Smoke Selection above for the passing native rerun.

## Validation Notes (2026-09-27)

- The side-by-side Git text comparison passes all 49 checks in
  `cargo test --locked --features gui-tests --lib git:: -- --test-threads=1 --quiet`,
  plus production GUI Clippy, the standalone desktop build, and the headless smoke
  harness. Native `./ci/desktop-smoke.ps1 -GitFlows` and a `-SkipBuild` retry
  completed the text-diff, exact-copy, malformed-JSON, and selected-comparison-commit
  checks. Both captured light/dark screenshots at 1600x1000 and 1080x720.
- Full native Git smoke still fails later in remote setup: the first rerun's
  `Remote URL` control did not contain the supplied local repository path; the
  retry's `Remote name` control did not contain `review`. The failure is reported
  by `Wait-SourceText` in [native controls](ci/desktop-smoke/controls.ps1).
  Root cause is not established; neither attempt is a full Git-flow pass.
  Cleanup restored the original desktop preferences after both attempts.
- `cargo test --locked --features gui-tests --all-targets -- --test-threads=1 --quiet`
  reaches 829 passing library tests and 15 ignored checks, including the corrected
  source-save regressions. The existing embedded frontend resource-version test
  still fails for `_snapshot/assets/app.js`; Cargo stops before later test targets.
  No frontend assets were changed during the save repair.

## Shared Deployment Optimization (2026-09-29)

- Azure and Cloudflare now share budget-aware PNG optimization in temporary
  copies: 250 MiB for Azure Free/unknown, 500 MiB for Standard, and 2 GiB for
  Cloudflare's selected assets. Sources stay unchanged, Cloudflare exclusions and
  per-file limits remain enforced, and snapshot cache versions follow optimized bytes.
- `cargo test --locked --features desktop --lib -- --quiet` passes with 661 tests
  and 17 optional checks ignored. Coverage includes optimized Cloudflare manifests,
  source preservation, limits, exclusions, cleanup, and snapshot cache versions.
  No live deployment was attempted.

## Cloudflare Validation (2026-09-28)

- The guarded temporary-Worker check passes automatic creation, production routing,
  changed-content and cached deployments, public bytes/MIME, and confirmed cleanup.
  The configured Worker was not changed. Native Cloudflare target editing passes
  production defaults, advanced settings, save/reopen, and credential preservation.
- The full headless desktop library run has 656 passing tests, 17 ignored checks,
  and the already tracked embedded frontend JavaScript version mismatch. No
  frontend assets were changed in the Cloudflare port.

## Validation Notes (2026-09-26)

- Deployment URL checks pass, but the full desktop model gate has 641 passing
  tests, 14 ignored checks, and one failure:
  `snapshot::site::tests::frontend_resource_versions_match_the_rust_asset_helper`
  reports a version mismatch for the embedded frontend JavaScript. The same
  test fails with desktop features disabled. No frontend assets were changed
  during the deployment URL fix.
- Non-publishing Azure, Netlify, and Cloudflare access checks pass. Native Windows
  smoke was not run because another desktop instance was open; isolated GPUI
  deployment interaction tests pass.

## Validation Notes (2026-09-25)

- The embedded frontend resource-version gap is resolved by rebuilding the assets.
  The full desktop model gate passes with 627 tests and 13 optional checks ignored.
- Full frontend lint rules and strict checks for changed files pass. The combined
  `bun run lint` command still flags existing CRLF formatting in untouched files;
  those files were not reformatted as part of desktop publishing changes.
- Live landing publication passes on Azure and Cloudflare. The authorized Netlify
  account rejects new deployments with a credit-usage limit (HTTP 403), so live
  Netlify landing publication remains unverified until its credits are restored.

## Resolved In September 2026 Hardening Pass

- Initial page navigation now defaults to 60000 ms, matching the legacy
  Node/VS Code runner. Omitted or zero navigation timeouts use this bounded
  default; positive overrides and the 30000 ms element-wait default are preserved.
  The regression failed with the previous 30-second fallback and passes with 60.
  CDP `Page.navigate` now receives the same budget. A loopback browser regression
  failed at its old internal 30-second cutoff, then passed with a 31-second
  document response while preserving explicit short navigation timeouts.
- Compare Results now reloads the completed request's run and output folder in
  Results instead of retaining an older pinned all-pass report. Native-event
  regressions cover run and saved-report selections, custom output folders,
  shared run names across suites, intermediate capture failures, command errors,
  and cancellation.
- Pixel-color tolerance is now configurable through `pixelMatchThreshold` at
  workspace, suite, and scenario levels, including desktop Fields. The range is
  0-100, the default is 5, and desktop steps are 0.1. Comparisons normalize by 100
  and retain captured/reviewed thresholds. Background-color, inheritance, bounds,
  regeneration, legacy approval, and native-editing regressions cover the change.
- Resetting the last scenario field no longer leaves orphaned YAML indentation
  that would move a following suite property into the scenario. The lossless
  semantic guard remains intact; document and native-event regressions cover
  the starter suite's indentationless list and inherited threshold reset.
- URL waits now read CDP frame metadata instead of evaluating `location.href` in
  a JavaScript context that a redirect can destroy. A deterministic Chromium
  regression reproduces the original missing-context error and passes after the
  fix. Local fixtures cover delayed cross-origin redirects, selector readiness,
  SPA query/fragment changes, bounded timeouts, and frame-scoped URL checks.
- Git review now uses a side-by-side text comparison instead of schema-dependent
  settings/scenario grouping. Invalid YAML/JSON and ordinary UTF-8 files retain
  line-numbered, aligned red/green changes with shared scrolling, change navigation,
  and exact-source copy controls. Model and native-event tests cover malformed
  drafts, comments, line endings, limits, binary fallback, and compact/wide layouts.
- Corrected YAML/JSON files now save even if their previously saved source has
  syntax errors. Optional report-label comparison no longer rejects the repair;
  strict draft validation and external-conflict checks remain. Model regressions
  cover suites, workspace configuration, viewports, and data, and a native-event
  test verifies that saving clears the suite list's invalid status.
- Report and landing deployment targets now retain their last successful URL
  across tab switches and app restarts, with open/copy controls per target.
  Model and native-event regressions cover provider identity, failed publication,
  safe URLs, destination edits, disk reload, and compact light/dark layouts.
- Desktop Workspace files now lists every regular file under `common`, including
  `_on-ready.js`, `_override.css`, and nested, extensionless, or binary assets.
  Suite-like helper names stay separate from runnable suites. UTF-8 text files up
  to 4 MiB are editable regardless of extension; JavaScript, CSS, and other text
  use Source without configuration parsing. Binary, non-UTF-8, unreadable, and
  oversized files remain visible but disabled. Model and native-event regressions
  cover discovery, raw Save/Save As, undo/redo, external conflicts, size limits,
  mouse/keyboard guards, and sidebar scrolling in both themes/window sizes.
- Duplicate scenario-ID errors now identify the suite, source file, and both
  conflicting scenario entries, including numbers, labels, and URLs with
  only passwords redacted. Parser and multi-suite regressions cover
  URL-derived IDs, label slugs, case-insensitive collisions, and valid reused URLs.
- Desktop deployment now supports Cloudflare Workers Static Assets with secure
  target credentials, incremental uploads, and verified publication. A live check
  confirmed report bytes, suite deep links, headers, missing-file 404s, and reuse
  on an unchanged redeploy. The client translates the Netlify index-page rewrite
  to Cloudflare's canonical root and rejects Azure access policies before API calls.
- Cloudflare deployment creates missing Workers and publishes to production by
  default. Saved targets also expose private/preserve routing, HTML behavior,
  compatibility date, and exclusions. Uploads honor single-asset/concurrency hints,
  invalidate MIME changes, bound raw memory, and distinguish rejected publications
  from uncertain outcomes without retrying publication.
- [Azure release preparation](.ado/workflows/tuyen-release.yaml) waits for `vswhere`
  to finish before selecting MSBuild, avoiding a stale exit code that rejected a
  valid installation. Native Windows reproduction and [offline release tests](ci/test-release.py)
  cover successful, empty, and failed lookups without PATH updates on failure.
- Source now supports selected-line comments, indentation, duplication, movement,
  and deletion through focused shortcuts and an Edit menu. GUI regressions cover
  undo/redo, JSON comment rejection, read-only and busy guards, Unicode/CRLF, and
  reversed selections. Exclusive end-line indentation and Ctrl+Shift+Z routing
  no longer affect an unselected line or invoke the wrong document history.
- Test suite rows now expose Rename/Delete context actions and focus-scoped F2/Delete
  keys. Guarded operations update references, historical test runs, local report
  manifests, and desktop selections/activity across known output roots. Optional
  `displayName` is editable in Fields and Rename and labels desktop/web UIs without
  changing filenames or IDs. Model, native-event, and desktop/mobile browser
  regressions cover preservation, rollback, focus, escaping, and stable routes.
- Source suggestions now accept Tab as well as Enter, preserving normal indentation
  outside the menu. Window reactivation restores lost focus only to the same visible
  source document without displacing another control. GUI regressions cover acceptance,
  Escape, indentation, and restoration guards; native schema smoke verifies keyboard
  operation after Alt+Tab away and native reactivation without an editor click.
- Snapshot [Netlify headers](frontend/hosting/_headers) now include
  `Referrer-Policy: no-referrer` and `X-Content-Type-Options: nosniff`, matching
  [Azure](frontend/hosting/staticwebapp.config.json). Generated assets were rebuilt,
  and the [hosting regression](src/snapshot/site.rs) passes. Cache assertions check
  required directives independently of their order or additional shared-cache directives.
- Desktop suite-creation and sidebar-scroll test fixtures now account for the
  workspace files created automatically on connection. Suite creation verifies
  preservation of existing entries; scrolling covers seven, eight, nine, and
  twelve total files in light/dark compact/wide windows. Both regressions pass
  without changing runtime discovery or layout.
- Git diagnostics now redact full authorization diagnostics case-insensitively,
  including multiline values, before display or copying. Live output is bounded,
  buffers complete lines across read boundaries, and omits remaining output after
  sensitive/oversized diagnostics. Synthetic regression tests cover URL secrets,
  header casing, whitespace, chunk splitting, and output limits.
- Desktop now accepts blank and comment-only workspace YAML as defaults, matching
  the CLI without rewriting the source. Regressions cover first-field edits,
  resetting the last field, exact undo, and continued rejection of explicit nulls
  and invalid non-workspace documents.
- Desktop folder opening now creates missing runtime defaults automatically,
  including `regressify.yaml`, without generating example suites or external-editor
  assets. CLI `init` shares workspace-config creation. Focused regressions cover
  repeat/concurrent setup, alternate YAML formats, preservation, and failed opens.
- Results Fit area and Fit width now report the rendered zoom to one decimal
  place. GUI regressions cover image shapes, tabs, both window sizes, preview
  resizing, and manual zoom. Comparison fixtures now use the shared capture-owned
  error artwork accessor, resolving the stale includes that blocked GUI tests.
- URL-only scenarios now share friendly names across desktop catalogs, draft
  headings, Results, and snapshot navigation/capture headings. Complete-suite
  display names are recorded independently of IDs and filenames; older reports
  use the same URL fallback. Model and desktop/mobile browser regressions cover
  naming collisions, repeated viewports, and unchanged capture identity.
- Desktop lists now have focus-scoped keyboard selection and navigation. Suite
  and scenario Ctrl+A/ranges respect filters; run checklists and Git trees retain
  their selection boundaries. Viewport, capture, workspace-file, and action lists
  support keyboard navigation without intercepting text-editing shortcuts.
- New and Rename suite dialogs now focus and select the filename stem, accept
  Enter from inputs and focused Create/Save buttons, and include a close X.
  Keyboard Cancel/X remain non-destructive; GUI regressions cover dismissal,
  busy-state locks, invalid names, collision preservation, and retry.
- Snapshot overview, suite results, and image review now share a responsive React
  SPA with clean URLs, light/dark themes, and quick suite navigation. Generated
  JS, CSS, fonts, JSON, and every image use stable filenames with `?v=` versions.
  Snapshot generation preserves capture history; deployment packages only the
  active manifest and refreshes versions after compression without renaming files.
  Frontend, Rust, worker, and local browser tests cover it.
- Snapshot review now has independently scrolling panes, a resizable suite/scenario
  navigator scoped to the current suite, native page-only image scrolling, and a
  selectable Info popover. A searchable suite dropdown keeps the current suite
  visible and overlays scenarios instead of expanding the pane. Suite failure
  arrows jump between filtered failed cards; desktop/mobile browser checks cover
  navigation, keyboard selection, dismissal, resize, scrolling, and copying.
- Snapshot branding now shares the desktop icon for the logo/favicon. Suite and
  image browser titles include the suite name. Suite list rows show uncropped,
  side-by-side Reference/Test/optional Diff previews, with mobile, missing-image,
  and long-label browser coverage.
- Snapshot side-by-side review now synchronizes high-contrast pixel crosshairs
  across Reference/Test/Diff. Desktop/mobile browser checks cover unequal image
  sizes, zoom, missing images, pointer cleanup, and unchanged native scrolling.
- Snapshot themes and initial page backgrounds now apply before the app bundle
  loads, removing light/dark startup flashes. Preferences synchronize across tabs
  without startup writes; desktop/mobile regressions cover system changes, invalid
  or blocked storage, and preference resets.
- Snapshot footers now stay at the viewport bottom on short, empty, and missing
  pages and follow long content in normal flow. Desktop/mobile footer regressions
  cover both document and review-pane scrolling without overlap.
- Desktop Snapshot and Deploy no longer pass built-in report text as explicit
  CLI overrides. Workspace snapshot title, heading, and Markdown subheading now
  reach staged Netlify/Azure reports. Legacy activity's prefilled defaults migrate
  to inherited values; custom and newly saved explicit overrides are preserved.
- Desktop update checks now run in the background for every newly opened instance,
  even inside a recent check's eight-hour window, then follow the shared eight-hour
  schedule while windows stay open. One check/download owner per user cache
  prevents concurrent work; startup preserves pauses and checksum retry limits.
  Every instance verifies the shared installer before showing its banner.
  Model and GUI tests cover startup, scheduling, ownership, cached reuse, pause,
  and shutdown takeover.
- Deployment staging no longer uploads unused TTF fonts. Exact-ZIP regression
  coverage confirms unreferenced old/current-run captures, diffs, source maps,
  metadata, and unselected suites are excluded without modifying source files.
- Native smoke shortcuts use typed `SendInput` events with balanced modifiers and
  extended navigation flags. The complete default authoring and `-SchemaFlows`
  checks pass, including completion, clipboard operations, saves, and preference
  restoration; [headless checks](ci/test-desktop-smoke.ps1) cover key-event generation.
- Netlify uses asynchronous digest manifests and uploads only missing content, with four concurrent file requests and bounded per-file retries. The one-hour deadline and periodic stall warnings remain; deployment creation is never automatically retried. Live publication and unchanged-content reuse were verified with a synthetic report.
- Netlify site validation and status reads now retry interrupted or timed-out response bodies after successful headers instead of exiting with `Cannot read Netlify response`. Loopback tests verify recovery, six-attempt exhaustion, and redacted timeout/interruption diagnostics; malformed or oversized responses still fail immediately.
- Results approval confirms inline for the current scenario/viewport, refreshes suite failure counts from the recalculated report, and stays in Results; whole-run approval remains in Runs.
- Scenario `selectors` supports full-page defaults, `viewport`, and visible CSS selector clips with stable target-specific filenames and report ordering.
- `asyncCompareLimit` is now a validated bounded worker limit for parallel pixel comparison.
- Report regeneration ignores generated diffs and clears stale diff artifacts when a pair becomes a pass.
- Screenshot output writes atomically, preserves color-management PNG chunks, and uses collision-resistant run IDs.
- Capture waits for an interactive DOM, lazy-loaded resources, decoded fonts/images, and stable layout. Configurable animation stabilization now runs before actions and at each checkpoint/final capture. `save` shares the final capture's resolved delay and bounded settling without replaying lazy-load scrolling. Local Chromium regressions cover saved pixels, delay inheritance/overrides, repeated checkpoints, invalid names, animation opt-outs, late-started animations, and preserved hover state.
- Text caret suppression no longer depends on `freezeAnimations`. Checkpoints and final captures hide blinking carets without changing focus, selection, or input values, including when settling is disabled. A local Chromium regression verifies identical screenshot pixels across a blink interval for inputs, textareas, and editable content with animation freezing on and off.
- Lazy-load scrolling now finishes before prerequisite, scenario, and legacy actions instead of disturbing their final hover state. It bypasses smooth scrolling and waits for stable offsets at the top before continuing, including scenarios without actions. Local Chromium regressions cover all action paths, `noScrollTop`, disabled capture settling, delayed/interrupted scrolling, and bounded failure when returning to the top is blocked.
- Interactive actions use full actionability validation, URL replacements apply to `goto`, and iframe waits/coordinates are hardened.
- URL waits support exact URLs, single-segment `*`, multi-segment `**`, and query-value wildcards.

## Resolved In June 2026 Fix Pass

- `press` now uses CDP `Input.dispatchKeyEvent` and supports modifier combinations like `Ctrl+A`, `Shift+Tab`, and `Meta+K`.
- Replacement profiles are loaded, validated, and applied during `test`; invalid regexes fail early as configuration errors.
- Resolved `hideSelectors`/`removeSelectors` are applied through the config cascade.
- `cookiePath` and persisted state paths are confined to the project/output directories; state respects `--site-env`.
- Unknown viewport names, duplicate viewport labels, empty labels, and zero dimensions fail before browser launch.
- Visual mismatches and website/action failures are report results, not process failures.
- `.tests.yml` and `.tests.json` files are parsed from their discovered paths instead of being incorrectly remapped to `.tests.yaml`.
- `needs` dependencies now fail early as configuration errors when missing, duplicated, cyclic, or deeper than 100 levels.
- Duplicate suite formats are deduplicated by suite name with `.tests.yaml` > `.tests.yml` > `.tests.json` precedence.
- `postInteractionWait` defaults to `1000` milliseconds through the workspace/suite/scenario cascade.
- `wait.url` uses active URL replacements in `test` mode; `ref` captures and waits against raw URLs.
- `restore`, `requiredLogin` / `--requiredLogin`, `init`, `install`, and `version` are implemented in the Rust CLI where they fit the Chromium/CDP design.
