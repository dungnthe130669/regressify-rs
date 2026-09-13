---
title: Create a Test Scenario
description: Learn how to define Regressify test scenarios, set target URLs, add actions, and tune per-scenario options for reliable runs.
guid: 620b9098-627b-4db5-bb0b-cd43b9949e35
date: 2023-06-29T17:00:00+00:00
updated: 2026-09-13T00:00:00+00:00
draft: false
hidden: false
weight: 30000
sort_by: weight
template: docs/page.html

extra:
  lead: A scenario tells Regressify which page to open, what to do before capture, and which settings apply only to that run. Use this reference when a suite needs more than a plain list of URLs and starts behaving like an actual test case.
  toc: true
  top: false
---

Each scenario describes one path through a page, from the first URL load to any clicks, waits, or overrides needed before the screenshot is captured.

## Properties

### Required properties

{% spoiler() %}

url

:::::

Type: `string`.

The URL you want to test.
Initial navigation, including the reload used after restoring local storage, is
bounded by the workspace `elementTimeout` setting before actions begin.

{% end %}

### Optional properties

{% spoiler() %}

actions

:::::

Type: array of actions.  
Default value: `[]`.

Provide the list of actions that the test browser should execute in sequence. Available options are:

| Action  | Use for                                                  |
| ------- | -------------------------------------------------------- |
| check   | Check a radio or checkbox                                |
| click   | Click an element                                         |
| focus   | Focus an element                                         |
| goto    | Navigate to a URL                                        |
| hide    | Hide an element - set `visibility: hidden !important`    |
| hover   | Hover an element                                         |
| input   | Type text in an element or upload files                  |
| persist | Save cookies and local storage for later scenarios       |
| press   | Press a key (or key combination) on an element           |
| remove  | Remove an element - set `display: none !important`       |
| save    | Capture an intermediate screenshot                       |
| scroll  | Scroll to an element                                     |
| select  | Select an option inside a `select` element               |
| uncheck | Uncheck a radio or checkbox                              |
| wait    | Wait for X milliseconds or wait for an element to appear |

For more information, please refer to [Create an action][create-an-action].

A scenario can have multiple actions. Sample:

```yml
actions:

  # Check the checkbox with `tos-accept` ID
  - check: checkbox#tos-accept

  # Click the button with `submit` class
  - click: button.submit

  # Focus an element with `username` ID
  - focus: '#username'

  # Hide the element with `cookie-modal` ID
  - hide: '#cookie-modal'

  # Hover mouse over an element contains `my-chart` ID
  - hover: '#my-chart'

  # Type `My Name` in an input element with `username` ID
  - input: 'input#username'
    value: My Name
    append: false

  # Type `my-name@domain.com` in an input element with `email` ID
  - input: 'input#email'
    value: my-name@domain.com
    append: false

  # Select all content inside an element with `text-area` ID
  - press: '#text-area'
    key: Ctrl+A

  # Remove a `div` element with `random-content` class
  - remove: div.random-content

  # Scroll to an element with `report` class
  - scroll: .report

  # Uncheck the checkbox named `subscribe-email`
  - uncheck: checkbox[name="subscribe-email"]

  # Delay 1 second
  - wait: 1000

  # Wait for an element with `popup` class appears
  - wait: .popup
```

Actions are executed in the order listed. If `clickSelector` is also set, it runs after the action list; prefer the explicit `click` action for new suites. `postInteractionWait` still applies after actions.

Legacy properties such as `clickSelectors`, `hoverSelector`, `hoverSelectors`, `keyPressSelector`, `keyPressSelectors`, and `scrollToSelector` are not implemented by the Rust CLI.

{% end %}

{% spoiler() %}

cssOverridePath

:::::

Type: `string`.  
Default value: Test suite's value.

{% end %}

{% spoiler() %}

cookiePath

:::::

Type: `string`.  
Default value: Test suite's value, then workspace value, then `common/_cookies.yaml`.

