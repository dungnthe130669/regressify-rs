---
title: Create an action
description: Reference for creating Optimizely CMS test actions such as click, input, select, wait, scroll, upload files, and frame handling.
guid: ba348ca1-eb67-4c0f-8461-8f36979aea18
date: 2023-07-03T17:00:00+00:00
updated: 2026-09-13T00:00:00+00:00
draft: false
hidden: false
weight: 40000
sort_by: weight
template: docs/page.html

extra:
  lead: This reference explains how actions shape each scenario, when to use them, and which details tend to cause avoidable failures, from navigation waits and file uploads to frame handling, persistence, and element visibility.
  toc: true
  top: false
---

## Actions

Each scenario can have zero, one or multiple actions. Each action must be one of the following types.

Interactive actions wait for the target element to exist, be visible, become stable, scroll into view, and not be covered by another element before interacting. Form controls must also be enabled. These checks intentionally make actions fail when a real user could not perform them.

Action selectors support CSS, `text=`, `xpath=`, Playwright CSS extensions such
as `:has-text()`, and `>>` selector chaining. CSS/text selectors traverse open
Shadow DOM. The bundled selector helper runs inside Chromium; Node is not
required for these actions. Closed shadow roots cannot be selected.

`frame` selectors use the same selector syntax. Same-origin, cross-origin,
and nested frames are resolved through CDP execution contexts. `goto` and URL
waits target the specified frame without navigating or checking the main page.
File uploads work in frames. With `useFileChooser: true`, `input` may identify
a visible upload button opening a separate hidden file input; without it,
`input` must identify a visible `<input type="file">`.
Direct browser operations used by interactive actions are bounded by the configured actionability timeout, so an unresponsive page cannot hold a scenario indefinitely.
Unknown action properties fail suite parsing instead of being ignored.

Desktop scenario logs record action targets and outcomes separately for each
concurrent scenario/viewport job. Text supplied through `input.value` is omitted
from action logs. Selectors, URLs, custom script messages, and errors may still
include project-provided data, so review logs before sharing them.

{% spoiler() %}

check

:::::

Check a checkbox or radio element.

Example:

```yml
actions:
  # Check the checkbox with `tos-accept` ID
  - check: checkbox#tos-accept
```

{% end %}

{% spoiler() %}

click

:::::

Click an element.

> Important:
>
> Click-triggered document navigation is awaited automatically. Use an explicit
> URL or selector `wait` for delayed application transitions and SPA readiness.

Example:

```yml
actions:
  # Click the button with .submit class
  - click: button.submit
```

{% end %}

{% spoiler() %}

focus

:::::

Focus an element.
The action fails if the requested element cannot become the active element,
preventing later keyboard input from being sent to a different target.

Example:

```yml
actions:
  # Focus an input element named `email`
  - focus: input[name="email"]
```

{% end %}

{% spoiler() %}

goto

:::::

Type: `string`.

Go to specific URL.
Regressify waits until the destination document is interactive before running the
next action. In `test` mode, active URL replacements are applied before
navigating; `ref` mode uses the URL as written.

Example:

```yml
actions:
  - goto: https://www.domain.com
```

{% end %}

{% spoiler() %}

hide

:::::

Hide an element - set `visibility: hidden !important`.

Example:

```yml
actions:
  # Hide the element with `cookie-modal` ID
  - hide: '#cookie-modal'
```

{% end %}

{% spoiler() %}

hover

:::::

Hover an element.

Example:

```yml
actions:
  # Hover mouse over an element contains `my-chart` ID
  - hover: '#my-chart'
```

{% end %}

{% spoiler() %}

input

:::::

Type text in an editable `<input>`, `<textarea>`, or contenteditable element.
Read-only and non-editable targets fail instead of silently succeeding.
Text is typed one character at a time with native keyboard events for mapped
keys, including inside frames. Unicode characters without a keyboard mapping
use Chromium text insertion. Browser edit constraints and `beforeinput`
handlers are honored. A native input receives a final `change` event.

Example:

```yml
actions:
  # Type `My Name` in an input element with `username` ID
  - input: 'input#username'
    value: My Name
    # `append` is an optional property, with `false` as the default value
    append: false

  # Type `my-name@domain.com` in an input element with `email` ID
  - input: 'input#email'
    value: my-name@domain.com
    # `append` is an optional property, with `false` as the default value
    append: true
  
  # Load `visual_tests/file1.txt` into the input with `file` ID
  - input: 'input#file'
    file: file1.txt
    # `useFileChooser` is an optional property, with `false` as the default value
    useFileChooser: true
  
  # Load `visual_tests/file1.txt` and `visual_tests/other/file2.pdf`
  # into the input with `files` ID
  # Note that the input must allow upload multiple files
  - input: 'input#files'
    file:
      - file1.txt
      - other/file2.pdf
    # `useFileChooser` is an optional property, with `false` as the default value
    useFileChooser: false
```

{% end %}

{% spoiler() %}

persist

:::::

Type: `string`.

Persists (saves) the current browser state, including cookies and local storage, to disk.

New state files use Playwright's `{ cookies, origins: [{ origin, localStorage }] }`
format. Local storage stays scoped to its origin and is restored before page
scripts execute. Changes made during navigation are retained when persisting;
later navigations do not reapply the original values. Legacy Rust
`{ cookies, localStorage }` files remain readable, with their unscoped local
storage assigned to the scenario's initial origin.

Example:

```yml
actions:
  - input: input#username
    value: aaaa
  - input: input#password
    value: pa$$w0rd
  - click: button#login-btn
  - persist: user-aaaa
```

