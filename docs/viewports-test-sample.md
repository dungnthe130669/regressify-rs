---
title: 'Sample: Viewport-Specific Testing'
guid: 5fb24910-6e86-4eae-ba0c-28a8d8a239b3
description: Use shared scenarios and viewport-scoped actions to test responsive phone, tablet, and desktop behavior in Regressify.
date: 2023-08-08T17:00:00+00:00
updated: 2026-10-05T00:00:00+00:00
draft: false
hidden: false
weight: 84000
sort_by: weight
template: docs/page.html

extra:
  lead: This sample shows how the same feature can need different scenarios on phone, tablet, and desktop layouts. It uses the theme toggle as a small but realistic example, because responsive UI likes to move the goalposts just when your tests thought they were settled.
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

Responsive interfaces rarely behave like one page in three jackets. This sample shows how to test the same feature across viewport-specific states without pretending mobile and desktop are interchangeable.

## Use case

Frequently, certain sections of the website exhibit variations across mobile, tablet, and desktop interfaces. For instance, the Header block may be entirely visible on desktop, whereas on mobile, it remains hidden until the user interacts with the ![Hamburger][hamburger] icon. Scope that interaction with action-level `viewportNames` to share the rest of the scenario. Separate scenarios remain useful when the expected end states differ.

## Shared Responsive Actions

With `phone`, `tablet`, and `desktop` defined in `common/_viewports.yaml`, one
scenario can open the menu only on the phone before all jobs toggle the theme:

```yaml
scenarios:
  - id: toggle-theme
    url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
    viewportNames: [phone, tablet, desktop]
    actions:
      - click: '#menu-btn'
        viewportNames: [phone]
      - click: '#theme-toggle'
      - save: phone-menu
        viewportNames: [phone]
```

Only the phone job expects `phone-menu`; every job still takes its final capture.
Unset, `null`, and `[]` action filters mean all selected viewports. Names must
belong to the scenario's effective selection, including inherited `needs` actions.
See [action viewport selection](create-action.md#viewport-selection) for validation,
disjoint save names, and the persist/state limitation.

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
        label: Click theme toggle a second time on tablet and desktop
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
        label: Click theme toggle a second time on mobile
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

Try removing the `viewportNames` property from some of the scenarios and compare how the results change.

## Advanced use cases

The `phone`, `tablet`, and `desktop` labels used in the test suite are defined in the **common/_viewports.yaml** file. You can adjust their names, dimensions, and inclusion rules to match the project.

Add custom definitions to a viewport file. To use another file, set the suite's
`viewportsPath` (or the workspace default); inline scenario `viewports` is not
supported. For example, define `common/custom-viewports.yaml` as:

```yaml
- label: special-desktop
  width: 1366
  height: 768
- label: special-mobile
  width: 600
  height: 1200
```

Then select those labels in the suite:

```yaml
viewportsPath: common/custom-viewports.yaml
scenarios:
  - url: https://tuyen.blog/optimizely-cms/testing/viewports-test-sample/
    viewportNames: [special-desktop, special-mobile]
    actions:
      - click: '#menu-btn'
        viewportNames: [special-mobile]
```

[moon]: /icons/moon.svg
[sun]: /icons/sun.svg
[hamburger]: /icons/hamburger.svg
