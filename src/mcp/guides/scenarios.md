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
suite and use portable single-component names of at most 80 UTF-8 bytes. Without
`id`, the ID is a lowercase slug of `label` (`Account overview` becomes
`account-overview`), or a 20-character hash of `url` when `label` is absent.

## Capture Lifecycle

Each selected viewport runs this sequence in its own fresh browser profile:

1. Open a blank tab at the viewport size; install Basic Auth, CSP bypass,
   saved state, and cookies.
1. Run `onBeforeScript`, navigate to `url` with `engineOptions.gotoParameters`,
   then wait until `readySelector` exists and is visible.
1. Apply the CSS override, `hideSelectors`, `removeSelectors`, and the browser
   ready script; freeze animations; scroll down and back to the top once to
   trigger lazy loading unless `noScrollTop` is true.
1. Run inherited `needs` actions, then `actions`, or legacy shortcuts when
   `actions` is omitted. Each `save` waits `delay`, settles, and captures at
   its position.
1. Run `onReadyScript`. If final captures remain, wait `postInteractionWait`,
   then `delay`, freeze animations, settle within `settleTimeout`, hide text
   carets, and capture each final target.

The lazy-load scroll is not repeated, so hover and scroll states created by
actions survive until capture. CSS and ready scripts apply once to each new
document after navigation. The whole sequence must finish within the workspace
`scenarioTimeout`; completed checkpoints are kept when it expires.

## Reusing Actions

`needs` accepts an ID or list of IDs from the same suite. It replays dependency
actions in the consumer's browser context before the consumer's own actions.
It is not scheduling, a prerequisite test result, or a shared browser session.
The dependency's `url` is not opened: the consumer navigates to its own `url`
first. Do not assume that naming another scenario automatically performs its
login; put necessary navigation and interactions into actions.
Missing IDs, self-dependencies, duplicate IDs, dependency cycles, and excessive
dependency depth are preflight errors. Do not make scenarios depend on run order.

Use `get_example(task="reuse-actions")` for a complete suite demonstrating reuse.
Viewport filters on inherited actions are checked against the consumer's
effective viewport set, not just the scenario where the actions were written.

## Capture Selection

- Without `selector` or `selectors`, the automatic final screenshot is full page.
- `selector: "#content"` crops only that final screenshot to the first matching
  element, keeping the `FINAL` file name.
- `viewport` in `selector`, `selectors`, or a `save` action's `selector`
  captures the currently visible viewport, including any scroll position left
  by actions, without scrolling. Otherwise `selectors` takes a string or list of
  CSS selectors and produces a separate capture for each target.
- Screenshot selectors are standard CSS evaluated in the selected tab's main
  document. The element must be visible with positive size; missing, hidden, or
  invalid targets fail instead of falling back to the full page. To capture
  framed content, target the `iframe` element itself.
- A plural selector list must be nonempty and contain no blank targets.
  `selectors: []` is invalid; omit it or use null for automatic full-page capture.
- Do not combine singular `selector` with plural `selectors`.
- A `save` action creates a named checkpoint; its own `selector` is independent
  of the scenario's final `selector`.
- If the last applicable action among the scenario's own `actions` is `save`,
  the automatic final full-page or singular-selector capture is skipped.
  Explicit plural `selectors` still run. Action viewport filters decide which
  action is last; saves inherited through `needs` still produce checkpoints but
  never suppress the final capture.
- Captures may be at most 100000 CSS pixels per side and 64000000 in total.

Captures are saved as `<scenario-id>[_<viewport>]__<target>.png`. `<viewport>`
is the slugified viewport label, omitted without a viewport file, and `<target>`
is `FINAL`, `VIEWPORT`, `$<save-name>`, or `SELECTOR-<slug>-<hash>`. Inherited
saves use the consuming scenario's ID. Use portable, distinct save names;
preflight rejects colliding screenshot names. Do not rename IDs or checkpoints
casually when existing references depend on them.

Comparison is strict about size: a test image whose dimensions differ from its
reference always fails, whatever the thresholds. Full-page captures therefore
fail when unrelated content changes the page height; capture components with
`selectors` when only part of a page is under test.

## Opened Menus and Overlays

An element capture is a rectangle of rendered pixels: Regressify scrolls the
first match toward the viewport center, captures the area inside its border
box, and restores the scroll position. The image includes anything drawn inside
that box, even an overlay from elsewhere in the page, and excludes everything
outside it, even the element's own overflowing children. Layers opened by
actions rarely fit inside their trigger's container:

- Dropdowns and mega menus usually hang below the header or navigation bar.
- Mobile menus are often fixed drawers or full-screen overlays.
- Account, language, cart, and search-suggestion lists, tooltips, popovers, and
  date pickers are often added at the end of `body` and positioned near their
  trigger.
- Dialogs and consent banners are fixed layers over the page.
- Accordions and other inline expansions usually grow their container, so
  capturing the container still works.

A scenario that opens a menu and captures `header` therefore records only the
header bar. Decide which state each image tests, then choose a target that
contains it:

- Closed state: capture the container, such as `header`, before opening
  anything, usually in its own scenario.
- Opened state: capture `viewport`, the opened layer's own element, or both,
  for example `selectors: [viewport, "#products-panel"]`. Use `viewport` for
  fixed drawers and overlays, and for layers opened by hover or closed by
  scrolling, because viewport captures never scroll.
- Several states in one scenario: `save` each state with its own `selector`,
  and close a layer, for example by clicking its toggle again or pressing
  `Escape` on it, before capturing anything it covers.

Wait for the opened layer itself, as in `wait: "#products-panel"`, before
capturing it. When the page's markup or styles show that a layer is positioned
absolutely or fixed, or rendered outside the container, assume a container
capture cuts it off. The recipes topic has a complete navigation suite.

## Viewports, Waiting, and State

`viewportNames` accepts one label or a list, matching case-insensitively. It
replaces inherited suite/workspace selection; omitted or null inherits, while an
empty list selects all configured viewports. An unknown label is an error.
Each scenario/viewport capture is independent. Without a viewport file, or with
an empty list in it, every scenario captures once at 1920x1080 CSS pixels and
scenario, suite, and workspace `viewportNames` are ignored; action-level
`viewportNames` must then be omitted, null, or empty.

`readySelector` waits after navigation until the element exists and is visible;
`readyTimeout` is whole milliseconds and defaults to the workspace element
timeout when omitted or zero. It runs before browser ready scripts and
dependency/scenario actions, so do not use it to wait for content created by
those later steps. Use observable page-state waits in `actions` for subsequent
changes. `delay` (default 1000 ms) runs before every checkpoint and final
capture; `postInteractionWait` (default 1000 ms) runs once after actions and
`onReadyScript`, before final captures only. Set either to 0 when explicit waits
already prove readiness.

Legacy click, hover, scroll, and text-append shortcuts run only when `actions`
is omitted. An explicit list, including `[]`, disables those shortcuts;
inherited `needs` actions still run. Ordered actions are clearer for new
scenarios.

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
