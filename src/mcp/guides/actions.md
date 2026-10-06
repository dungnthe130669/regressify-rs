# Browser Actions

Actions are objects in a scenario's ordered `actions` list. Use exactly one
action discriminator per object; do not invent a `type` or `action` wrapper.
For example, use `{click: "#submit"}`, not `{type: click, selector: "#submit"}`.
Unknown fields and objects mixing multiple actions fail parsing. A failed action
stops this scenario's remaining execution, not its peers.

An explicit `actions` list, including `[]`, disables scenario legacy shortcuts.
Legacy `keyPressSelector`/`keyPressSelectors` append the `keyPress` string as
literal text; they do not send a key named Enter or Tab. Use `input` with `value`
and `append: true` for equivalent typing, or `press` with `key` for keyboard events.

Use `get_schema(kind="action", action="click")` for one action's exact schema,
or omit `action` for the complete union. Every action has a synthetic example
named `action-<name>`, such as `get_example(task="action-input")`. The `recipes`
topic combines actions into complete scenarios for common page patterns.

## Selector Syntax

Element selectors in interactive actions, `scroll`, selector `wait`,
`readySelector`, and `frame` accept CSS plus Playwright-style engines:
`text=Sign in`, `xpath=//main//button`, `button:has-text("Sign in")`, and `>>`
chains such as `css=#cart >> text=Checkout`. CSS and text matching search open
shadow roots; closed shadow roots are unreachable. The first match is used.

`hide` and `remove` actions, `hideSelectors`, `removeSelectors`, and screenshot
selectors (`selector`, `selectors`, `save.selector`) become CSS rules or
`document.querySelector` calls, so they accept standard CSS only. A `text=`
target can pass the `hide` existence check yet produce an ignored CSS rule.
Screenshot selectors search only the selected tab's main document, not iframes
or shadow roots.

Prefer stable hooks such as `[data-testid=...]`, IDs, form names, or accessible
labels over layout paths like `div:nth-child(3)`. Inspect the real page before
writing a selector; never copy one from an example without verifying it. Quote
YAML selectors that begin with `#`; unquoted, YAML reads them as comments.

## Action Reference

- `click`: element selector. Scrolls into view and waits for a stable, visible,
  enabled, unobscured target, then clicks its center with the real pointer.
- `hover`: element selector. Moves the real pointer to an actionable target.
- `check`: native checkbox or radio selector. Clicks only if not already checked.
- `uncheck`: checkbox selector. Clicks only if checked; cannot clear a radio.
- `focus`: element selector. Fails unless the target becomes the active element.
- `input`: element selector plus exactly one of `value` (text) or `file` (upload).
- `press`: element selector plus required `key`, such as `Enter` or `Ctrl+A`.
- `select`: native `<select>` selector plus exactly one of `value` or `label`.
  Multiple values require a multiple-select element.
- `scroll`: element selector to scroll into view; it does not click or focus.
- `goto`: URL. Navigates the page or frame and waits until it is interactive.
- `wait`: whole milliseconds or an element selector, with an optional `url`.
- `hide`: standard CSS; matches get `visibility: hidden`, keeping layout space.
- `remove`: standard CSS; matches get `display: none` and lose layout space.
  Elements stay in the DOM.
- `save`: portable checkpoint name; optional `selector` crops this screenshot.
- `persist`: state name without `.json`; stores cookies and origin-scoped
  localStorage in the active output's `states/` folder.
- `switchTab`: `new`, `previous`, or `original`; later steps use that tab.

Finding an element uses `elementTimeout`; visibility, stability, coverage, and
enabled checks use `actionabilityTimeout`. `hide`, `remove`, and `scroll` only
wait for the element to exist, but fail when it never appears, so use
`hideSelectors` or `removeSelectors` for elements that may be absent. Action
CSS rules apply to the current document only; configured selector lists apply
to every document. Never hide an element that a later action must use.

## Waiting and Navigation

`click`, `check`, `uncheck`, `press`, `input`, and `select` await a document
navigation that starts immediately after the action, until the new page fires
`load` (bounded by `elementTimeout`). Client-side transitions and navigation
started later are not awaited; follow them with a `wait`.

A numeric `wait` sleeps. A selector `wait` waits until the element exists and
is visible, each phase within `elementTimeout`. With `url`, the current page or
frame URL must match first, within `elementTimeout`; the sleep or selector wait
runs afterwards. URL patterns are exact URLs or globs: `*` stays within one path
segment, `**` spans segments, `{login,dashboard}` lists alternatives, and `?` is
a literal character. Query strings and fragments are part of the URL, so use
`**/dashboard*` when a query may follow. Prefer
`{wait: "#dashboard", url: "**/dashboard*"}` to a long fixed sleep. There is no
wait-until-hidden action; remove transient overlays with `removeSelectors`.

