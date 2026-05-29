---
title: Create a Test Scenario
description: Learn how to define Regressify test scenarios, set target URLs, add actions, and tune per-scenario options for reliable runs.
guid: 620b9098-627b-4db5-bb0b-cd43b9949e35
date: 2023-06-29T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
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
| hide    | Hide an element - set `visibility: hidden !important`    |
| hover   | Hover an element                                         |
| input   | Type text in an element or upload files                  |
| press   | Press a key (or key combination) on an element           |
| remove  | Remove an element - set `display: none !important`       |
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

If the `actions` property is set, the following properties will be ignored:

- `clickSelector`
- `clickSelectors`
- `hoverSelector`
- `hoverSelectors`
- `keyPressSelector`
- `keyPressSelectors`
- `scrollToSelector`
- `postInteractionWait`

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
Default value: Test suite's value.

Import cookies in YAML format.

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
Recommend using only lowercase alphanumeric characters (`a-z`, `0-9`), minus (`-`), and underscore (`_`).

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

{% end %}

{% spoiler() %}

postInteractionWait

:::::

Type: `number`.  
Default value: Test suite's value.  

Wait for X milliseconds before taking the screenshot.

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

### Obsoleted properties

{% spoiler(danger=true) %}

clickSelector - *obsoleted*

:::::

> OBSOLETED - use [actions.click][action-click] instead.

Type: `string`.  
Default value: Empty.

Click the specified DOM element prior to screen shot.  
Notes: this property will be ignored if the [actions](#spoiler--actions) property is set.

{% end %}

{% spoiler(danger=true) %}

clickSelectors - *obsoleted*

:::::

> OBSOLETED - use [actions.click][action-click] instead.

Type: `string[]`.  
Default value: `[]`.

Click the specified DOM elements prior to screen shot.  
Notes: this property will be ignored if the [actions](#spoiler--actions) property is set.

{% end %}

{% spoiler(danger=true) %}

hideSelectors - *obsoleted*

:::::

> OBSOLETED - use [actions.hide][action-hide] instead.

Type: `string[]`.  
Default value: Test suite's value.

Array of selectors set to `visibility: hidden`  
Notes: this property will be ignored if the [actions](#spoiler--actions) property is set.

{% end %}

{% spoiler(danger=true) %}

removeSelectors - *obsoleted*

:::::

> OBSOLETED - use [actions.remove][action-remove] instead.

Type: `string[]`.  
Default value: Test suite's value.

Array of selectors set to `display: none`.  
Notes: this property will be ignored if the [actions](#spoiler--actions) property is set.

{% end %}

[create-an-action]: @/optimizely-cms/testing/create-action.md
[action-click]: @/optimizely-cms/testing/create-action.md#spoiler--click
[action-hide]: @/optimizely-cms/testing/create-action.md#spoiler--hide
[action-remove]: @/optimizely-cms/testing/create-action.md#spoiler--remove
