# Outstanding Issues

Issues identified during adversarial review that remain after the September 2026 hardening pass.

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

- Desktop deployment now supports Cloudflare Workers Static Assets with secure
  target credentials, incremental uploads, and verified publication. A live check
  confirmed report bytes, suite deep links, headers, missing-file 404s, and reuse
  on an unchanged redeploy. The client translates the Netlify index-page rewrite
  to Cloudflare's canonical root and rejects Azure access policies before API calls.
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
- Snapshot overview, suite results, and image review now share a responsive React
  SPA with clean URLs, light/dark themes, and quick suite navigation. Generated
  JS, CSS, fonts, JSON, and every image use stable filenames with `?v=` versions.
  Snapshot generation preserves capture history; deployment packages only the
  active manifest and refreshes versions after compression without renaming files.
  Frontend, Rust, worker, and local browser tests cover it.
- Snapshot review now has independently scrolling panes, a resizable suite/scenario
  navigator scoped to the current suite, native page-only image scrolling, and a
  selectable Info popover. Suite failure arrows jump between filtered failed cards;
  desktop/mobile browser checks cover navigation, resize, scrolling, and copying.
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
- Capture waits for an interactive DOM, lazy-loaded resources, decoded fonts/images, and stable layout; capture-only animation freezing is configurable.
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