You can later reuse this saved state to test the site as if you are already logged in by [specifying the state name][test-suite-state] in your test suite:

```yaml wrap
state: user-aaaa
```

State names must be non-empty portable filename components no longer than 80
bytes. State files are written under the active Regressify output directory, so
`--site-env staging` stores them under `.regressify__staging/states/`.
When a named state is absent there, the TS workspace's `.states/<name>.json`
is read as a migration fallback. The legacy file is never modified.
Configured `state` and `restore` files are validated before Chromium launches;
missing or malformed state files fail the command instead of silently using an
empty session. State updates are written atomically so interrupted persistence
cannot publish a partial JSON file.

{% end %}

{% spoiler() %}

press

:::::

Press a key (or key combination) on an element.

Supported modifier names are `Ctrl`/`Control`, `Alt`/`Option`, `Shift`, and `Meta`/`Cmd`/`Command`. A key combination may contain one non-modifier key, such as `Ctrl+A`, `Shift+Tab`, or `Meta+K`.
`ControlOrMeta` selects Control on Windows/Linux and Meta on macOS. Standalone
modifiers, literal `+` (or `Control++`), physical codes such as `KeyA`, and
function keys are supported. Modifiers are pressed and released around the key.

> Important:
>
> Key-triggered document navigation is awaited automatically. Use an explicit
> wait for delayed application transitions or SPA readiness.

Example:

```yml
actions:
  # Select all text inside a textarea with `message` ID
  - press: 'textarea#message'
    key: Ctrl+A
```

{% end %}

{% spoiler() %}

remove

:::::

Remove an element - set `display: none !important`.

Example:

```yml
actions:
  # Remove a `div` element with class `.random-content`
  - remove: div.random-content
```

{% end %}

{% spoiler() %}

save

:::::

Capture an intermediate full-page screenshot before the scenario's final
capture. Names must be non-empty portable filename components no longer than
100 bytes; path separators, reserved Windows filename characters, and the
`_diff` suffix are rejected. Reports list saved screenshots in action order
before that scenario's final capture targets.

Example:

```yml
actions:
  - save: after-login
```

{% end %}

{% spoiler() %}

scroll

:::::

Scroll to an element.

Example:

```yml
actions:
  # Scroll to an element with `.report` class
  - scroll: .report
```

{% end %}

{% spoiler() %}

select

:::::

Select an option inside a `select` element.

Example:

Let say you have following HTML:

```html
<select name="method" multiple>
  <option value="email">Send via email</option>
  <option value="sms">Send via SMS</option>
  <option value="skype">Send via Skype</option>
</select>
```

Then you can create select actions like following sample:

```yml
actions:
  # Select the option with `email` value
  - select: select[name="method"]
    value: email

  # Select the option with `Send via email` label
  - select: select[name="method"]
    label: Send via email

  # Select options with `sms` or `skype` value
  - select: select[name="method"]
    value:
      - sms
      - skype
```

> Notes:
>
> The `select` action requires either `value` or `label` to be set, but not both. Omit or include both of them will leading to an error.
> The selected value or label must match an existing option. Selecting multiple values requires a `<select multiple>` element.

{% end %}

{% spoiler() %}

uncheck

:::::

Uncheck a checkbox element. A checked radio button cannot be cleared through a
real user click; select another radio in its group instead.

Example:

```yml
actions:
  # Uncheck the checkbox named `subscribe-email`
  - uncheck: checkbox[name="subscribe-email"]
```

{% end %}

{% spoiler() %}

wait

:::::

Wait for X milliseconds or wait for an element to exist and become visible.

URL patterns also support brace alternatives, for example
`**/{login,dashboard}?tab=*`. A `?` is a literal query delimiter, not a wildcard.
Select actions retry missing or disabled options until `elementTimeout` expires.
If there is navigation, add the URL pattern of the target page also.

In `test` mode, URL replacements from the active replacement profile and suite/workspace `urlReplacements` are applied to `url` before matching. In `ref` mode, wait URLs are matched as written.
URL patterns support `*` within a path segment, `**` across path depth, and
wildcards in query values.

> Important:
>
> Within a scenario, whenever there is an action that leads to a navigation to other URL, please ensure that the very next action wait for that URL to fully load before performing any other steps.

Example:

```yml
actions:
  # Delay 2 seconds
  - wait: 2000

  # Wait for an element with `popup` class to appear
  - wait: .popup

  # Wait for the navigation to `/quote`, then delay 2 seconds
  # `**` matches any path depth
  - wait: 2000
    url: '**/quote'

  # Wait for the navigation to `/quote`, then wait for `#my-form` element to appear
  # `*` matches within one path segment; `**` matches any path depth
  - wait: '#my-form'
    url: '**/quote'
```

{% end %}

## Shared properties

The following properties are applicable to all actions.

{% spoiler() %}

frame

:::::

Type: `string | string[]`.  
Default: empty.

This property specifies the selector for the target iframe within which the current action should be executed.
Frames wait for the configured element timeout while loading, must be visible and
same-origin, and an array must contain at least one selector. Regressify checks
every parent frame boundary for overlays before interacting with a nested target.

Example:

```yaml
actions:
  # Click `#submit` element inside the `.child` iframe
  - click: '#submit'
    frame: .child
  
  # Click `#submit` element inside the `#grandchild` iframe
  # `grandchild` is an element within the `.child` iframe
  - click: '#submit'
    frame:
      - .child
      - '#grandchild'
```

{% end %}

[test-suite-state]: @/optimizely-cms/testing/create-test-suite.md#spoiler--state
