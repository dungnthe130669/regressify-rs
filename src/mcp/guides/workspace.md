# Workspace Configuration

The workspace root contains `regressify.yaml` or `regressify.yml`, a YAML object
with optional fields. Prefer one file; if both exist, the CLI loads `.yaml`.
Use `{}` to inherit defaults. Do not create a `workspace:` or `config:` wrapper.
Use `get_schema(kind="workspace")` for the exact field set, descriptions,
accepted types, and constraints. A JSON-looking value may be valid YAML, but a
file named `regressify.json` is not automatically discovered as workspace config.

## Precedence and Exceptions

For fields supported at each level, values resolve scenario, then suite, then
workspace, then built-in defaults. Omitted/null optional values inherit. Explicit
`false` and allowed zero values are not treated as missing. `hideSelectors`,
`removeSelectors`, and viewport selections replace rather than append lists.
An empty viewport selection means all configured viewports, not no viewports.

Do not assume every field exists at every scope. Examples:

- `delay`, `noScrollTop`, and `requiredLogin` have workspace/scenario settings,
  not suite settings. The global `--required-login` flag can force the login flag.
- `freezeAnimations`, element/actionability/settle/scenario timeouts, and report
  title/heading/subheading settings are workspace-only.
- `viewportsPath` exists at workspace and suite scopes; the scenario chooses
  labels through `viewportNames` instead.
- `selector`, `selectors`, and `save.selector` are separate capture options, not
  inherited workspace configuration.
- `basicAuth` entries for the same origin override inherited credentials; other
  origins remain inherited. Origin matching prevents sending credentials to an
  unrelated redirect or a proxy.
- URL replacements concatenate in this order: profile, then workspace, then suite.
  They are not an ordinary replacing list and are not a scenario-level setting.
- Suite `state` replaces workspace `state`; scenario `restore` adds names to that
  selected list in order, without duplicates.
- `logLevel` is overridden by CLI verbosity flags, then `REGRESSIFY_LOG_LEVEL`,
  before the file cascade. It changes display only, not archived collection.
- Snapshot command title/heading/subheading options override the corresponding
  workspace report text. Suite/scenario overrides do not apply to report text.

## Units and Defaults

| Setting                                                                 | Default and Meaning                                                     |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `browser`                                                               | `chromium`; Firefox and WebKit are unsupported.                         |
| `asyncCaptureLimit`                                                     | 5; positive whole number, also bounded by the CLI's global capture cap. |
| `asyncCompareLimit`                                                     | 2; positive whole number.                                               |
| `misMatchThreshold`                                                     | 0.1; allowed percentage of differing pixels, from 0 to 100.             |
| `pixelMatchThreshold`                                                   | 5; per-pixel color tolerance on a separate 0 to 100 scale.              |
| `delay`                                                                 | 1000 milliseconds before capture; non-negative.                         |
| `postInteractionWait`                                                   | 1000 milliseconds; non-negative.                                        |
| `elementTimeout`                                                        | 30000 milliseconds for element existence waits.                         |
| `actionabilityTimeout`                                                  | 5000 milliseconds for interaction checks.                               |
| `settleTimeout`                                                         | 5000 milliseconds; zero disables screenshot settling.                   |
| `scenarioTimeout`                                                       | 600000 milliseconds for a complete scenario/viewport capture.           |
| `freezeAnimations`                                                      | true before capture.                                                    |
| `useCssOverride`                                                        | true; applies the configured CSS override file.                         |
| `debug`, `bypassCsp`, `ignoreSslErrors`, `noScrollTop`, `requiredLogin` | false. Headed debug mode is disabled in CI without a display.           |

Thresholds and timing values must be finite; do not use negative delays or zero
concurrency. `elementTimeout`, `actionabilityTimeout`, and `scenarioTimeout` must
be positive whole milliseconds; zero is invalid. `elementTimeout` also bounds
document and Node hooks. `settleTimeout: 0` disables settling, not all waits or
animation freezing. A scenario's initial navigation timeout lives under
`engineOptions.gotoParameters.timeout`; omitted or zero means 60000 milliseconds,
independent of `elementTimeout`. `waitUntil` is `commit`, `domcontentloaded`,
`load` (default), or `networkidle`. The overall scenario timeout still applies.
Omitted/zero scenario `readyTimeout` inherits `elementTimeout`. Use the hooks
topic for script timing and troubleshooting for interpreting timeout failures.

## Files, Replacements, and Credentials

`cookiePath`, `cssOverridePath`, `jsOnReadyPath`, and `viewportsPath` resolve from
the workspace root, not from the suite's folder. Explicit configured paths must
exist as regular files inside the canonical workspace, including after symlink
resolution. An absolute path is allowed only if it remains inside that workspace.
Missing optional defaults differ from explicit missing paths; consult the files
guide before assuming a default file is required.

`ref` captures raw reference URLs. `test` applies URL replacement rules in order,
including supported navigation and URL-wait targets. Each rule has `ref` and
`test` strings; `regex: true` enables Rust regular expressions. Optional flags
are `g`, `i`, `m`, `s`, and `u`; invalid/repeated flags or flags without regex fail.
Do not assume JavaScript-only regex features such as lookaround work.
`--replacement-profile` overrides `REPLACEMENT_PROFILE`, otherwise `default` is
selected from the replacement-profile file. Invalid selected profiles fail.

For HTTP Basic Auth, supply an object or array of objects with `origin`,
`username`, and `password`. Use `${VARIABLE}` or `$VARIABLE` placeholders, resolved
locally at execution. Missing/empty environment values fail validation. Never
commit literal passwords or send resolved values to the MCP server. Cookies,
persisted state, and Basic Auth are separate mechanisms; none is an automatic
application-specific login workflow.
