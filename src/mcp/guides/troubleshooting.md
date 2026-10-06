# Diagnose Regressify Failures

Use this guide with the user's authorized local file, terminal, and browser tools.
The MCP server cannot inspect a workspace, receive logs for analysis, execute a
capture, or change baselines. Keep credentials, cookies, saved state, private
source, screenshots, and raw logs local. Do not send them in MCP tool arguments.

## Establish the Failing Operation

1. Compare `regressify version` with the version in MCP responses. Inspect the
   installed command's `--help` before assuming a flag or field is available.
1. Record the exact command, workspace root, output root/environment, suite,
   scenario ID, viewport, and run ID. Preserve the original CLI error and report.
1. Identify the stage: preflight/configuration, browser startup/navigation,
   state/hooks, actions, capture, comparison, or report generation. A categorical
   cause alone is insufficient: an invalid zero timeout can be a preflight error,
   not a browser that took too long.
1. Read the applicable topic/schema and inspect the actual local inputs. Validate
   names, paths, inheritance, and conditional action fields before changing code.

Exit zero is not proof that every capture passed. Website/action failures and
visual mismatches are report results; configuration, infrastructure, and artifact
write failures fail the process. A failing scenario stops without stopping its
peers. Missing diagnostics do not establish either a successful or failed capture.

## Read Process Errors

A nonzero exit prints a final line beginning `ERROR:` with the full message
chain; that line is authoritative. Earlier console records carry only a coarse
keyword category, which can mislead: an unknown-field error may show
`cause=Timeout` because its expected-field list mentions `readyTimeout`.
Typical preflight messages and corrections:

- `data did not match any variant of untagged enum TestAction`, with a line and
  column: the action item starting there has a misspelled or unsupported key, a
  `type` or `action` wrapper, two action keys, a wrong value type, or a null
  value from an unquoted `#` selector. Compare it with
  `get_schema(kind="action", action=...)`.
- `unknown field ..., expected one of ...`: a misspelled or wrong-scope key.
- `Unknown viewport name(s)` or `Invalid viewportNames in test suite`: use
  labels from the effective viewport file and the consuming scenario's set.
- `must contain exactly one scenario with id`: fix the `needs` reference.
- `Duplicate scenario ID` or `colliding screenshot filename`: make scenario
  IDs, save names, and selector targets distinct.
- `Invalid selectors for scenario`: an invalid `selector`, `selectors`, or save
  name.
- `Selected scenario ... must match exactly one scenario` or
  `No matching test suites found`: check suite names and file discovery.
- `Capture run directory already exists`: choose a new `--run-id`.
- `Node hooks require Node.js and a workspace installation of playwright`:
  install the dependency with permission, or remove the hook.
- Missing, escaping, or malformed `cookiePath`, `cssOverridePath`,
  `jsOnReadyPath`, state, or Basic Auth values: repair the referenced local
  file or environment variable without printing secrets.

## Reproduce Narrowly

Only run captures against authorized pages. Replace the example suite/scenario
IDs with real ones, keep the same `--root`, `--site-env`, or `--regressify-dir` as
the failed operation, and choose a fresh run ID to avoid confusing old artifacts.
Global flags precede the command:

```sh
regressify --log-level debug --run-id diagnose-checkout-001 test --scenario shop:checkout --fetch-browser false
```

`--fetch-browser false` requires cached managed Chromium; it prevents downloads,
not browser execution. `regressify install` may download Chromium and needs user
authorization. `--debug` can show Chromium outside CI. There is no dedicated
validation-only command: a valid `test` command proceeds to browser capture.
`--skip-compare` captures without comparison when isolating capture behavior.

Every capture command creates a new run directory and refuses an existing one,
so use a new `--run-id` for each attempt. Without `--run-id`, capture runs are
named by UTC time (`YYYYMMDD_HHMMSS_nnnnnnnnn`), and `compare` or `approve`
picks the lexicographically greatest run directory; custom IDs that start with
a letter sort after every timestamp. Pass `--run-id` to target a reviewed run.

