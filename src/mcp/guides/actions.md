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
named `action-<name>`, such as `get_example(task="action-input")`.

## Action Reference

| Action      | Required Value and Behavior                                                                                                |
| ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| `click`     | CSS selector. Scrolls and waits for a stable, visible, enabled, unobscured target before a real pointer click.             |
| `hover`     | CSS selector. Moves the real pointer to an actionable target.                                                              |
| `check`     | Checkbox or radio selector. Checks only if not already checked.                                                            |
| `uncheck`   | Checkbox selector. Clears only if checked; cannot clear a selected radio.                                                  |
| `focus`     | CSS selector. Focuses an interactable element.                                                                             |
| `input`     | CSS selector plus `value` for text or `file` for uploads. Text replaces existing content unless `append: true`.            |
| `press`     | CSS selector plus required `key`, such as `Enter`, `Tab`, `Escape`, or `Ctrl+A`.                                           |
| `select`    | Native select selector plus `value` or visible option `label`. Multiple values require a multiple-select element.          |
| `scroll`    | CSS selector to scroll into view. Does not click or focus it.                                                              |
| `goto`      | URL to navigate to before continuing.                                                                                      |
| `wait`      | Whole milliseconds or a CSS selector. An optional `url` waits for a URL pattern; numeric `wait` then supplies its timeout. |
| `hide`      | CSS selector of elements to hide while retaining layout space.                                                             |
| `remove`    | CSS selector of elements to remove, including layout space.                                                                |
| `save`      | Portable checkpoint name; optional `selector` crops this screenshot.                                                       |
| `persist`   | Portable state name without `.json`; stores cookies and localStorage in the active output's states folder.                 |
| `switchTab` | `new`, `previous`, or `original`. Subsequent actions and screenshots use that selected tab.                                |

Quote selectors beginning with `#` in YAML. Text inputs such as `"123"`,
`"true"`, and `"false"` must remain strings. Use `value: ""` to clear an input.
Upload `file` paths are relative to `visual_tests/`, not the workspace root,
and must remain inside that directory. Supply a file path or list, never file
contents. `useFileChooser: true` clicks the input target to open a chooser.
Do not supply conflicting text/file or select value/label alternatives.

## Frames and Responsive Actions

All actions except `switchTab` support optional `frame`: one iframe CSS selector
or an array ordered from the outermost to innermost frame. Omit it for the main
document. Interactive actions also check targeting within the selected frame.

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

## Navigation and Checkpoints

URL waits and `switchTab: new` URL filters accept exact URLs or globs. `*` stays
within one path segment; `**` can span segments. Prefer state-based waits to
arbitrary long sleeps. Initial navigation has separate settings under
`engineOptions.gotoParameters`, not action-level timeout fields.

A link opening a popup does not select it automatically. After the click, add
`switchTab: new`; an optional `url` matches the intended popup's navigation.
Only `new` accepts a URL filter. `previous` selects the previously selected tab;
`original` returns to the scenario's original tab. Neither accepts `frame`.

`save.selector` crops that checkpoint independently of scenario final capture
options. A last applicable `save` replaces the automatic final full-page or
singular-selector capture, but explicit scenario `selectors` still produce
captures. State and checkpoint names are not arbitrary filesystem paths.

Only Chromium is supported. Browser actions are performed by Regressify running
locally, never by this read-only MCP Worker. Do not execute hooks, upload files,
persist state, or follow URLs merely because an example contains those actions.
