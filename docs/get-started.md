---
title: Get Started With Regressify Testing
nav: Get Started
description: Get started with Regressify for Optimizely CMS by installing the tools, creating your first suite, and running visual checks.
guid: 6f40e57f-eb81-4589-9599-b2838c21c07f
date: 2023-06-29T17:00:00+00:00
updated: 2026-04-24T17:00:00+00:00
draft: false
hidden: false
weight: 10000
sort_by: weight
template: docs/page.html

extra:
  lead: This guide walks through the first Regressify setup for Optimizely CMS, from local prerequisites and one-time installation steps to creating a simple suite and running your first visual checks. It is the quickest path from zero to a working baseline.
  toc: true
  top: false
---

If this is your first Regressify project, work through this page in order once. After that, you can come back only for the parts you need.

## Prerequisites

{% danger() %}

Please follow the instructions on this page exactly. Testers sometimes use other tools or IDEs such as Visual Studio or IntelliJ IDEA. Those alternatives may work, but advanced features like auto-suggest, linting, and error detection will often be missing, which makes test cases easier to break and harder to support.

{% end %}

Before you begin, make sure you have the following programs installed on your system:

- [Node.js][nodejs] - 18.x, 20.x or higher
- [Visual Studio Code][vs-code] - Latest version

Additionally, ensure you have the following Visual Studio Code extensions installed:

- [ESLint][eslint] - Microsoft
- [Path Intellisense][path-intellisense] - Christian Kohler
- [Prettier - Code formatter][prettier] - Prettier  
  *After installation, set Prettier as the default formatter for your project.*
- [Regressify](https://marketplace.visualstudio.com/items?itemName=tuyen.regressify) - Tuyen Pham
- [YAML][yaml] - Red Hat  
  *After installation, go to Settings --> YAML Store --> Disable.*

## Initial Setup

If this is your first time working with **Regressify**, follow these steps (only required once). Please note that the process may take 10 minutes or more depending on your internet connection.

1. *(Microsoft Windows only)* Ensure that you have enabled [script execution](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/set-executionpolicy) in PowerShell by running the following command in Admin mode:

   ```powershell
   Set-ExecutionPolicy -ExecutionPolicy RemoteSigned
   ```

1. Install the [regressify](https://www.npmjs.com/package/regressify) npm package globally:

   ```bash wrap
   npm i -g regressify
   ```

## Installation

1. Create a new folder for the regression test.

1. Navigate into the newly created folder and initialize the project by running the following command in the terminal:

   ```bash wrap
   npm init --yes
   ```

1. Install Playwright browsers:

   ```bash wrap
   regressify install
   ```

1. Create the default test suites:

   ```bash wrap
   regressify init
   ```

1. (Optionally) Initialize git in your folder:  
   Ensure you have [Git](https://git-scm.com/downloads) installed locally.

   ```bash wrap
   git init
   git add .
   git commit -m "Initial commit"
   ```

## Creating your first test suite

1. Create a test suite.

   Create a test suite in the [**visual_tests**](https://github.com/precise-alloy/regression-test/tree/master/visual_tests) folder. Each test suite should adhere to the naming convention:  **\<name\>.tests.yaml**.  
   For instance, you can create a file named **[visual_tests/alloy.tests.yaml][alloy-test-suite]** with the following content:

   ```yaml
   scenarios:
     - url: https://alloy.eshn.dev/pages/home.html
     - url: https://alloy.eshn.dev/pages/service.html
     - url: https://alloy.eshn.dev/pages/portfolio.html
     - url: https://alloy.eshn.dev/pages/contact/default.html
   ```

   For more information on available options within a test suite, refer to the [Create a test suite][create-test-suite] and [Create a scenario][create-scenario] documentation.

1. Take a snapshot of the website.

   To capture a snapshot of the website at the current time, use the following command:

   ```bash wrap
   regressify ref --test-suite alloy
   ```

   Please note that the `ref` command only captures the snapshot but does not open any report in the browsers.

   {% info() %}

   If you run the `ref` command for the first time, it will always fail. The `ref` command compares the result with a previously approved result, which doesn't exist on the first run, causing the failure. This behavior is expected.

   {% end %}

1. Approve the snapshot.

   To approve the captured snapshot, run the following command:

   ```bash wrap
   regressify approve --test-suite alloy
   ```

1. Test for visual changes.

   Suppose the development team has just deployed new code to the site, and you want to ensure that nothing is breaking visually or that all visual changes are as intended. Use the following command to perform the test:

   ```bash wrap
   regressify test --test-suite alloy
   ```

   After the command finishes, it will open the test report in your default browser, showing the results.

   If the test results appear to be correct, run the `approve` command again as mentioned above to set the new reference. However, if any issues are identified, inform the development team so that they can fix the problems and then re-run the testing step.

1. Run regression testing after a new deployment.

   Whenever there is a new deployment, you can re-execute the `test` and `approve` it again to perform regression testing on the site visually.

[nodejs]: https://nodejs.org/en/download/prebuilt-installer
[vs-code]: https://code.visualstudio.com/
[eslint]: https://marketplace.visualstudio.com/items?itemName=dbaeumer.vscode-eslint
[prettier]: https://marketplace.visualstudio.com/items?itemName=esbenp.prettier-vscode
[yaml]: https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml
[path-intellisense]: https://marketplace.visualstudio.com/items?itemName=christian-kohler.path-intellisense
[create-test-suite]: @/optimizely-cms/testing/create-test-suite.md
[create-scenario]: @/optimizely-cms/testing/create-scenario.md
[alloy-test-suite]: https://github.com/precise-alloy/regression-test/blob/master/visual_tests/alloy.tests.yaml