Import cookies in YAML format. Cookies are loaded before the first navigation
and the path must stay inside the project root. Each cookie needs a non-empty
`name`, a string `value`, and either `url` or `domain`. Optional `secure` and
`httpOnly` values must be booleans; `sameSite` is `Strict`, `Lax`, or `None`.
`SameSite: None` requires `secure: true`.

```yaml
- name: session
  value: example-token
  domain: .example.com
  path: /
  secure: true
  httpOnly: true
  sameSite: Lax
```

{% end %}

{% spoiler() %}

delay

:::::

Type: `number`.  
Default value: `1000`.

When the page is ready, wait for X milliseconds before taking the screenshot.  
Use this for scenarios where the page under test needs extra time to load or render.

{% end %}

{% spoiler() %}

id

:::::

Type: `string`.  
Default value: empty.

Scenario ID. This property is for identifying this scenario when you re-use in other one. Use any value you find meaningful, but should be as short and concise as possible.  
IDs must be a portable single path component: they cannot be empty or contain
path separators, control characters, or Windows-reserved filename characters.
Leading or trailing whitespace and trailing dots are also rejected.
Lowercase alphanumeric characters (`a-z`, `0-9`), minus (`-`), and underscore
(`_`) are recommended.

Use together with the `needs` property.

Example:

```yaml
scenarios:
  - url: https://www.domain.com
    id: first-visit
```

{% end %}

{% spoiler() %}

jsOnReadyPath

:::::

Type: `string`.  
Default value: Test suite's value.

{% end %}

{% spoiler() %}

label

:::::

Type: `string`.

Name of current scenario.  
If not set, the path of current URL will be used.  
Recommend using only lowercase alphanumeric characters (`a-z`, `0-9`), minus (`-`), and underscore (`_`).

Example:

```yaml
scenarios:
  - url: https://www.domain.com
    label: check-login-with-valid-credentials
```

{% end %}

{% spoiler() %}

misMatchThreshold

:::::

Type: `number`.  
Default value: Test suite's value.  
Min value: `0`.  
Max value: `100`.

This parameter represents the percentage of permissible differing pixels for a test to pass. A lower value ensures more precise test results. The choice of a suitable value depends entirely on your project's specific requirements.

{% end %}

{% spoiler() %}

needs

:::::

Type: `string | string[]`.  
Default value: `[]`.

List of the IDs of other scenarios needed to be executed before this scenario.

Example:

```yaml
scenarios:
  - url: https://www.domain.com
    id: open-form
    actions:
      - click: '#form-popup'

  - url: https://www.domain.com
    id: input-form
    # Execute `open-form` actions
    # before executing this scenario's actions
    needs: open-form
    actions:
      - input: input#username
        value: My Name
      - input: input#password
        value: Passw0rd!
  
  - url: https://www.domain.com
    label: Submit form
    # Execute `open-form` and `input-form` actions
    # before executing this scenario's actions
    needs: input-form
    actions:
      - click: button[type="submit"]
```

{% end %}

{% spoiler() %}

noScrollTop

:::::

Type: `boolean`.  
Default value: `false`.

Disable scrollTop prior to screen capture.

By default, Regressify scrolls the page down and back to the top before the final screenshot to trigger lazy-loaded images. Set this to `true` when that behavior is not wanted.

{% end %}

{% spoiler() %}

postInteractionWait

:::::

Type: `number`.  
Default value: Test suite's value, then workspace value, then `1000`.

Wait for X milliseconds after actions and before taking the screenshot. Set this to `0` when the scenario should capture immediately after interactions.

{% end %}

{% spoiler() %}

restore

:::::

Type: `string | string[]`.  
Default value: Test suite `state`, then workspace `state`.

Restore one or more persisted states for this scenario. Values are combined with suite/workspace `state` values and duplicates are removed while preserving order. State files are loaded from the active output directory, for example `.regressify/states/auth.json` or `.regressify__staging/states/auth.json`.

Example:

```yaml
scenarios:
  - url: https://www.domain.com/account
    restore:
      - auth
      - preferences
```

