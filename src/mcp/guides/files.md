# Predefined and Generated Files

Paths below are relative to the user's Regressify workspace, not the MCP Worker
package or this service's source. CLI init and desktop folder opening preserve
existing runtime defaults and add missing ones. Both also create or refresh the
managed MCP configuration and agent section described below. Only explicit CLI
`regressify init` adds an example suite, exported schemas, and other VS Code
settings/extension recommendations; existing copies of those files are preserved.

## Managed Agent Files

- `.vscode/mcp.json`: adds `servers.regressify-mcp` when missing, or updates only
  its URL to the installed CLI's bundled endpoint. Other servers, fields, input
  definitions, JSONC comments, and trailing commas are preserved. A current URL
  is not rewritten. A custom URL under this managed server name is refreshed too.
- `AGENTS.md`: creates the file, appends a missing `## Regressify CMP` section,
  or updates outdated instructions through the next top-level H2 or EOF.
  Other sections remain unchanged. Markdown parsing ignores quoted/code examples
  and preserves already-current content despite equivalent heading/list syntax.

Do not place unrelated instructions inside the managed section; its entire H2
range is maintained. Duplicate managed sections are consolidated. Malformed MCP
JSONC, unsafe paths, unreadable files, or read-only files needing an update fail
setup without replacing the affected file. An unfinished Markdown block that
would swallow an appended heading also fails unchanged. Older installed versions
may lack automatic desktop setup; compare CLI and server versions when diagnosing.

## User-Authored Runtime Files

| Path                                                             | Contents and Loading                                                                                               |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `regressify.yaml` or `regressify.yml`                            | Workspace YAML object; `{}` inherits defaults. Existing `.yml` is preserved by initialization.                     |
| `visual_tests/<name>.tests.yaml`, `.tests.yml`, or `.tests.json` | Suite object containing `scenarios`. Use one format per suite name.                                                |
| `common/_viewports.yaml`                                         | Top-level list of `{label, width, height}` objects. Default viewport file; override with `viewportsPath`.          |
| `common/_cookies.yaml`                                           | Top-level cookie list; initialized as `[]`. Override with `cookiePath`, including an explicit JSON file.           |
| `common/_replacement-profiles.yaml` or `.yml`                    | Object with `profiles`, mapping names to ordered replacement-rule lists. Initialized as `profiles: {default: []}`. |
| `common/_override.css`                                           | Browser CSS injected before capture when `useCssOverride` is true; override with `cssOverridePath`.                |
| `common/_on-ready.js`                                            | Browser-side JavaScript applied once per document before capture; override with `jsOnReadyPath`.                   |

Configured paths must remain inside the canonical workspace and be regular
files. Explicit missing paths are errors. The default cookie/CSS/ready files are
optional when absent, but validated when present. A missing default viewport
file permits an unnamed default capture; an explicit missing viewport file is an
error. No named viewport filter can select labels from an absent file.

## Viewport and Profile Formats

```yaml
- label: phone
  width: 375
  height: 667
- label: desktop
  width: 1366
  height: 768
```

Viewport dimensions are positive whole CSS-pixel values. Labels must be nonblank,
case-insensitively unique, and produce distinct screenshot suffixes of at most
48 bytes. Distinct text can normalize to the same suffix, so avoid punctuation
variants. `viewportNames` values select these labels, not device types or sizes.

```yaml
profiles:
  default: []
  staging:
    - ref: https://example.com
      test: https://staging.example.com
```

Select the second profile with `regressify test --replacement-profile staging`.
Only test captures rewrite reference URLs. Profile rules run before workspace
rules and then suite rules. Use the matching schemas and the `viewports` and
`replacement-profiles` examples for complete values.

## Cookies

A cookie entry requires a nonempty string `name`, string `value`, and a nonblank
`url` or `domain`. `domain` may be a string or nonempty list; list entries expand
into separate cookies. Optional `path` and `sameSite` are strings; `secure` and
`httpOnly` are booleans; `expires` is a finite numeric Unix timestamp in seconds.
`sameSite` accepts Strict, Lax, or None, ignoring case; None requires secure true.
Additional browser cookie fields are allowed, unlike unknown typed config keys.

Name plus value must fit within Chromium's 4096-byte limit. Values must not contain
CR, LF, or NUL. Keep chunked authentication cookies separate; do not join, trim,
truncate, or rewrite their contents. Quote values in YAML; block `|-` avoids a
trailing newline when needed. Real cookies are secrets and must stay local.
The `cookies` example contains only a synthetic consent value. Cookie validation
uses the runtime parser, not an independently maintained JSON Schema here.

## CSS and JavaScript

CSS overrides are ordinary CSS, not YAML. Hide only approved nondeterministic
regions; do not conceal the behavior under test. Ready JavaScript executes in
the page, with DOM APIs and `window.__REGRESSIFY__` / `window.regressify` context.
The context includes the scenario's identity, URL, and `requiredLogin` flag.
CSS/ready hooks apply once for each document, including subsequent navigation.
Errors fail the capture; do not assume a failed script was applied.

These browser scripts do not have Node's filesystem or `require`. The separate
scenario `onBeforeScript` and `onReadyScript` options invoke Node hooks and need
Node plus workspace Playwright/playwright-core. Do not close the attached
browser. Read `get_authoring_guide(topic="hooks")` for their six-argument export
contract, browser context shape, lifecycle, and timeouts; `onReadyScript` runs
after actions, not before them. Upload files referenced by `input.file` live under `visual_tests/` and
use paths relative to that folder. Never execute untrusted scripts as validation.

## Generated and Sensitive Files

`common/test-schema.json`, `common/regressify-schema.json`, and
`common/replacement-profiles-schema.json` are editor schemas generated by `init`.
They are not runtime configuration. Existing copies are not refreshed by `init`;
check CLI/schema versions before relying on an old copy. VS Code settings and
extension recommendations are editor integration, not suite fields.

The default output directory is `.regressify/`; `--site-env` selects
`.regressify__<environment>/`, and `--regressify-dir` overrides the output root.
These root options precede the command. Its `states/<name>.json` files contain
sensitive browser state generated by `persist`. Do not hand-author, publish, or
upload them. Reference/test PNGs, reports, manifests, and diagnostic journals
are generated output, not test configuration. Preserve baselines until the
user explicitly authorizes reference capture or approval.
Retrieve `get_authoring_guide(topic="troubleshooting")` for concrete report paths,
capture-error/log sidecars, operation journals, and missing-log interpretation.

Legacy `data/` files can be copied by CLI initialization; do not create new
workspaces in that layout. `_signing-in.yaml` is not a newly initialized default.
Do not invent a required login file or assume that `requiredLogin` signs in.
