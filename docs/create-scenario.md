---
title: Create a scenario
description: Create and configure test scenarios for Regressify CLI visual regression testing.
---

## Properties

### Required properties

#### url

Type: `string`.

The URL to test. A headless browser will navigate to this URL and capture a screenshot.

```yaml
scenarios:
  - url: https://example.com/
```

### Optional properties

#### actions

Type: array of actions.  
Default value: `[]`.

A list of actions to execute sequentially before capturing the final screenshot. Available actions:

| Action  | Use for                                             |
| ------- | --------------------------------------------------- |
| check   | Check a radio or checkbox                           |
| click   | Click an element                                    |
| focus   | Focus an element                                    |
| goto    | Navigate to a different URL                         |
| hide    | Hide an element (`visibility: hidden !important`)   |
| hover   | Hover an element                                    |
| input   | Type text in an element or upload files             |
| persist | Save browser state (cookies + localStorage) to disk |
| press   | Press a key (or key combination) on an element      |
| remove  | Remove an element (`display: none !important`)      |
| save    | Take an intermediate screenshot                     |
| scroll  | Scroll to an element                                |
| select  | Select an option inside a `<select>` element        |
| uncheck | Uncheck a radio or checkbox                         |
| wait    | Wait for milliseconds, an element, or a navigation  |

See [Actions Reference](actions.md) for full details.

Example:

```yaml
actions:
  - check: checkbox#tos-accept
  - click: button.submit
  - focus: '#username'
  - hide: '#cookie-modal'
  - hover: '#my-chart'
  - input: 'input#username'
    value: My Name
  - press: '#text-area'
    key: Ctrl+A
  - remove: div.random-content
  - scroll: .report
  - uncheck: checkbox[name="subscribe-email"]
  - wait: 1000
  - wait: .popup
```

#### cookiePath

Type: `string`.  
Default value: none.

Path to a YAML file containing cookies to load before running this scenario.

#### cssOverridePath

Type: `string`.  
Default value: Test suite's value.

Path to a CSS override file for this specific scenario.

#### delay

Type: `number`.  
Default value: none.

Wait for X milliseconds after all actions are executed (and after `postInteractionWait`), right before capturing the final screenshot. Useful for pages that need extra time to fully render.

```yaml
scenarios:
  - url: https://example.com/slow-page
    delay: 3000
```

#### hideSelectors

Type: `string[]`.  
Default value: none (inherits from test suite).

Array of CSS selectors set to `visibility: hidden !important` before screenshots. Applied in addition to the test suite's `hideSelectors`.

#### id

Type: `string`.  
Default value: auto-generated from `label` or URL hash.

Unique identifier for this scenario. Required when using the [needs](#needs) property in other scenarios.

```yaml
scenarios:
  - url: https://example.com
    id: first-visit
```

#### label

Type: `string`.  
Default value: auto-generated from URL.

Human-readable name for the scenario. Used for identification in reports and as the basis for auto-generated IDs.

```yaml
scenarios:
  - url: https://example.com
    label: check-login-with-valid-credentials
```

#### misMatchThreshold

Type: `number`.  
Default value: Test suite's value.  
Range: `0` – `100`.

Override the test suite's mismatch threshold for this specific scenario.

#### needs

Type: `string | string[]`.  
Default value: none.

List of scenario IDs whose actions should be executed before this scenario's own actions. This enables action reuse across scenarios — each scenario still opens a fresh browser page, but the prerequisite actions are replayed first.

```yaml
scenarios:
  - url: https://example.com/form
    id: fill-username
    actions:
      - input: '#username'
        value: testuser

  - url: https://example.com/form
    id: fill-password
    needs: fill-username
    actions:
      - input: '#password'
        value: Pa$$w0rd

  - url: https://example.com/form
    label: Submit form
    needs: fill-password
    actions:
      - click: button[type="submit"]
      - wait: 3000
        url: '**/success'
```

Dependencies are resolved recursively — if scenario C `needs: B` and B `needs: A`, then C will execute A's actions, then B's actions, then its own.

#### noScrollTop

Type: `boolean`.  
Default value: `false`.

When `false` (default), the page is scrolled to the top before capturing the final screenshot. Set to `true` to disable this behavior.

#### postInteractionWait

Type: `number`.  
Default value: Test suite's value.

Wait for X milliseconds after all actions are executed, before the delay and screenshot. Overrides the test suite's value if set.

#### removeSelectors

Type: `string[]`.  
Default value: none (inherits from test suite).

Array of CSS selectors set to `display: none !important` before screenshots. Applied in addition to the test suite's `removeSelectors`.

#### useCssOverride

Type: `boolean`.  
Default value: `true`.

Whether to inject the CSS override file for this scenario.

#### viewportNames

Type: `string | string[]`.  
Default value: all available viewports.

> **Note:** Viewport support is planned but not yet implemented. Currently all tests run at the default 1920×1080 viewport.
