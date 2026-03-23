---
title: 'Sample: Reuse Scenarios'
guid: 81daa4ed-2201-489f-9b96-ac30284dcd8d
date: 2023-08-30T17:00:00+00:00
updated: 2025-01-10T17:00:00+00:00
draft: false
hidden: false
weight: 86000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

Consider the previous [sample form](@/optimizely-cms/testing/form-submission-test-sample.md). This time we will write a test suite in re-usable way.

## Test suite

To create the test suite, follow these steps:

1. Create a file named **visual_tests/form-reuse-scenarios.tests.yaml** in your test project.

1. Add the following content to the file:

   ```yaml h500
   #* @include ./_reuse-scenarios.tests.yaml *#
   ```

## Verify

Execute the following command to verify the test suite:

```bash
regressify ref --test-suite form-reuse-scenarios
regressify approve --test-suite form-reuse-scenarios
regressify test --test-suite form-reuse-scenarios
```
