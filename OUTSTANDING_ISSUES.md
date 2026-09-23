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

### P2: Process Pipe Joins Escape Timeout and Cancellation

- Evidence: the [Git wait loop](src/desktop/git/process.rs#L122) stops checking its
  deadline/cancellation when the parent exits, then unconditionally joins stdin,
  stdout, and stderr threads while still holding the child group. The
  [desktop worker](src/desktop/runner.rs#L693) has the same reader-join boundary.
- Trigger: a Git hook/helper or engine descendant outlives its parent and retains
  an inherited pipe. The reader waits for EOF; the group handle is not released,
  and subsequent cancellation no longer reaches the finished parent wait loop.
  Git can remain busy beyond its advertised deadline, and worker close can hang.
- Verification: static control-flow review plus the pinned command-group 5.0.1
  parent `try_wait` behavior. Existing cancellation tests keep the parent alive;
  the parent-exits-first case was not executed during this review.
- Repair/test: bound writer/reader completion and preserve cancellation/process-tree
  cleanup until all streams close. Add a fixture whose parent exits while a child
  retains stdout/stderr; assert bounded completion, cancellation, and no descendants.

### P2: Authorization Header Values Survive Git Error Redaction

- Evidence: [redact](src/desktop/git/process.rs#L167) splits on whitespace and
  replaces only words containing a URL or the exact text `Authorization:`.
  The following `Bearer`/`Basic` value remains in returned/displayed Git stderr.
- Trigger: a Git hook, helper, or diagnostic emits an authorization header. Lowercase
  header names and values on following tokens are not protected by the current rule.
- Verification: compiled the actual redactor with a synthetic input;
  `Authorization: Bearer synthetic-review-token` retained the synthetic token.
  No real credential or remote response was accessed.
- Repair/test: redact full header values case-insensitively and bound line parsing;
  cover Bearer/Basic, mixed case, whitespace, multiline errors, URLs, and surrounding
  safe text in the existing process tests.

## Review Checks (2026-09-23)

- `cargo test --features desktop --lib`: 516 passed, 9 ignored, no failures.
- Ignored Chromium/Node fixtures, GUI interaction, installer execution, and live
  provider/update services were not run. Test success does not resolve the gaps above.

## Resolved In September 2026 Hardening Pass

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
