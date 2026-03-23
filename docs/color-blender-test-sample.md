---
title: 'Sample: Color Blender'
description: Sample Visual Regression Test Suite for verifying the functionality of Color Blender page.
---

## Use case

Verify that the Color Blender page works correctly.

## Test suite

Create a file named **visual_tests/color-blender.tests.yaml**.

## Verify

Execute the following commands to verify the test suite:

```bash
# Capture reference screenshots
regressify ref --test-suite color-blender

# Run tests and compare against reference
regressify test --test-suite color-blender
```

If you need to update the reference after verifying the test results:

```bash
regressify approve --test-suite color-blender
```
