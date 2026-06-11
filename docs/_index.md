---
title: Testing
guid: 92d0f663-c845-4897-a2e4-34424292e6a7
description: Browse Optimizely CMS testing guides, sample suites, and maintenance notes for building and updating visual regression coverage with Regressify.
date: 2023-06-29T17:00:00+00:00
updated: 2026-04-17T17:00:00+00:00
template: docs/section.html
sort_by: weight
weight: 30000
draft: false
hidden: false

extra:
  hide_other: true
  lead: 'This section collects the practical side of Regressify testing for Optimizely CMS: setup guides, sample suites, reference pages, and a running change log. If you need to build coverage, copy a working example, or check what changed, start here.'
---

These pages are meant to help you move from first setup to reusable test suites without digging through a pile of half-related examples.

## Start Here

- [Get Started With Regressify Testing](@/optimizely-cms/testing/get-started.md) for the first local setup, initial installation, and your first working suite.
- [Regressify CLI Commands](@/optimizely-cms/testing/commands.md) for command behavior, global flags, output directories, and exit-code rules.
- [Create a test suite](@/optimizely-cms/testing/create-test-suite.md) for suite structure, grouping scenarios, and suite-level configuration.
- [Create a Test Scenario](@/optimizely-cms/testing/create-scenario.md) for per-page behavior, actions, and scenario-specific settings.
- [Create an action](@/optimizely-cms/testing/create-action.md) for the click, input, wait, navigation, and element-handling details that usually decide whether a suite feels solid or temperamental.

## Build Coverage Faster

- [Create a Test Suite for an Entire Site](@/optimizely-cms/testing/create-test-suite-for-entire-site.md) if the site has a sitemap and you want a starter suite without copying URLs one by one.
- [Sample: Reuse Scenarios](@/optimizely-cms/testing/reuse-scenarios-test-sample.md) if you want repeated steps to stay maintainable instead of breeding quietly in every file.
- [Sample: Form Submission](@/optimizely-cms/testing/form-submission-test-sample.md), [Sample: File Upload](@/optimizely-cms/testing/file-upload-test-sample.md), and [Sample: Viewports](@/optimizely-cms/testing/viewports-test-sample.md) are good examples when you need a working pattern to copy rather than a blank page and optimism.

## Keep The Project Healthy

- [Update the Regressify Test Library](@/optimizely-cms/testing/update-test-library.md) when your local test tooling has drifted behind.
- [Push Local Test Changes to a Remote Repository](@/optimizely-cms/testing/push-to-remote.md) when the suite is ready to leave your machine and become a shared team asset.
- [Change Log](@/optimizely-cms/testing/change-log.md) when you want to see what changed in the docs and available options without trawling through commit history.

## Choose By Task

Use this section by the problem you are solving:

- Use the getting-started guide when you are building a test project from scratch.
- Use the suite, scenario, and action references when a test already exists but needs clearer structure or more reliable behavior.
- Use the sample pages when you need a working example for a common interaction pattern.
- Use the maintenance pages when the library changes, the docs add new capabilities, or the project is ready to be shared with the rest of the team.