Capture commands select a suite by logical filename and an exact scenario ID.
Compare/approve scenario selections instead use the recorded suite output ID,
which may differ from its filename. Recompute a reviewed run's comparisons with:

```sh
regressify --run-id diagnose-checkout-001 compare --scenario shop-output:checkout
regressify snapshot
```

`compare` changes reports/diff images but does not recapture pages; `snapshot`
builds report assets from existing comparisons, which `test` and `ref` already
do. Neither repairs stale screenshot pixels, and `compare` reuses thresholds
recorded at capture time. `ref` and `approve` change baselines and require
explicit authorization; never use them as an automatic repair for a mismatch.

## Find Reports and Evidence

`<output>` defaults to `.regressify/`; `--site-env staging` selects
`.regressify__staging/`, and `--regressify-dir` overrides that root. Inspect the
actual suite output ID and run ID instead of guessing from a display name.

- `<output>/<suite-id>/bitmaps_test/<run-id>/`: every capture of that run,
  including `ref` runs, plus `report.json`, `_diff.png` images,
  `.png.error.json` failure markers, `.log.json` links, a `.diagnostics.json`
  marker, and `.report-*.json` manifests of expected captures, thresholds, and
  URLs. Do not treat an error placeholder as a good image.
- `<output>/<suite-id>/bitmaps_reference/`: approved baselines and their log
  links. Preserve these during diagnosis.
- `<output>/<suite-id>/html_report/config.js`: the latest report data.
- `<output>/_diagnostics/<operation-id>/`: `index.json` and
  `segment-NNNNNN.jsonl` journals, one operation per capture job or command.
- `<output>/index.html` and `<output>/_snapshot/`: the report website. Serve the
  output folder over HTTP to view it; opening the file directly does not work.

Capture files are named `<scenario-id>[_<viewport>]__<target>.png`, where
`<target>` is `FINAL` (full page or singular `selector`), `VIEWPORT`,
`$<save-name>`, or `SELECTOR-<slug>-<hash>` for each `selectors` entry.
Scenarios without `id` use a slug of `label`, or a 20-character hash of `url`.

Each `report.json` `tests[]` entry has `status` (`pass` or `fail`) and a `pair`
with `fileName`, `scenarioId`, `selector` (`document`, `viewport`,
`save:<name>`, or a selector capture name), `url`, `referenceUrl`,
`misMatchThreshold`, `pixelMatchThreshold`, `comparisonOutcome` (`success`,
`mismatch`, or `failure`), optional `diff` (`misMatchPercentage`,
`isSameDimensions`, `dimensionDifference`), `engineErrorMsg`, and
`referenceLog`/`testLog` links. Common `engineErrorMsg` values:

- `Reference image not found`: no baseline has this capture name, because the
  scenario is new, an ID, save, or viewport was renamed, or its reference
  capture failed.
- `Dimensions differ: reference WxH vs test WxH`: always a failure, whatever
  the thresholds; usually the page height changed. Inspect the diff first.
- `Test scenario failed to capture: <Category> failure (causes: [...])`, or the
  `Reference` and `Both` variants: a page-level failure left a placeholder. In a
  `ref` run the message says `Test` because that run's capture failed; the
  placeholder is never promoted.
- `Comparison error: ...`: an image could not be read or compared.

A visual mismatch has `comparisonOutcome` `mismatch`, with
`misMatchPercentage` above `misMatchThreshold`. In diff images, magenta pixels
mark differences over a dimmed copy of the reference.

## Read Capture Records

Failure markers and `engineErrorMsg` keep only an error category (`Timeout`,
`Actionability`, `Navigation`, `NotFound`, `InvalidData`, `Permission`,
`Storage`, or `Unknown`), never the selector or page text. Find the failing
step in the console records or the capture journal. A console record looks like:

```text
[5345ms 64e6b72f-...] [Error] ActionsFrames: Stage finished ActionIndex=1 ActionType=Click Target=sha256:bb62... Condition=Exists FrameDepth=0 outcome=Failure duration=2080ms cause=Timeout failure (causes: [Timeout])
```

Journal JSON lines carry the same `stage`, `eventName`, `fields`, `outcome`, and
`errorCause` in camelCase, plus an `execution` block with suite, scenario, and
viewport identities, viewport size, and `captureRole`. Useful fields:

- `ActionIndex`: 1-based position among actions executed in that capture job,
  counting inherited `needs` actions first; viewport-skipped actions are not
  counted. `ActionType` names the action; `Navigate` is `goto`.
- `Condition` on a failed action: `Exists` means nothing matched within
  `elementTimeout` (wrong selector, frame, tab, or page, or content not loaded);
  `Visible` means zero size, hidden, `display: none`, or opacity 0, including
  elements hidden by your own `hideSelectors` or `removeSelectors`; `Stable`
  means still moving; `Unobscured` means another element, such as a consent
  banner, sticky header, or modal, covers its center; `Enabled` means
  disabled, `aria-disabled`, or inert.
- `Wait` records have no `Condition`. `Actionability` means its selector stayed
  hidden; `Timeout` means the selector never appeared or the `url` never
  matched. A numeric wait has no `Target`, so its failure is the URL.
- Stage `Navigation` with `Destination` and `TimeoutMs` covers initial
  navigation and `goto`. A failure right after successful initial navigation,
  before any `StateHooks` record with `State=Ready`, is the `readySelector`.
- Stage `StateHooks` with `ActionType=Hook`: `State=Ready` applies CSS and the
  ready script; with `TimeoutMs` it is a Node hook. `ActionType=Restore` loads
  saved state, or the cookie file when an `Artifact` field is present. Stage
  `ScreenshotSettling` with `Condition=LazyLoading` is the 20-second lazy-load
  scroll. Stage `CaptureArtifacts` with `Capture` is a screenshot target, often
  a missing or hidden screenshot selector.
- When the last started stage never finishes and the cause is `Timeout`, the
  whole `scenarioTimeout` expired.

`Target`, `Destination`, `Capture`, and execution identities are `sha256:` plus
the hex SHA-256 of the exact UTF-8 text: the selector as written, the navigated
URL after replacements, the capture file name without `.png`, the suite or
scenario ID, or the viewport label (`Default` without one). To confirm a
suspect, hash the local candidate and compare; never try to recover unknown
values.

Default console output shows `info` and above; `--log-level debug` adds skipped
actions and settling details. `--log-level trace|debug|info|warn|error`,
`--log-verbose`, and `REGRESSIFY_LOG_LEVEL` change display, not archived
collection. `--no-print` suppresses diagnostic console records. A capture's
`.log.json` names its journal in `archive.operationId`; open
`_diagnostics/<operation-id>/index.json` for `state`, `records`, and
`missingRecords`. Archives can be complete, partial, missing, legacy
unavailable, or unreadable. Missing logs after a crash, disabled archive,
disk/queue limit, or corrupt sidecar are not an empty successful timeline.
Inspect warnings and `REGRESSIFY_DISABLE_DIAGNOSTIC_ARCHIVE`; do not delete
evidence to hide a warning. Disk/chunk limits are operation settings, not
fields to add to suite YAML.

In the desktop app, use Results/Capture Log for image execution and View Log for
the saved command operation. The snapshot report exposes referenced sanitized
capture logs. Startup failures may instead have journals under the user's local
application-data directory at `regressify/diagnostics/startup/_diagnostics`.
Capture journals are not browser console logs or network-body recordings. When
the category is not enough, reproduce with `--debug` and browser developer
tools, with authorization.

## Common Failures

- Element not found or not actionable: confirm the URL, tab, frame, and
  selector against the live page, then act on the `Condition`. Wait for the URL
  and a destination element after navigation, add `switchTab: new` after links
  that open tabs, set `frame` for iframes, dismiss or remove covering overlays,
  click the visible label of a visually hidden native checkbox, and use
  `useFileChooser` for hidden file inputs. Do not blindly raise timeouts.