{% end %}

{% spoiler() %}

requiredLogin

:::::

Type: `boolean`.  
Default value: Workspace value, then `false`.

Marks the scenario as requiring login in the resolved scenario context. The global `--requiredLogin` flag forces this value to `true` for every scenario. The Rust CLI exposes the value to custom page scripts as `window.__REGRESSIFY__.requiredLogin` and `window.regressify.requiredLogin`; the actual login flow should still be implemented through persisted `state`, `restore`, cookie files, or explicit actions.

{% end %}

{% spoiler() %}

useCssOverride

:::::

Type: `boolean`.  
Accepted values: `true` or `false`.  
Default value: `true`.

Should the scenario use CSS override.

{% end %}

{% spoiler() %}

viewportNames

:::::

Type: `string | string[]`.  
Default value: all available viewports.

Choose the viewports you want applied for the scenario.  
If this property is set at both the scenario and test suite levels, the scenario-level setting will take precedence.
Unknown viewport names fail as configuration errors before any browser is launched.

Sample:

```yaml hl_lines=4-6
scenarios:
  - url: https://tuyen.blog/
    label: Click theme toggle on tablet and desktop
    viewportNames:
      - tablet
      - desktop
    actions:
      - click: theme-toggle
```

{% end %}

{% spoiler() %}

selectors

:::::

Type: `string | string[]`.

Capture one or more CSS selectors instead of the full page. Each target writes a
separate screenshot. Use the special value `viewport` to capture the currently
visible viewport. Omit `selectors` to retain the default full-page capture.

Targets are captured in declaration order, must be non-empty and unique, and
must resolve to a visible element when captured. Regressify restores the prior
page scroll position after each selector capture so one target does not alter a
later viewport, selector, or full-page capture. Comparison reports label each
target as `document`, `viewport`, a saved step, or a stable selector capture
name. Full-page and selector targets must resolve to finite positive dimensions
no larger than 100000 CSS pixels in either direction or 64000000 total CSS
pixels.

```yaml
scenarios:
  - url: https://www.domain.com/dashboard
    selectors:
      - viewport
      - main .summary
      - '#activity-feed'
```

{% end %}

### Obsoleted properties

{% spoiler(danger=true) %}

clickSelector - *obsoleted*

:::::

> OBSOLETED - use [actions.click][action-click] instead.

Type: `string`.  
Default value: Empty.

Click the specified DOM element prior to screen shot.  
Notes: this legacy property runs after `actions` when both are configured. Prefer the explicit `click` action for new suites.

{% end %}

{% spoiler(danger=true) %}

clickSelectors - *obsoleted*

:::::

> OBSOLETED - use [actions.click][action-click] instead.

Type: `string[]`.  
Default value: `[]`.

Click the specified DOM elements prior to screen shot.  
Notes: this legacy property is not implemented by the Rust CLI. Prefer the explicit `click` action for new suites.

{% end %}

{% spoiler(danger=true) %}

hideSelectors - *obsoleted*

:::::

> OBSOLETED - use [actions.hide][action-hide] instead.

Type: `string[]`.  
Default value: Test suite's value.

Array of selectors set to `visibility: hidden`  
Notes: this property is applied through the config cascade before capture. Prefer the explicit `hide` action when the change belongs to a specific action sequence.

{% end %}

{% spoiler(danger=true) %}

removeSelectors - *obsoleted*

:::::

> OBSOLETED - use [actions.remove][action-remove] instead.

Type: `string[]`.  
Default value: Test suite's value.

Array of selectors set to `display: none`.  
Notes: this property is applied through the config cascade before capture. Prefer the explicit `remove` action when the change belongs to a specific action sequence.

{% end %}

[create-an-action]: @/optimizely-cms/testing/create-action.md
[action-click]: @/optimizely-cms/testing/create-action.md#spoiler--click
[action-hide]: @/optimizely-cms/testing/create-action.md#spoiler--hide
[action-remove]: @/optimizely-cms/testing/create-action.md#spoiler--remove
