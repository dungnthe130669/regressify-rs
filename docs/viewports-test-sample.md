---
title: 'Sample: Viewports'
description: Testing different viewports (mobile, tablet, desktop) for responsive website verification.
---

## Use case

Certain sections of websites vary across mobile, tablet, and desktop viewports. For example, a header may show a hamburger menu on mobile but be fully expanded on desktop.

## Test suite

Create a file named **visual_tests/theme-toggle.tests.yaml**:

```yaml
scenarios:
  - url: https://tuyen.blog/
    label: Visit the page

  - url: https://tuyen.blog/
    label: Click theme toggle
    actions:
      - click: '#theme-toggle'

  - url: https://tuyen.blog/
    label: Click theme toggle twice
    actions:
      - click: '#theme-toggle'
      - click: '#theme-toggle'
```

> **Note:** The `viewportNames` property is planned but not yet implemented. Currently all tests run at the default 1920×1080 viewport. You can still test different viewport sizes by configuring browser launch options.

## Verify

```bash
# Capture reference screenshots
regressify ref --test-suite theme-toggle

# Run tests and compare against reference
regressify test --test-suite theme-toggle
```

If you need to update the reference after verifying the test results:

```bash
regressify approve --test-suite theme-toggle
```
