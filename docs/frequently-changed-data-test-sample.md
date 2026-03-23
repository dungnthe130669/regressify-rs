---
title: 'Sample: Frequently Changed Data'
description: How to handle frequently changed data (charts, ads, carousels) in visual regression tests.
---

## Use case

Some page elements display data that changes on every load — charts, ads, auto-play videos, exchange rates, etc. This makes visual regression testing unreliable for those specific areas.

## Approach

Combine manual and automated testing:

1. **Manual testing:** A QA engineer verifies the dynamic section once.
2. **Automated testing:** Hide the dynamic section using `hideSelectors` or `removeSelectors` so that automated screenshots remain stable.

## Test suite

Create a file named **visual_tests/frequently-changed-data.tests.yaml** with scenarios that hide dynamic elements:

```yaml
removeSelectors:
  - '#chart-container'
scenarios:
  - url: https://example.com/dashboard
    label: Dashboard without chart
```

## Verify

```bash
# Capture reference screenshots
regressify ref --test-suite frequently-changed-data

# Run tests and compare against reference
regressify test --test-suite frequently-changed-data
```

If you need to update the reference after verifying the test results:

```bash
regressify approve --test-suite frequently-changed-data
```
