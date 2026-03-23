---
title: 'Sample: Form Submission'
description: Sample Visual Regression Test Suite for verifying form submission functionality.
---

## Use case

Test a sign-up form with validation rules (username length, email format, password complexity, confirm password matching).

## Test suite

Create a file named **visual_tests/form-submission.tests.yaml** in your test project.

## Verify

```bash
# Capture reference screenshots
regressify ref --test-suite form-submission

# Run tests and compare against reference
regressify test --test-suite form-submission
```

If you need to update the reference after verifying the test results:

```bash
regressify approve --test-suite form-submission
```
