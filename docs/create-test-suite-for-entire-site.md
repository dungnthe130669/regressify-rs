---
title: Creating a Test Suite for the entire site
description: Follow these steps to create a test suite for the entire site. You can use the provided file as a template and customize it according to your requirements.
guid: 26fbe005-872f-4e76-8db6-a6fecadeb8a6
date: 2023-08-07T17:00:00+00:00
updated: 2024-12-10T17:00:00+00:00
draft: false
hidden: false
weight: 90000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

Follow these steps to create a test suite for the entire site. You can use the provided file as a template and customize it according to your requirements.

{% info() %}

Please note that the provided script below works only for websites with a sitemap and extracts URLs listed within that sitemap. Ensure your site has a sitemap for the script to function correctly.

{% end %}

1. Copy the the following script to your clipboard:

   ```text h200 wrap trim
   (() => {
   /* @include ./_sitemap-crawler.js */
   })();
   ```

1. Go to the website you want to test (e.g., the site you are currently on).

   > Please use ***Chrome*** or ***Edge*** browser, as the technologies used in the script are not available on other browsers yet.

1. Open the **Developer Tools** in your browser, and navigate to the **Console** tab.

1. Paste the copied script into the **Console** and press <kbd>Enter</kbd> to execute it.

1. The browser will prompt you to save the test suite. Choose your preferred folder and rename the file as needed.

1. Keep the browser and the current page open, wait until you see following message in the **Console**:

   > FINISH! - `N` URLs found.

1. Open the saved file and modify it as per your specific testing needs.

Please note that the steps outlined above are tailored for websites with sitemaps containing up to hundreds of thousands of URLs. If the website being tested comprises millions or billions of pages, specialized tools are required to gather all the necessary information. Feel free to contact me directly if you require assistance in such cases.
