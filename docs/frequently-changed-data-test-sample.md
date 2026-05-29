---
title: 'Sample: Frequently Changed Data'
description: Learn how to test pages with frequently changing data in Regressify by separating manual checks from stable automated coverage.
guid: 7f825e07-f19c-47ad-9898-fcff7d0ad705
date: 2023-06-30T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
draft: false
hidden: false
weight: 81000
sort_by: weight
template: docs/page.html
styles:
  - assets/css/b-frequently-changed-data-chart-critical.css

extra:
  lead: This sample explains how to deal with pages that refuse to sit still, such as charts, carousels, ads, or forecast widgets. The goal is not to outsmart the moving parts, but to isolate them so the rest of the page can still be tested reliably.
  toc: true
  top: false
---

This sample focuses on pages that change even when your code did not, which is a charming trait right up until screenshots start failing.

## Use case

Consider the following sample chart:

{{ div(id='chart-container', class='chart-container') }}

{{ load_asset(path='assets/vendors/charts/random-data-chart.js', module=true, async=true) }}

If you reload this page, you will notice that it displays completely different data. This makes it challenging to perform visual regression testing using an automation tool. Although the chart in this page is just a sample, in real-world scenarios, there are various instances of frequently changed data, such as an auto-play video, auto-slide carousel, ads, exchange rates, stock index, weather forecast, and more.

## Approach

To address scenarios involving frequently changed data, I suggest a combination of manual and automated testing. Follow the steps below:

### Manual Testing

A QA engineer should perform an initial manual test on the chart (or any part with frequently changed data). This allows for verification and validation against the expected data.

### Automated Testing

After the manual test is completed successfully, the remaining parts of the page can be tested using an automation tool.

However, the section with frequently changed data should be hidden from view during the automated tests. This ensures that test results remain consistent and unaffected by dynamic data changes.

## Test suite

To create the test suite for frequently changed data, follow these steps:

1. Create a file named **visual_tests/frequently-changed-data.tests.yaml** in your test project.

1. Add the following content to the file:

   ```yml
   #* @include ./_frequently-changed-data.tests.yaml *#
   ```

Please note that ***the last scenario in the test suite will intentionally fail***. You can visually inspect the results after running the test to identify the reason for the failure.

## Verify

Execute the following command to verify the test suite:

```bash
regressify ref --test-suite frequently-changed-data
regressify approve --test-suite frequently-changed-data
regressify test --test-suite frequently-changed-data
```

## Conclusion

By combining manual testing and automation techniques, you can effectively handle scenarios involving frequently changed data in your test suites. This approach ensures reliable and accurate test results while accommodating dynamic data changes.
