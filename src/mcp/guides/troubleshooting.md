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

Capture commands select a suite by logical filename and an exact scenario ID.
Compare/approve scenario selections instead use the recorded suite output ID,
which may differ from its filename. Recompute a reviewed run's comparisons with:

```sh
regressify --run-id diagnose-checkout-001 compare --scenario shop-output:checkout
regressify snapshot
```

`compare` changes reports/diff images but does not recapture pages; `snapshot`
builds report assets from existing comparisons. Neither repairs stale screenshot
pixels. `ref` and `approve` change baselines and require explicit authorization;
never use them as an automatic repair for a mismatch.

## Find Reports and Evidence

`<output>` defaults to `.regressify/`; `--site-env staging` selects
`.regressify__staging/`, and `--regressify-dir` overrides that root. Inspect the
actual suite output ID and run ID instead of guessing from a display name.

| Location                                                             | Evidence                                                                                                       |
| -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `<output>/<suite-id>/tests/<run-id>/`                                | Raw captured screenshots.                                                                                      |
| `<output>/<suite-id>/bitmaps_test/<run-id>/report.json`              | Comparison results for this run; inspect the associated reference/test/diff images and capture errors.         |
| `<output>/<suite-id>/bitmaps_reference/`                             | Approved/reference images; preserve these during diagnosis.                                                    |
| Image-adjacent `.png.error.json` files                               | Capture error markers. Do not treat an error placeholder as a good reference.                                  |
| Image-adjacent `.log.json` links and run `.diagnostics.json` markers | Links to the original capture execution, not necessarily the latest command.                                   |
| `<output>/_diagnostics/<operation-id>/index.json`                    | Local command/capture journal metadata and referenced JSON-line segments. Follow the index and recorded links. |

In the desktop app, use Results/Capture Log for image execution and View Log for
the saved command operation. The snapshot report exposes referenced sanitized
capture logs. Startup failures may instead have journals under the user's local
application-data directory at `regressify/diagnostics/startup/_diagnostics`.

Use stage, outcome, action index, and execution identity to correlate records
with local scenarios/actions. Some names, URLs, and selectors are opaque hashes;
do not attempt to reverse them. Capture journals are not browser console logs or
network-body recordings. Use separately authorized browser tools for those.

`--log-level trace|debug|info|warn|error`, `--log-verbose`, and `REGRESSIFY_LOG_LEVEL`
change display, not archived collection. `--no-print` suppresses diagnostic
console records. Archives can be complete, partial, missing, legacy unavailable,
or unreadable. Missing logs after a crash, disabled archive, disk/queue limit,
or corrupt sidecar are not an empty successful timeline. Inspect warnings and
`REGRESSIFY_DISABLE_DIAGNOSTIC_ARCHIVE`; do not delete evidence to hide a warning.
Disk/chunk limits are operation settings, not fields to add to suite YAML.

## Common Failures

| Symptom                                         | Check and Correct                                                                                                                                                                                                                                             |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unknown field or untagged action parse error    | Use camelCase and the correct scope/schema. Quote `#` selectors and text values. Use one action discriminator; `input` needs text or files, `select` needs value or label. Do not add a `type` wrapper.                                                       |
| Invalid timeout                                 | `elementTimeout`, `actionabilityTimeout`, and `scenarioTimeout` must be positive whole milliseconds. `settleTimeout: 0` disables settling; omitted/zero `readyTimeout` inherits element timeout; omitted/zero navigation timeout uses 60000 ms.               |
| Unknown suite, scenario, or viewport            | Check file suffix/discovery precedence, exact scenario IDs, suite file name versus output ID, configured viewport file, and case-insensitive labels. `viewportNames` uses labels, not CSS or the special screenshot value `viewport`.                         |
| No captures or unexpected capture count         | Check scenario/viewport filters, inherited action filters, named saves, and singular versus plural selectors. `selectors: []` is invalid; omitted/null means automatic full-page capture. A last applicable save suppresses the automatic final capture only. |
| Missing dependency or capture-name collision    | Check `needs` IDs/cycles and every scenario/viewport/save/selector name. Names must remain portable and distinct after normalization. Do not repair identity collisions by deleting baselines.                                                                |
| Missing file or path escapes workspace          | Resolve configured paths from workspace root; upload paths start at `visual_tests/`. Check regular-file status, symlinks, and `.yaml`/`.yml` precedence. Do not disable containment checks.                                                                   |
| Missing state, 401, or unexpected login page    | Verify the active output environment, state names, cookie attributes, and Basic Auth origins/placeholders locally. State must exist before capture preflight; `needs` and `requiredLogin` do not automatically create a login session.                        |
| Element not found or not actionable             | Confirm the current URL/tab/frame and selector. Check visible/enabled state, overlays, animation, and lazy loading. Use state-based waits and explicit `switchTab`; do not blindly increase all timeouts.                                                     |
| Navigation or URL wait timeout                  | Check authorized URL, replacements, redirects, TLS, and the intended `waitUntil` checkpoint. A URL wait does not select a popup; `networkidle` may not fit a continuously active application.                                                                 |
| Node module/export error or hook timeout        | Read the hooks topic. Check workspace dependencies, `.mjs`/`.cjs`, the six-argument function export, element timeout, and lifecycle order. `onReadyScript` runs after actions and cannot repair earlier save checkpoints.                                     |
| Visual mismatch with successful capture         | Compare reference/test/diff at the same viewport and verify the baseline, page data, fonts, rendering, and intended change. Distinguish allowed mismatch percentage from per-pixel color tolerance; do not mask important content.                            |
| Artifact permission, disk, or unsafe-path error | Preserve good outputs and check the failing path, free space, locks, and permissions. Initialization can fail on malformed/read-only MCP or agent files; repair the draft or access issue, not unrelated project data.                                        |

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
