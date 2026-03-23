---
title: Create a test suite
guid: 4e46191b-a1e4-4d2d-82ff-5ec3827a33cb
description: Learn about creating effective test suites for your projects. Understand how to group test scenarios, choose their scope, and benefit from customizable properties. Explore real-world examples, including migrating websites, and gain insights into capturing screenshots and enhancing test cases with additional properties.
date: 2023-06-29T17:00:00+00:00
updated: 2026-01-13T17:00:00+00:00
draft: false
hidden: false
weight: 20000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

A test suite is a collection of [test scenarios][create-test-scenario] that are grouped together based on your decision. You have the flexibility to determine which scenarios should be included in the same test suite. For example, a test suite can encompass test cases for the entire Home page, including different blocks, or focus solely on the Header block with various states (e.g., closed/opened menu).

In one of my projects, the objective was to assist customers in migrating multiple websites from an old platform to a newer one. To ensure that everything was functioning correctly, I created dedicated test suites for each website. This allowed the QA engineer to test any individual website as needed.

At a minimum, a test suite contains a list of URLs to be tested. For example:

```yaml
scenarios:
  - url: https://alloy.eshn.dev/pages/home.html
  - url: https://alloy.eshn.dev/pages/service.html
  - url: https://alloy.eshn.dev/pages/portfolio.html
  - url: https://alloy.eshn.dev/pages/contact/default.html
```

The purpose of these tests is to load the specified URLs and capture screenshots. For more complex test cases, you can utilize a combination of the properties provided below.

## Properties

### Required properties

{% spoiler() %}

scenarios

:::::

Type: `ScenarioModel[]`.

List of test scenarios. Refer to [create a scenario](@/optimizely-cms/testing/create-scenario.md) for more information.

{% end %}

### Optional properties

{% spoiler() %}

asyncCaptureLimit

:::::

Type: `number`.  
Default value: `5`.

This parameter determines the maximum number of screens captured in parallel during testing. Increasing this value can accelerate the execution of your test suite. However, it's important to note that if your network connections or your website's capacity to handle concurrent requests is limited, raising this limit too high could result in less stable test results.

{% end %}

{% spoiler() %}

asyncCompareLimit

:::::

Type: `number`.  
Default value: `50`.

This parameter dictates the maximum number of screens that can be compared in parallel. Unlike `asyncCaptureLimit`, this value isn't dependent on your network connections or your website's capacity. Instead, it's contingent on your computer's hardware specifications. If your computer boasts robust hardware capabilities, increasing this value will result in quicker comparisons between reference and test results.

{% end %}

{% spoiler() %}

browser

:::::

Type: `string`.  
Default value: `chromium`.  
Allowed values: `chromium`, `firefox`, `webkit`.

The browser used for the test.

{% end %}

{% spoiler() %}

debug

:::::

Type: `boolean`.  
Default value: `false`.

Should the tool displays the Chrome window as tests are running. This will allow you to visually see your app state at the time of your test.

Example:

```yaml
debug: true
```

{% end %}

{% spoiler() %}

cssOverridePath

:::::

Type: `string`.

CSS override path.  
Default value: **common/_override.css**

{% end %}

{% spoiler() %}

hideSelectors

:::::

Type: `string[]`.  
Default value: `[]`.

Array of selectors set to `visibility: hidden`.  
This property will affects all the scenarios within the same test suite.

{% end %}

{% spoiler() %}

ignoreSslErrors

:::::

Type: `boolean`.  
Default value: `false`.

Whether the tool should ignore SSL errors.

{% end %}

{% spoiler() %}

misMatchThreshold

:::::

Type: `number`.  
Default value: `0.1`.  
Min value: `0`.  
Max value: `100`.

This parameter represents the percentage of permissible differing pixels for a test to pass. A lower value ensures more precise test results. The choice of a suitable value depends entirely on your project's specific requirements.

{% end %}

{% spoiler() %}

postInteractionWait

:::::

Type: `number`.  
Default value: `1000`.  

Wait for X miliseconds prior to screen shot.

{% end %}

{% spoiler() %}

removeSelectors

:::::

Type: `string[]`.  
Default value: `[]`.

Array of selectors set to `display: none`.  
This property will affects all the scenarios within the same test suite.

{% end %}

{% spoiler() %}

state

:::::

Type: `string | string[]`.  
Default value: Empty.

Loads a previously saved browser state before running tests. This should be used in conjunction with the [persist action][action-persist].

Example:

```yaml wrap
state: user-aaaa
```

{% end %}

{% spoiler() %}

urlReplacements

:::::

Type: `ReplacementModel[]`.  
Default value: `[]`.

Find/replace for the URLs in `test` command.

Example:

```yaml
# Below settings have no effect when running under `ref` command.
# When running using `test` command,
# replace the URL from `https://legacy1.domain.com/` to `https://new1.domain.com/`,
# and from `https://legacy2.domain.com/` to `https://new2.domain.com/`
urlReplacements:
  - ref: https://legacy1.domain.com/
    test: https://new1.domain.com/
  - ref: https://legacy2.domain.com/
    test: https://new2.domain.com/
```

{% end %}

{% spoiler() %}

useCssOverride

:::::

Type: `boolean`.  
Default value: `true`.

Should the scenarios use CSS override.

{% end %}

{% spoiler() %}

viewportNames

:::::

Type: `string | string[]`.  
Default value: all available viewports.

Choose the viewports you want applied for the test suite.

Sample:

```yaml hl_lines=4-6
viewportNames:
  - tablet
  - desktop
```

{% end %}

[create-test-scenario]: @/optimizely-cms/testing/create-scenario.md
[action-persist]: @/optimizely-cms/testing/create-action.md#spoiler--persist
