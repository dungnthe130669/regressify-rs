---
title: Create a Test Suite for an Entire Site
description: Generate a Regressify starter suite for an entire site by crawling its sitemap and exporting the discovered URLs.
guid: 26fbe005-872f-4e76-8db6-a6fecadeb8a6
date: 2023-08-07T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
draft: false
hidden: false
weight: 90000
sort_by: weight
template: docs/page.html

extra:
  lead: Use this workflow when a site already exposes a sitemap and you want a starter suite without copying URLs by hand until your patience resigns. The browser script exports the discovered pages into a file you can trim, organize, and adapt for real testing.
  toc: true
  top: false
---

If the site already publishes a sitemap, this approach gives you a practical starting point faster than building a long URL list manually.

{% info() %}

The script below works only for websites with a sitemap and extracts the URLs listed there. Make sure the site has a sitemap before you try it.

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

The steps above work well for sites whose sitemaps contain up to hundreds of thousands of URLs. If the site runs into the millions or beyond, you will need a more specialized crawler and collection workflow.
