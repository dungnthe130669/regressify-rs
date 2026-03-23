---
title: Create a test suite
description: Learn how to create and configure test suites for Regressify CLI visual regression testing.
---

A test suite is a collection of [test scenarios](create-scenario.md) grouped together. You decide which scenarios belong in the same suite — for example, all tests for a single page, or tests for an entire website migration.

Test suite files are placed in the `visual_tests/` directory and must be named `*.tests.yaml` (or `.tests.yml` / `.tests.json`).

At a minimum, a test suite contains a list of URLs to test:

```yaml
scenarios:
  - url: https://example.com/
  - url: https://example.com/about
  - url: https://example.com/contact
```

Each URL will be loaded in a headless Chromium browser and a full-page screenshot will be captured.

## Properties

### Required properties

#### scenarios

Type: `ScenarioModel[]`.

List of test scenarios. See [Create a scenario](create-scenario.md) for all available properties.

### Optional properties

#### cssOverridePath

Type: `string`.  
Default value: `common/_override.css`.

Path to a CSS file that will be injected into every scenario's page. Useful for hiding dynamic elements (ads, animations) globally.

#### debug

Type: `boolean`.  
Default value: `false` (overridden by `--debug` CLI flag).

When `true`, launches the browser in headed (visible) mode so you can see the test running.

```yaml
debug: true
```

#### hideSelectors

Type: `string[]`.  
Default value: `[]`.

Array of CSS selectors that will be set to `visibility: hidden !important` on every scenario's page before screenshots are captured.

```yaml
hideSelectors:
  - '#cookie-banner'
  - .ad-container
```

#### misMatchThreshold

Type: `number`.  
Default value: `0.1`.  
Range: `0` – `100`.

The percentage of permissible differing pixels for a test to pass. A lower value means stricter comparison. Can be overridden per-scenario.

```yaml
misMatchThreshold: 0.5
```

#### postInteractionWait

Type: `number`.  
Default value: none.

Wait for X milliseconds after all actions are executed, before capturing the final screenshot. Can be overridden per-scenario.

```yaml
postInteractionWait: 1000
```

#### removeSelectors

Type: `string[]`.  
Default value: `[]`.

Array of CSS selectors that will be set to `display: none !important` on every scenario's page before screenshots are captured.

```yaml
removeSelectors:
  - .random-content
  - '#auto-play-video'
```

#### state

Type: `string | string[]`.  
Default value: none.

Load a previously saved browser state (cookies + localStorage) before running tests. Used together with the [persist action](actions.md#persist).

```yaml
state: logged-in-user
```

#### urlReplacements

Type: `ReplacementModel[]`.  
Default value: `[]`.

Find/replace rules applied to scenario URLs. Useful for testing the same scenarios against different environments.

```yaml
urlReplacements:
  - ref: https://legacy.example.com/
    test: https://new.example.com/
```

#### useCssOverride

Type: `boolean`.  
Default value: `true`.

Whether to inject the CSS override file (see `cssOverridePath`).

#### viewportNames

Type: `string | string[]`.  
Default value: all available viewports.

> **Note:** Viewport support is planned but not yet implemented. Currently all tests run at the default 1920×1080 viewport.