- Screenshot selector failure: the `CaptureArtifacts` target is missing,
  hidden, zero-sized, inside an iframe or shadow root, or written with `text=`
  or another non-CSS engine.
- Navigation or URL wait timeout: check the authorized URL, replacements,
  redirects, TLS, and `waitUntil`. A URL wait does not select a popup; end the
  pattern with `*` when a query string may follow; `networkidle` may never
  occur on continuously active pages.
- Missing state, 401, or unexpected login page: verify the active output
  environment, state names, cookie attributes, and Basic Auth origins and
  placeholders locally. State must exist before capture preflight; `needs` and
  `requiredLogin` do not create a login session.
- Browser ready script or CSS failure: a `StateHooks` record with `State=Ready`
  failed. Check for JavaScript exceptions and the `elementTimeout` bound, and
  try the script on the page in browser developer tools.
- Node module/export error or hook timeout: read the hooks topic. Check
  workspace dependencies, `.mjs`/`.cjs`, the six-argument default export,
  `elementTimeout`, and lifecycle order. `onReadyScript` runs after actions and
  cannot repair earlier save checkpoints.
- Invalid timeout: `elementTimeout`, `actionabilityTimeout`, and
  `scenarioTimeout` must be positive whole milliseconds. `settleTimeout: 0`
  disables settling; omitted/zero `readyTimeout` inherits element timeout;
  omitted/zero navigation timeout uses 60000 ms.
- Visual mismatch with successful capture: compare reference, test, and diff at
  the same viewport, and verify baseline age, page data, fonts, rendering, and
  intended changes. Distinguish the allowed mismatch percentage from per-pixel
  color tolerance; do not mask important content or raise thresholds to hide a
  real regression.
- Mismatches that vary between runs: stabilize time-based, random,
  personalized, animated, or lazily loaded content with `hideSelectors`,
  `removeSelectors`, explicit waits, `settleTimeout`, or `delay`. Captures
  already freeze animations and hide text carets.
- No captures or unexpected capture count: check scenario/viewport filters,
  inherited action filters, named saves, and singular versus plural selectors.
  `selectors: []` is invalid; omitted/null means automatic full-page capture. A
  last applicable save suppresses only the automatic final capture.
- Missing dependency or capture-name collision: check `needs` IDs and cycles
  and every scenario/viewport/save/selector name. Names must remain portable and
  distinct after normalization. Do not repair collisions by deleting baselines.
- Missing file or path escapes workspace: resolve configured paths from the
  workspace root; upload paths start at `visual_tests/`. Check regular-file
  status, symlinks, and `.yaml`/`.yml` precedence. Do not disable containment.
- Chromium preparation failure: `--fetch-browser false` needs the cached build,
  which an authorized `regressify install` prepares. Proxies and firewalls must
  allow the download; `--proxy` applies to it.
- Artifact permission, disk, or unsafe-path error: preserve good outputs and
  check the failing path, free space, locks, and permissions. Initialization
  can fail on malformed/read-only MCP or agent files; repair the draft or access
  issue, not unrelated project data.

## MCP Connection Problems

The endpoint is an HTTP MCP handler ending in `/mcp`, not a browser-facing HTML
page. Confirm the configured URL/type and reconnect the client after a service
upgrade to refresh discovery. Compare server/CLI versions. For self-hosted
services, check proxy/TLS, credentials, allowed origins, and the provider's local
diagnostics; do not disable authentication or Origin checks as a workaround.

CLI init and desktop folder opening manage `regressify-mcp` in `.vscode/mcp.json`
and the `## Regressify CMP` section in `AGENTS.md`. They refresh the managed
endpoint/instructions while preserving unrelated content. Older installed
versions may lack automatic desktop setup. Explain a missing or outdated MCP
service to the user instead of inventing undocumented configuration.
