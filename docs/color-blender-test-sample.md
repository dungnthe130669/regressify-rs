---
title: 'Sample: Color Blender'
description: Use this sample Regressify suite to verify the Color Blender page and confirm the core interactions still render correctly.
guid: 01e7e0de-c848-4a1a-b6b8-334b656c77e2
date: 2023-06-29T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
draft: false
hidden: false
weight: 80000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

This sample keeps things simple: one page, one known target, and a quick way to confirm your test setup is behaving.

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
