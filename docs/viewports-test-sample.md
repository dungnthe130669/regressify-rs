---
title: 'Sample: Viewports'
guid: 5fb24910-6e86-4eae-ba0c-28a8d8a239b3
description: Frequently, certain sections of the website exhibit variations across mobile, tablet, and desktop interfaces. When assessing the functionality of such a Header, distinct test scenarios tailored to each viewport are essential.
date: 2023-08-08T17:00:00+00:00
updated: 2025-01-10T17:00:00+00:00
draft: false
hidden: false
weight: 84000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

```css injected
main img[alt="Hamburger"],
main img[alt="Moon"],
main img[alt="Sun"] {
  margin: 0 0.5em;
  display: inline-block;
  transform: translateY(0.1em) scale(1.5);
  line-height: 1em;
  height: 1em;
  width: auto;
}

@media not print {
  [data-theme='dark'] main img[alt="Hamburger"],
  [data-theme='dark'] main img[alt="Moon"],
  [data-theme='dark'] main img[alt="Sun"] {
    filter: invert(100%);
  }
}
```

## Use case

Frequently, certain sections of the website exhibit variations across mobile, tablet, and desktop interfaces. For instance, the Header block may be entirely visible on desktop, whereas on mobile, it remains hidden until the user interacts with the ![Hamburger][hamburger] icon. When assessing the functionality of such a Header, distinct test scenarios tailored to each viewport are essential.

## Test case

Validation of Theme Toggle Button Functionality.

Objective: To confirm the proper functioning of the theme toggle button located at the upper part of this website.

1. Locate the ![Sun][sun] or ![Moon][moon] icon positioned at the top-right corner of this website.

1. On mobile devices, clicking the ![Hamburger][hamburger] icon should reveal the theme toggle icon.

1. After clicking the ![Sun][sun] icon, the site's appearance should change to **Light** mode, and the icon should be replaced by the ![Moon][moon] icon.

1. After clicking the ![Moon][moon] icon, the site's appearance should change to **Dark** mode, and the icon should be replaced by the ![Sun][sun] icon.

Note: The test assumes the presence of the ![Sun][sun] or ![Moon][moon] toggle button and its behavior as described. Any deviation from the expected behavior should be considered a defect.

## Test suite

To create the test suite, follow these steps:

1. Create a file named **visual_tests/theme-toggle.tests.yaml** in your test project.

1. Add the following content to the file:

   ```yml h500
   scenarios:
     # Visit the page on tablet and desktop without any user interaction
     - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
       label: Visit the page on tablet and desktop without any user interaction
       viewportNames:
         - tablet
         - desktop

     # The page under test in below test case should have dark theme
     - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
       label: Click theme toggle on tablet and desktop
       viewportNames:
         - tablet
         - desktop
       actions:
         - click: '#theme-toggle'

     # The page under test in below test case should have light theme
     - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
       label: Click theme toggle 2nd times on tablet and desktop
       viewportNames:
         - tablet
         - desktop
       actions:
         - click: '#theme-toggle'
         - click: '#theme-toggle'

     # Visit the page on mobile without any user interaction
     - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
       label: Visit the page on mobile without any user interaction
       viewportNames:
         - phone

     # The page under test in below scenario should have dark theme
     - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
       label: Click theme toggle on mobile
       viewportNames:
         - phone
       actions:
         - click: '#menu-btn'
         - click: '#theme-toggle'

     # The page under test in below scenario should have light theme
     - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
       label: Click theme toggle 2nd times on mobile
       viewportNames:
         - phone
       actions:
         - click: '#menu-btn'
         - click: '#theme-toggle'
         - click: '#theme-toggle'
   ```

## Verify

Execute the following command to verify the test suite:

```bash
regressify ref --test-suite theme-toggle
regressify approve --test-suite theme-toggle
regressify test --test-suite theme-toggle
```

## Further Experimentation

You can try to remove the `viewportNames` property from some of above test scenarios and observe the outcome of the test results.

## Advanced use cases

The designations of `phone`, `tablet`, and `desktop` within the aforementioned test suite are defined within the **common/_viewports.yaml** file. You have the flexibility to modify their labels, dimensions, and even include or exclude viewports to align with the specific requirements of the project.

Should any test scenarios necessitate a unique viewport that is not predefined in the aforementioned file, you can seamlessly conduct testing using the `viewports` property, exemplified in the following instance:

```yaml
scenarios:
  - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
    viewports:
      - label: My special device # Use any name you find meaningful
        width: 1366
        height: 768

      - label: My other special device
        width: 600
        height: 1200
```

[moon]: /icons/moon.svg
[sun]: /icons/sun.svg
[hamburger]: /icons/hamburger.svg
