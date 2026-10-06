# Browser and Node Hooks

Hooks are executable project code. Use them only when ordinary actions and
configuration do not express the requested behavior. Inspect existing scripts
locally and obtain permission before adding dependencies or running untrusted code.
This MCP server supplies instructions only; it never executes hooks.

## Two Different Hook Systems

| Setting          | Scope and Execution                                                                                                                                       |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `jsOnReadyPath`  | Workspace, suite, or scenario. Browser-side JavaScript, once per active page document after readiness and before actions. Default: `common/_on-ready.js`. |
| `onBeforeScript` | Scenario only. Node.js, after state/cookie setup and before initial navigation.                                                                           |
| `onReadyScript`  | Scenario only. Node.js, after dependency, scenario, and legacy actions, before automatic final captures. Receives the selected tab.                       |

Paths are relative to the workspace root, not to the suite or hook directory.
Explicit paths must name regular files inside the canonical workspace. Browser
scripts use DOM APIs, not Node imports, `require`, or filesystem APIs.

## Capture Lifecycle

1. Create the scenario page, apply browser/viewport settings, and install state
   restoration and cookies.
1. Run `onBeforeScript` on the page before its initial navigation.
1. Navigate using `engineOptions.gotoParameters`, then wait until
   `readySelector` exists and is visible.
1. Apply the CSS override, `hideSelectors`/`removeSelectors`, and the browser
   ready script; freeze animations and scroll once for lazy content.
1. Run inherited `needs` actions, then scenario `actions`, or legacy shortcuts
   when `actions` is omitted. Each applicable `save` captures at its position.
1. Run `onReadyScript`. If final captures remain, wait `postInteractionWait`
   and `delay`, settle, and capture them.

`readySelector` is checked before browser ready scripts and actions. Do not point
it at content that those later steps create. `onReadyScript` cannot prepare an
earlier click/input or alter an already completed `save`; use an earlier action
or browser ready script where appropriate. A last applicable `save` can suppress
the automatic final capture, while explicit scenario `selectors` still run.
Browser hooks are applied to new documents after navigation and tab selection;
switching back to an already processed document does not rerun them.

## Node Contract

Node hooks need local Node.js and a workspace installation of `playwright` or
`playwright-core`. With permission, `npm install --save-dev playwright-core`
provides the attachment library; it does not replace Regressify's Chromium setup.
Use `.mjs` with a default export, or `.cjs` with `module.exports = async function`.
A named-only export is not accepted. A `.js` file follows the workspace
`package.json` module type, so prefer an explicit extension. Hooks are awaited
and must finish. The hook process starts in the workspace root and inherits the
CLI's environment variables, so it can read credentials from `process.env`
instead of storing them in suite files.

```javascript
export default async function onReady(page, scenario, viewport, isReference, context, config) {
  await page.waitForSelector("main", {
    state: "visible",
    timeout: config.elementTimeoutMs,
  });
}
```

| Argument      | Meaning                                                                                                                                                                                      |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `page`        | The existing Playwright Page for this scenario's currently selected tab.                                                                                                                     |
| `scenario`    | Scenario configuration plus resolved `requiredLogin`, initial capture `url`, and suite output ID in `testSuite`. Use `page.url()` for the current URL after navigation actions.              |
| `viewport`    | `{label, width, height}` or `null` when no named viewport is configured.                                                                                                                     |
| `isReference` | `true` for reference capture, `false` for test capture.                                                                                                                                      |
| `context`     | The existing Playwright BrowserContext containing `page`.                                                                                                                                    |
| `config`      | Resolved camelCase capture configuration. Runtime timeouts use `elementTimeoutMs`, `actionabilityTimeoutMs`, `settleTimeoutMs`, and `scenarioTimeoutMs`. Basic Auth credentials are omitted. |

Reuse the supplied page/context; do not launch another browser or close the
attached browser, context, or page. Use Regressify's `switchTab` action when
subsequent actions/captures need a different tab. Other scenarios have independent
pages; never depend on their execution order or mutate a sibling's session.

Node execution, including attachment/import, is bounded by workspace
`elementTimeout` (default 30000 ms); the overall `scenarioTimeout` also applies.
Throw an error when the intended operation fails. Standard output is discarded;
retained standard error is bounded and contributes to hook failures. Reports
and capture logs keep only an error category for a failed hook, not its message.
While diagnosing, temporarily append sanitized progress notes to a local,
version-control-ignored file from the hook and remove that code afterwards.
Never log credentials, cookies, input values, or private response bodies.

Retrieve `get_example(task="node-before-hook")`, `node-ready-hook`, and
`hooked-suite` together for a complete setup. The first hook sets a synthetic
reference/test header; the second waits for a visible `main` before final capture.
Adapt them to verified application behavior, not merely to make a test pass.

## Browser Ready Context

Both `window.__REGRESSIFY__` and `window.regressify` refer to this shape:

```json
{
  "requiredLogin": false,
  "scenario": {
    "id": "home",
    "label": "Home",
    "description": null,
    "url": "https://example.com",
    "requiredLogin": false
  }
}
```

Identity/description fields may be absent or null according to the scenario.
Use `window.location.href` for the currently navigated URL, especially after URL
replacements, `goto`, or popup selection. `requiredLogin` is a flag, not a login
implementation. An asynchronous browser script should return its promise, for
example with an async immediately invoked function. Do not export a Node module
from `common/_on-ready.js`.

For missing modules, wrong exports, hook timeouts, and hook-versus-action ordering
failures, retrieve `get_authoring_guide(topic="troubleshooting")`.
