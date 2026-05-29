---
title: 'Draft: Frequently Changed Data Sample'
description: Draft sample page for testing fast-changing chart data in Regressify, with guidance on mixing manual checks and automated coverage.
guid: fc8c72ef-a715-44e5-a974-9aa93d11ec16
date: 2023-07-02T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
draft: true
hidden: false
weight: 99000
sort_by: weight
template: docs/page.html
styles:
  - assets/css/b-frequently-changed-data-chart-critical.css

extra:
  toc: true
  top: false
---

This draft sample uses a chart that changes on reload to show why some pages can make screenshot tests sulk if you treat every pixel as stable.

## Use case

Consider the following sample chart:

```html injected
<div id="my-chart-container">
  <canvas id="my-chart"></canvas>
</div>
```

{{ load_asset(path='assets/js/random-data-chart.js', module=true, async=true) }}

If you reload this page, you will notice that it displays completely different data. This makes it challenging to perform visual regression testing using an automation tool. Although the chart in this page is just a sample, in real-world scenarios, there are various instances of frequently changed data, such as an auto-play video, auto-slide carousel, ads, [stock index][stock-index], [weather forecast][weather-forecast], and more.

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

Execute the following commands to verify the test suite:

```bash
regressify ref --test-suite frequently-changed-data
regressify approve --test-suite frequently-changed-data
regressify test --test-suite frequently-changed-data
```

## Conclusion

By combining manual testing and automation techniques, you can effectively handle scenarios involving frequently changed data in your test suites. This approach ensures reliable and accurate test results while accommodating dynamic data changes.

[stock-index]: https://www.nasdaq.com/market-activity/index/ndx
[weather-forecast]: https://www.accuweather.com/en/vn/hanoi/353412/weather-forecast/353412
