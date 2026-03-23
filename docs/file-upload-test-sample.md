---
title: 'Sample: File Upload'
description: How to test file upload functionality in visual regression tests.
---

## Use case

Test file upload inputs — both single and multiple file uploads.

## Test suite

Create a file named **visual_tests/file-upload.tests.yaml**:

```yaml
scenarios:
  - url: https://example.com/upload-form
    label: Upload files
    actions:
      # Upload single file (path relative to visual_tests/)
      - input: '#my-document'
        file: alloy.tests.yaml

      # Upload multiple files
      - input: '#my-documents'
        file:
          - alloy.tests.yaml
          - color-blender.tests.yaml
```

If the upload doesn't work, try adding `useFileChooser: true`:

```yaml
actions:
  - input: '#my-documents-selector'
    file:
      - alloy.tests.yaml
      - samples/file.pdf
    useFileChooser: true
```

## Verify

```bash
# Capture reference screenshots
regressify ref --test-suite file-upload

# Run tests and compare against reference
regressify test --test-suite file-upload
```

If you need to update the reference after verifying the test results:

```bash
regressify approve --test-suite file-upload
```
