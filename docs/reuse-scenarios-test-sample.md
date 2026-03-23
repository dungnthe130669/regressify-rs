---
title: 'Sample: Reuse Scenarios'
description: How to reuse scenario actions across multiple test scenarios using the needs property.
---

## Use case

When testing multi-step forms, you often need to repeat the same setup actions (fill username, fill email, etc.) across different scenarios. The `needs` property lets you chain scenario actions so each scenario builds on previous ones.

## Test suite

Create a file named **visual_tests/form-reuse-scenarios.tests.yaml**. See [form-reuse-scenarios.tests.yaml](../visual_tests/form-reuse-scenarios.tests.yaml) for an example that chains form input scenarios.

## Verify

```bash
# Capture reference screenshots
regressify ref --test-suite form-reuse-scenarios

# Run tests and compare against reference
regressify test --test-suite form-reuse-scenarios
```

If you need to update the reference after verifying the test results:

```bash
regressify approve --test-suite form-reuse-scenarios
```