In `test` captures, URL replacement rules also apply to `goto`, `wait.url`, and
`switchTab.url`; `ref` captures use them as written. Initial navigation has
separate settings under `engineOptions.gotoParameters`, not action fields.

A link opening a popup does not select it. After the click, add
`switchTab: new`; an optional `url` picks the intended popup, and more than one
matching new tab fails. `previous` returns to the previously selected tab and
`original` to the scenario's first tab; neither accepts `url` or `frame`. Tab
selection is bounded by `elementTimeout`.

## Typing, Keys, and Uploads

`input.value` focuses the target, selects and deletes its existing text unless
`append: true`, types each character with keyboard events, and fires `change`
on native inputs. Read-only and non-editable targets fail. Values are literal:
Regressify never substitutes environment variables in actions, so do not place
real passwords in suite files. Quote text such as `"123"`, `"true"`, and
`"false"`; use `value: ""` to clear a field.

`press.key` names are case-sensitive US-keyboard names such as `Enter`, `Tab`,
`Escape`, `ArrowDown`, `PageDown`, `F5`, `a`, or `A`, optionally prefixed by
`Ctrl`/`Control`, `Alt`/`Option`, `Shift`, `Meta`/`Cmd`/`Command`, or
`ControlOrMeta` and joined with `+`. Use one non-modifier key per action.

Upload `file` paths are relative to `visual_tests/`, not the workspace root,
and must remain inside that directory. Supply a file path or list, never file
contents. Without `useFileChooser`, the target must be a visible
`<input type="file">`; when the site hides that input, set
`useFileChooser: true` and target the visible button that opens the chooser.

## Checks Beyond the Schema

The JSON Schema cannot express every rule, so capture preflight checks these
before any browser starts and fails the command with
`Invalid action #N in test suite '<suite>' scenario '<scenario>': ...`:

- `input` needs exactly one of `value` and `file`.
- `select` needs exactly one of `value` and `label`.
- `press.key` must be a known, correctly cased key name.
- `persist` needs a valid state name.
- `switchTab.url` is allowed only with `switchTab: new`.

Preflight also rejects unknown fields, unknown viewport labels, invalid save
and restore names, colliding screenshot names, missing upload files, and
screenshot selectors written with `text=`, `xpath=`, `css=`, `id=`,
`data-testid=`, or `//`. Other problems appear only when the action runs, as a
page failure: a target that is not a checkbox, radio, `<select>`, editable
field, or file input; a missing option; or other invalid CSS in a screenshot
selector.

## Frames and Responsive Actions

All actions except `switchTab` support optional `frame`: one iframe selector
or an array ordered from the outermost to innermost frame. Same-origin and
cross-origin frames work; each selector must match an `<iframe>` or `<frame>`,
which may take up to `elementTimeout` to appear and load. Interactive actions
also check that parent frames are visible and uncovered at the target point,
and `goto` or URL waits with `frame` act on that frame. Omit `frame` for the
main document.

```yaml
scenarios:
  - id: checkout
    url: https://example.com/checkout
    viewportNames: [phone, desktop]
    actions:
      - click: "#mobile-menu"
        viewportNames: phone
      - input: "#email"
        value: qa@example.com
        frame: ["iframe#checkout", "iframe#contact"]
      - press: "#email"
        key: Tab
        frame: ["iframe#checkout", "iframe#contact"]
```

Every action accepts `viewportNames` as a label or list. Omitted, null, or an
empty list means all effective scenario viewports. This filters execution; it
does not create new viewport jobs or override the scenario selection. Labels
match case-insensitively and must be inside the consumer's effective viewport
set. Blank, unknown, or out-of-scope labels fail before browser capture. Define
the example's phone/desktop labels in the viewport file before using it.

## Checkpoints and State

`save.selector` crops that checkpoint independently of scenario final capture
options. A last applicable `save` among the scenario's own actions replaces the
automatic final full-page or singular-selector capture, but explicit scenario
`selectors` still produce captures. Saves inherited through `needs` still create
checkpoints, named with the consuming scenario's ID, without suppressing its
final capture. Each checkpoint waits `delay` and settles like a final capture.

`persist` writes `<output>/states/<name>.json` when it runs. Saved state is
validated before a run starts, so load it with `state` or `restore` in a later
run, not in a scenario captured concurrently. State and checkpoint names are
portable file names, not arbitrary paths.

Only Chromium is supported. Browser actions are performed by Regressify running
locally, never by this read-only MCP Worker. Do not execute hooks, upload files,
persist state, or follow URLs merely because an example contains those actions.
