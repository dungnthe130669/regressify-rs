---
title: Creating a Test Suite for the entire site
description: How to create a test suite that covers every page of a website using its sitemap.
---

## Overview

To create a test suite for an entire site, you need a list of all URLs. If the site has a sitemap, you can extract URLs from it.

### Steps

1. Get all URLs from the sitemap (manually or via a script).

2. Create a test suite YAML file with each URL as a scenario:

   ```yaml
   scenarios:
     - url: https://example.com/
     - url: https://example.com/about
     - url: https://example.com/contact
     # ... add all URLs from sitemap
   ```

3. Save as **visual_tests/entire-site.tests.yaml**.

4. Run the tests:

   ```bash
   # Capture reference screenshots
   regressify ref --test-suite entire-site

   # Run tests and compare
   regressify test --test-suite entire-site
   ```

> **Tip:** For sites with dynamic content, use `hideSelectors` or `removeSelectors` to exclude frequently changing areas from the comparison.
