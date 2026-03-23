---
title: 'Sample: File Upload'
guid: c63de907-cf4c-431a-b815-9b2d7d1b1117
date: 2023-08-11T17:00:00+00:00
updated: 2025-09-03T17:00:00+00:00
draft: false
hidden: false
weight: 85000
sort_by: weight
template: docs/page.html
styles:
  - assets/css/b-upload-file-test-sample-critical.css

extra:
  toc: true
  top: false
---

Consider following sample form. We will practice the upload file actions with below test suite.

```html injected
<form class="t-o-upload-file-test-sample">
  <div class="t-o-upload-file-test-sample__item">
    <label for="my-document">Upload single file:</label>
    <br>
    <input type="file" id="my-document">
  </div>
  <div class="t-o-upload-file-test-sample__item">
    <label for="my-documents">Upload multiple files:</label>
    <br>
    <input type="file" id="my-documents" multiple>
  </div>
  <div class="t-o-upload-file-test-sample__item">
    <label for="my-documents-file-chooser">Special case using File Chooser:</label>
    <br>
    <button type="button" id="my-documents-selector">Upload Files using File Chooser</button>
    <input type="file" id="my-documents-file-chooser" multiple>
  </div>
  <div id="my-documents-list"></div>
</form>
```

{{ load_asset(path='assets/js/sample-file-upload.js', module=true, async=true) }}

## Test suite

To create the test suite, follow these steps:

1. Create a file named **visual_tests/file-upload.tests.yaml** in your test project.

1. Add the following content to the file:

   ```yaml h500
   scenarios:
     - url: https://tuyen.blog/optimizely-cms/testing/file-upload-test-sample/
       label: Upload files
       actions:
         # Upload single file
         - input: '#my-document'
           file: alloy.tests.yaml

         # Upload multiple files
         - input: '#my-documents'
           file:
             - alloy.tests.yaml
             - color-blender.tests.yaml

     # - url: https://tuyen.blog/optimizely-cms/testing/file-upload-test-sample/
     #   label: Upload multiple files for single file input
     #   actions:
     #     - input: '#my-document'
     #       file:
     #         - alloy.tests.yaml
     #         - color-blender.tests.yaml
   ```

## Verify

Execute the following command to verify the test suite:

```bash
regressify ref --test-suite file-upload
regressify approve --test-suite file-upload
regressify test --test-suite file-upload
```

## Notes

If the upload file action doesn't work, try add `useFileChooser` property as in sample below:

```yaml
scenarios:
  - url: https://tuyen.blog/optimizely-cms/testing/file-upload-test-sample/
    label: Upload file
    actions:
      - input: '#my-documents-selector'
        file:
          - alloy.tests.yaml
          - samples/file.pdf
        # Only use below property if the scenario
        # doesn't work without it
        useFileChooser: true
```
