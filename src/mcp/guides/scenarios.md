# Test Scenarios

A scenario is one entry in a suite's `scenarios` array. It opens its own required
`url`, executes optional `actions` in order, and captures the requested result.
Use `get_schema(kind="scenario")` for exact supported fields. A scenario is not
a standalone suite file: its object must be placed inside `scenarios`.

```yaml
scenarios:
  - id: account
    label: Account overview
    url: https://example.com/account
    readySelector: main
    actions:
      - click: "#details"
      - wait: "#account-details"
    selector: "#account-details"
```

URLs and selectors here are synthetic. Verify them against the user's page.
Quote YAML selectors beginning with `#`; otherwise YAML treats them as comments.
Prefer explicit `id` and `label`. IDs are case-insensitively unique within their
suite and use portable single-component names of at most 80 UTF-8 bytes.

## Reusing Actions

`needs` accepts an ID or list of IDs from the same suite. It replays dependency
actions in the consumer's browser context before the consumer's own actions.
It is not scheduling, a prerequisite test result, or a shared browser session.
Do not assume that naming another scenario automatically performs its login or
navigates to its URL; put necessary navigation and interactions into actions.
Missing IDs, self-dependencies, duplicate IDs, dependency cycles, and excessive
dependency depth are preflight errors. Do not make scenarios depend on run order.

Use `get_example(task="reuse-actions")` for a complete suite demonstrating reuse.
Viewport filters on inherited actions are checked against the consumer's
effective viewport set, not just the scenario where the actions were written.

## Capture Selection

- Without `selector` or `selectors`, the automatic final screenshot is full page.
- `selector: "#content"` crops only that final screenshot to one CSS selector.
- `selectors: viewport` captures the visible viewport. A string or list of CSS
  selectors produces separate captures for those targets.
- A plural selector list must be nonempty and contain no blank targets.
  `selectors: []` is invalid; omit it or use null for automatic full-page capture.
- Do not combine singular `selector` with plural `selectors`.
- A `save` action creates a named checkpoint; its own `selector` is independent
  of the scenario's final `selector`.
- If the last applicable action is `save`, the automatic final full-page or
  singular-selector capture is skipped. Explicit plural `selectors` still run.
  "Applicable" accounts for action viewport filters and inherited actions.

Use portable, distinct save names. Preflight rejects colliding screenshot names.
Do not rename IDs or checkpoints casually when existing references depend on them.

## Viewports, Waiting, and State

`viewportNames` accepts one label or a list, matching case-insensitively. It
replaces inherited suite/workspace selection; omitted or null inherits, while an
empty list selects all configured viewports. An unknown label is an error.
Each scenario/viewport capture is independent.

`readySelector` waits after navigation; `readyTimeout` is whole milliseconds and
defaults to the workspace element timeout when omitted or zero. It runs before
browser ready scripts and dependency/scenario actions, so do not use it to wait
for content created by those later steps. Use observable page-state waits in
`actions` for subsequent changes. Legacy click, hover, scroll, and text-append
shortcuts run only when `actions` is omitted. An explicit list, including `[]`,
disables those shortcuts; inherited `needs` actions still run. Ordered actions
are clearer for new scenarios.

Workspace/suite `state` restores saved state; scenario `restore` adds state names
to the inherited selection in order, without duplicates. `persist` writes cookies
and origin-scoped localStorage for later captures. State names omit `.json` and
must be portable; files live in the active output directory's `states/` folder.
Configured state files must already exist and validate before capture starts.
Use separate authorized captures to create state before a dependent run; do not
try to establish it through concurrently scheduled scenarios. Never commit or
upload real saved state. `requiredLogin` is only a flag exposed to ready scripts;
it does not perform authentication.

`onBeforeScript` and `onReadyScript` are optional Node hooks, distinct from the
browser-side `common/_on-ready.js`. They require local Node plus Playwright or
playwright-core installed in the workspace. They attach to the existing browser
and must not close it. `onBeforeScript` runs before initial navigation;
`onReadyScript` runs after all dependency, scenario, and legacy actions, before
remaining final captures. It does not affect earlier `save` screenshots.
Read `get_authoring_guide(topic="hooks")` for export signatures, arguments,
timeouts, and the `hooked-suite`, `node-before-hook`, and `node-ready-hook` examples.
Do not execute or introduce untrusted scripts. For failed captures, use the
troubleshooting topic before changing the scenario or its baselines.
