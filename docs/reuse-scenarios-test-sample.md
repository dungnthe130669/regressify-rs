---
title: 'Sample: Reuse Scenarios'
description: Learn how to reuse shared scenario pieces in a Regressify test suite so repeated form actions stay easier to maintain.
guid: 81daa4ed-2201-489f-9b96-ac30284dcd8d
date: 2023-08-30T17:00:00+00:00
updated: 2026-10-05T00:00:00+00:00
draft: false
hidden: false
weight: 86000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

Start with the earlier [sample form](@/optimizely-cms/testing/form-submission-test-sample.md). This time, the goal is to write the suite in a reusable way so repeated steps do not multiply like gremlins.

## Test suite

To create the test suite, follow these steps:

1. Create a file named **visual_tests/form-reuse-scenarios.tests.yaml** in your test project.

1. Add the following content to the file:

   ```yaml h500
   #* @include ./_reuse-scenarios.tests.yaml *#
   ```

## Viewport-Scoped Prerequisites

Actions inherited through `needs` keep their own `viewportNames` filters and
execute against the consuming scenario's current viewport. The prerequisite's
scenario selection does not replace the consumer's capture jobs.

```yaml
scenarios:
  - id: open-menu
    url: https://example.com
    viewportNames: [desktop, mobile]
    actions:
      - click: '#menu-toggle'
        viewportNames: [mobile]
  - id: products
    url: https://example.com
    viewportNames: [desktop, mobile]
    needs: open-menu
    actions:
      - click: '#products-menu'
```

Both labels must exist in the viewport file. Every explicit action filter in the
full prerequisite chain must belong to the consumer's effective set: changing
`products` to `[desktop]` makes the inherited `[mobile]` filter a pre-browser
configuration error. Split common and viewport-specific prerequisites when a
consumer needs a narrower selection. Omitted, `null`, and `[]` action filters
remain shared, and inherited saves are planned only on matching viewports.

`needs` is action replay, not ordering between concurrent scenario jobs. Review
the [persist/state limitation](create-action.md#viewport-selection) before scoping
a `persist` name that another scenario loads through `state` or `restore`.

## Verify

Execute the following command to verify the test suite:

```bash
regressify ref --test-suite form-reuse-scenarios
regressify approve --test-suite form-reuse-scenarios
regressify test --test-suite form-reuse-scenarios
```
