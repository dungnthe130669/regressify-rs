---
title: 'Sample: Color Blender'
description: Sample Visual Regression Test Suite for verifying the functionality of Color Blender page.
guid: 01e7e0de-c848-4a1a-b6b8-334b656c77e2
date: 2023-06-29T17:00:00+00:00
updated: 2025-01-10T17:00:00+00:00
draft: false
hidden: false
weight: 80000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

## Use case

Verify that the [Color Blender](@/utils/general/color-blender.md) page works correctly.

## Test suite

Create a file named **visual_tests/color-blender.tests.yaml**, with following content:

```yml h500
#* @include ./_color-blender.tests.yaml *#
```

## Verify

Execute the following command to verify the test suite:

```bash
regressify ref --test-suite color-blender
regressify approve --test-suite color-blender
regressify test --test-suite color-blender
```
