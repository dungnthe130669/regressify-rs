# Test Actions

Each test scenario can have zero, one, or multiple actions. Actions are executed sequentially in the order they are defined.

## Actions Reference

- [check](#check) — Check a checkbox or radio element
- [click](#click) — Click an element
- [focus](#focus) — Focus an element
- [goto](#goto) — Navigate to a URL
- [hide](#hide) — Hide an element (`visibility: hidden`)
- [hover](#hover) — Hover over an element
- [input](#input) — Type text or upload files into an element
- [persist](#persist) — Save browser state (cookies + localStorage) to disk
- [press](#press) — Press a key or key combination on an element
- [remove](#remove) — Remove an element (`display: none`)
- [save](#save) — Take an intermediate screenshot
- [scroll](#scroll) — Scroll to an element
- [select](#select) — Select an option in a `<select>` element
- [uncheck](#uncheck) — Uncheck a checkbox or radio element
- [wait](#wait) — Wait for a delay, an element, or a navigation

---

## check

Check a checkbox or radio element.

| Property | Type                 | Required | Description                                       |
| -------- | -------------------- | -------- | ------------------------------------------------- |
| `check`  | `string`             | yes      | CSS selector targeting the checkbox/radio element |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame).   |

```yaml
actions:
  - check: checkbox#tos-accept
```

**Implementation:** Finds the element and clicks it. Inside a frame, executes via JS `el.click()`.

**Rust types:** `CheckAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## click

Click an element.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `click`  | `string`             | yes      | CSS selector of the element to click            |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

> **Important:** If the click leads to a navigation to another URL, ensure the very next action is [wait](#wait) for that URL to fully load.

```yaml
actions:
  - click: button.submit
```

**Implementation:** Finds the element and clicks it. Inside a frame, executes via JS `el.click()`.

**Rust types:** `ClickAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## focus

Focus an element.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `focus`  | `string`             | yes      | CSS selector of the element to focus            |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - focus: input[name="email"]
```

**Implementation:** Finds the element and focuses it. Inside a frame, executes via JS `el.focus()`.

**Rust types:** `FocusAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## goto

Navigate to a specific URL.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `goto`   | `string`             | yes      | The URL to navigate to                          |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - goto: https://www.domain.com
```

**Implementation:** Calls `page.goto(url)`.

**Rust types:** `GotoAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## hide

Hide an element by setting `visibility: hidden !important`.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `hide`   | `string`             | yes      | CSS selector of the element to hide             |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - hide: '#cookie-modal'
```

**Implementation:** Injects a `<style>` tag into the document head with the rule `{selector} { visibility: hidden !important; }`.

**Rust types:** `HideAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## hover

Hover over an element.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `hover`  | `string`             | yes      | CSS selector of the element to hover            |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - hover: '#my-chart'
```

**Implementation:** Uses the browser's native hover. Inside a frame, dispatches a `mouseover` event via JS.

**Rust types:** `HoverAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## input

Type text into an element, or upload files to a file input.

| Property         | Type                 | Required | Description                                                                                     |
| ---------------- | -------------------- | -------- | ----------------------------------------------------------------------------------------------- |
| `input`          | `string`             | yes      | CSS selector of the input element                                                               |
| `value`          | `string`             | no       | Text to type into the element. Mutually exclusive with `file`.                                  |
| `append`         | `bool`               | no       | If `true`, appends to existing value. Default: `false` (clears first).                          |
| `file`           | `string \| string[]` | no       | File path(s) relative to `visual_tests/`. Mutually exclusive with `value`.                      |
| `useFileChooser` | `bool`               | no       | If `true`, clicks the input to trigger the file chooser before setting files. Default: `false`. |
| `frame`          | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame).                                                 |

> **Validation:** Exactly one of `value` or `file` must be provided. Specifying both or neither is an error.

### Type text

```yaml
actions:
  # Type into an input, clearing existing value first (default)
  - input: 'input#username'
    value: My Name

  # Append to existing value
  - input: 'input#email'
    value: my-name@domain.com
    append: true
```

### Upload files

```yaml
actions:
  # Upload a single file
  - input: 'input#file'
    file: file1.txt
    useFileChooser: true

  # Upload multiple files
  - input: 'input#files'
    file:
      - file1.txt
      - other/file2.pdf
```

**Implementation:**
- **Text input:** Finds the element, optionally clears it (via `el.value = ''`), then types character-by-character via `type_str()`. In a frame, uses JS to set the value and dispatch `input`/`change` events.
- **File upload:** Resolves paths relative to `visual_tests/`, validates they exist and are relative. Uses the CDP command `DOM.setFileInputFiles` to set files on the input node. If `useFileChooser` is `true`, clicks the element first.

**Rust types:** `InputAction`, `FileInput` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## persist

Save the current browser state (cookies and localStorage) to disk. This allows reusing the state (e.g., a logged-in session) in other test suites via the `state` property.

| Property  | Type                 | Required | Description                                     |
| --------- | -------------------- | -------- | ----------------------------------------------- |
| `persist` | `string`             | yes      | Name for the saved state                        |
| `frame`   | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - input: input#username
    value: aaaa
  - input: input#password
    value: pa$$w0rd
  - click: button#login-btn
  - persist: user-aaaa
```

To reuse the saved state in another test suite:

```yaml
state: user-aaaa
```

**Implementation:** Collects cookies via `page.get_cookies()` and localStorage via JS `JSON.stringify(Object.fromEntries(Object.entries(localStorage)))`. Saves both as JSON to `.regressify/states/{name}.json`.

**Rust types:** `PersistAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## press

Press a key or key combination on an element.

| Property | Type                 | Required | Description                                      |
| -------- | -------------------- | -------- | ------------------------------------------------ |
| `press`  | `string`             | yes      | CSS selector of the target element               |
| `key`    | `string`             | yes      | Key or key combination (e.g., `Enter`, `Ctrl+A`) |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame).  |

> **Important:** If the press leads to navigation, ensure the very next action is [wait](#wait) for that URL.

```yaml
actions:
  - press: 'textarea#message'
    key: Ctrl+A
```

**Implementation:** Finds the element and presses the key via `press_key()`. Inside a frame, dispatches `keydown`/`keyup` events via JS.

**Rust types:** `PressAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## remove

Remove an element from view by setting `display: none !important`.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `remove` | `string`             | yes      | CSS selector of the element to remove           |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - remove: div.random-content
```

**Implementation:** Injects a `<style>` tag into the document head with the rule `{selector} { display: none !important; }`.

**Rust types:** `RemoveAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## save

Take an intermediate screenshot during the action sequence.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `save`   | `string`             | yes      | Name for the screenshot step                    |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - save: before-submit
  - click: button.submit
  - save: after-submit
```

**Implementation:** Calls `save_screenshot()` with `ScreenshotType::Step(name)`. The file is saved as `{suite_id}__{scenario_id}${step_name}.png`.

**Rust types:** `SaveAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## scroll

Scroll to an element.

| Property | Type                 | Required | Description                                     |
| -------- | -------------------- | -------- | ----------------------------------------------- |
| `scroll` | `string`             | yes      | CSS selector of the element to scroll to        |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - scroll: .report
```

**Implementation:** Finds the element and calls `scroll_into_view()`. Inside a frame, uses JS `el.scrollIntoView()`.

**Rust types:** `ScrollAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## select

Select an option inside a `<select>` element.

| Property | Type                 | Required | Description                                                             |
| -------- | -------------------- | -------- | ----------------------------------------------------------------------- |
| `select` | `string`             | yes      | CSS selector of the `<select>` element                                  |
| `value`  | `string \| string[]` | no       | Option value(s) to select. Mutually exclusive with `label`.             |
| `label`  | `string`             | no       | Option label (display text) to select. Mutually exclusive with `value`. |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame).                         |

> **Validation:** Exactly one of `value` or `label` must be provided. Specifying both or neither is an error.

Given this HTML:

```html
<select name="method" multiple>
  <option value="email">Send via email</option>
  <option value="sms">Send via SMS</option>
  <option value="skype">Send via Skype</option>
</select>
```

```yaml
actions:
  # Select by value
  - select: select[name="method"]
    value: email

  # Select by label
  - select: select[name="method"]
    label: Send via email

  # Select multiple values
  - select: select[name="method"]
    value:
      - sms
      - skype
```

**Implementation:** Iterates over `<option>` elements via JS. For `value`, marks options whose value is in the provided list. For `label`, matches on `option.text`. Dispatches a `change` event after selection.

**Rust types:** `SelectAction`, `SelectValue` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## uncheck

Uncheck a checkbox or radio element.

| Property  | Type                 | Required | Description                                     |
| --------- | -------------------- | -------- | ----------------------------------------------- |
| `uncheck` | `string`             | yes      | CSS selector of the checkbox/radio element      |
| `frame`   | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame). |

```yaml
actions:
  - uncheck: checkbox[name="subscribe-email"]
```

**Implementation:** Finds the element and clicks it. Inside a frame, checks `el.checked` first and only clicks if it's currently checked.

**Rust types:** `UncheckAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## wait

Wait for a delay (milliseconds), an element to appear, or a navigation to complete.

| Property | Type                 | Required | Description                                                                               |
| -------- | -------------------- | -------- | ----------------------------------------------------------------------------------------- |
| `wait`   | `number \| string`   | yes      | Milliseconds to wait (number) or CSS selector of an element to wait for (string)          |
| `url`    | `string`             | no       | URL pattern to wait for after navigation. Supports `**` glob for arbitrary path segments. |
| `frame`  | `string \| string[]` | no       | Target iframe selector(s). See [frame](#frame).                                           |

> **Important:** Whenever an action causes navigation to another URL, the very next action should be a `wait` with the target `url` to ensure the page loads before continuing.

```yaml
actions:
  # Delay 2 seconds
  - wait: 2000

  # Wait for an element to appear
  - wait: .popup

  # Wait for navigation to /quote, then delay 2 seconds
  - wait: 2000
    url: '**/quote'

  # Wait for navigation to /quote, then wait for an element
  - wait: '#my-form'
    url: '**/quote'
```

**Implementation:**
1. If `url` is set: waits for navigation, then verifies the current URL matches. The `**` glob matches any sequence of characters in the URL.
2. If `wait` is a number: sleeps for that many milliseconds.
3. If `wait` is a string: finds the element (polls with 100ms intervals and 30s timeout when inside a frame).

**Rust types:** `WaitAction`, `Wait` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.

---

## Shared Properties

### frame

Type: `string | string[]`
Default: none

Specifies the CSS selector for one or more iframes in which the action should be executed. For nested iframes, provide an array of selectors from outermost to innermost.

```yaml
actions:
  # Click inside a single iframe
  - click: '#submit'
    frame: .child

  # Click inside a nested iframe
  - click: '#submit'
    frame:
      - .child
      - '#grandchild'
```

**Implementation:** Builds a JS expression that traverses `document.querySelector(sel).contentDocument` for each frame selector in order, then executes the action within the innermost frame's document context.

**Rust types:** `Frame` enum in `src/types/actions.rs`.
