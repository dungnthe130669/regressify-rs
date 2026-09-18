---
title: Regressify CLI Commands
description: Command reference for the Rust Regressify CLI, including global flags, command behavior, output folders, and exit-code semantics.
guid: 35858e8f-70fd-4c4d-a3cb-a0ba3c23210f
date: 2026-06-11T00:00:00+00:00
updated: 2026-09-13T00:00:00+00:00
draft: false
hidden: false
weight: 15000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

This page describes the Rust CLI commands implemented by this repository.

## Global flags

Global flags are placed before the command:

```bash wrap
regressify --root ./my-tests --site-env staging test --test-suite alloy
```

| Flag                          | Purpose                                                                                                                                                                                                                                                           |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--root`, `-r`                | Project root. Defaults to the current directory.                                                                                                                                                                                                                  |
| `--site-env`                  | Writes output to `.regressify__<env>` instead of `.regressify`. Persisted state is also scoped to that directory. The environment name must use only ASCII letters, digits, `_`, or `-`.                                                                          |
| `--run-id`                    | Portable explicit capture directory ID shared by all selected suites. `test` and `ref` write to it; `compare` and `approve` select it. Defaults to a sortable UTC timestamp for capture and the lexically latest run for standalone compare/approve when omitted. |
| `--async-capture-limit`       | Positive global maximum scenario/viewport captures running at once. Defaults to `5`; suite/workspace `asyncCaptureLimit` can impose a lower per-suite cap.                                                                                                        |
| `--async-capture-ramp-ms`     | Delay between adding concurrency slots. Defaults to `1000`.                                                                                                                                                                                                       |
| `--debug`                     | Runs Chromium with a visible window. Suite-level `debug: true` also enables this. Automatically disabled in CI (no display).                                                                                                                                      |
| `--requiredLogin`             | Forces `requiredLogin` true in the resolved scenario context exposed to custom `jsOnReadyPath` scripts.                                                                                                                                                           |
| `--proxy`                     | Proxy URL used by HTTP clients where supported.                                                                                                                                                                                                                   |
| `--pause`, `--pause-if-error` | Wait for input after execution or after errors.                                                                                                                                                                                                                   |

## Commands

| Command    | Purpose                                                                                                                                             |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `init`     | Create starter `common/` files and a sample `visual_tests/alloy.tests.yaml` without overwriting existing files.                                     |
| `install`  | Fetch the Chromium browser binary used by this CLI into the project download cache.                                                                 |
| `version`  | Print the CLI package version. `regressify --version` is also supported by Clap.                                                                    |
| `ref`      | Capture baseline screenshots and promote successful captures to references. Failed page captures are skipped and existing references are preserved. |
| `test`     | Capture current screenshots, compare against references unless `--skip-compare` is set, and generate snapshot report data.                          |
| `compare`  | Compare the latest test run for each matching suite against reference screenshots.                                                                  |
| `approve`  | Promote latest successful test screenshots to references. Failed capture placeholders are skipped.                                                  |
| `snapshot` | Generate the HTML summary report from existing comparison data.                                                                                     |
| `optimize` | Optimize PNG files in the project output tree.                                                                                                      |

All suite filters accept exact names or glob patterns. Use `all`, `*`, or omit `--test-suite` to process every discovered suite. Invalid glob patterns, unreadable suite directories, and an explicit filter that matches no suite fail the command instead of silently reducing coverage.

## Individual Scenarios

`test` and `ref` accept repeated `--scenario <suite>:<scenario-id>` flags:

```bash
regressify test --scenario shop:checkout
regressify ref --scenario shop:checkout --scenario account:sign-in
```

The suite name is its logical name under `visual_tests`, without the file suffix.
Scenario IDs match exactly, not as globs or label substrings. An omitted `id`
uses the same label-derived slug or URL-derived ID as a normal suite run. When
`--test-suite` is also supplied, each scenario must belong to a matching suite.
Unknown or ambiguous selections fail before browser launch or capture writes.
No scenario flags retains whole-suite behavior.

Only selected scenarios produce capture jobs and report entries, across their
configured viewports and saved steps. Their `needs` actions still execute using
the full suite definitions; dependencies do not become additional independent
captures. Reference capture preserves unselected baselines.

`compare` and `approve` also accept `--scenario`, using the suite's **output ID**
instead of its file name when they differ. Use `--run-id` to target an exact run:

```bash
regressify --run-id reviewed-run compare --scenario shop-output:checkout
regressify --run-id reviewed-run approve --scenario shop-output:checkout
```

These commands match recorded scenario IDs in the capture metadata, not screenshot
filename prefixes. All requested output selections are validated before changing
reports or references. Missing scenarios or legacy runs without scenario metadata
fail with a recovery message; whole-suite operations remain compatible with older
runs. Scoped comparison regenerates the report for the selected captures, while
scoped approval leaves other reference images untouched and skips failed captures.

## Exit codes

Regressify separates tool failures from website results.

The command exits non-zero for configuration or infrastructure problems: malformed YAML, invalid viewport names, invalid URL replacement regexes, unsupported browser settings, browser download failures, browser launch failures, and report/artifact write failures.

The command exits zero for website and visual results: navigation timeouts, missing or obscured elements, failed actions, failed scenario captures, missing references, and visual mismatches. Those are written into the comparison and snapshot reports so all scenarios can be reviewed together.

## Browser support

The Rust CLI currently drives Chromium through the Chrome DevTools Protocol. `browser: chromium` is supported. `firefox` and `webkit` are rejected as configuration errors until browser backends are implemented.

## Replacement profiles

The `test` command can apply URL replacement profiles from `common/_replacement-profiles.yaml` or `common/_replacement-profiles.yml`:

```bash wrap
regressify test --test-suite alloy --replacement-profile staging
```

Profile selection order is CLI `--replacement-profile`, then the `REPLACEMENT_PROFILE` environment variable, then `default`. The `test` command applies replacements to scenario URLs and action `wait.url` values. The `ref` command ignores replacements so references are captured from raw suite URLs.

## Optimize

`optimize` compresses PNGs only under the active Regressify output directory (`.regressify` or `.regressify__<site-env>`). It is lossless: it does not quantize colors, resize images, or remove color data, so decoded screenshot pixels remain unchanged.

| Flag            | Purpose                                                        |
| --------------- | -------------------------------------------------------------- |
| `--speed`, `sp` | Compression speed from `1` (smallest files) to `10` (fastest). |
| `--min-size-kb` | Skip files smaller than this many kilobytes. Defaults to `50`. |

An unreadable PNG or failed file replacement makes `optimize` fail rather than silently leaving a partially processed output tree.

## Scenario timeouts

Set `scenarioTimeout` in `regressify.yaml` or `regressify.yml` to bound one complete scenario/viewport capture in milliseconds. It defaults to `600000` (ten minutes). When the deadline expires, Regressify preserves completed intermediate captures, records failed-capture placeholders for remaining expected targets, and continues the remaining scenarios instead of allowing one unresponsive page or CDP operation to stall the run.

## Capture stability

Regressify waits for an interactive DOM after navigation, decodes fonts and images before capture, and uses one `settleTimeout` budget for resource readiness plus DOM/layout quiet. `freezeAnimations` defaults to `true`; it completes finite CSS/Web Animations, freezes infinite ones at their initial frame, and hides the blinking text caret. Set `freezeAnimations: false` only when a scenario intentionally captures an animated intermediate state.

Capture runs use sortable UTC directory IDs with nanosecond precision, so back-to-back invocations do not overwrite each other's screenshots or reports.
Capture commands refuse to reuse an existing suite/run directory. Choose a new
`--run-id` for each invocation or remove an intentionally stale run first; this
prevents old screenshots from being mixed into a new comparison.

## Comparison Reports

Comparison reports retain diff artifacts only for current mismatches. The standalone `compare` command accepts `--async-compare-limit` (default `2`); `test` and `ref` use the suite or workspace `asyncCompareLimit` value.

New capture runs record both the raw reference page URL and the resolved test
page URL for every report entry. The report links to the reference page and,
when URL replacements are active, its corresponding test page. Run `test` or
`ref` once after upgrading to add this metadata to an existing suite; standalone
`compare` reuses it without needing another browser capture.

Capture metadata also records the scenario ID and human label (or page URL when
unlabelled). Reports expose these as optional `scenarioId` and `scenarioLabel`
fields. The desktop uses the captured name even if the scenario is later renamed
or removed. Screenshot names, matching keys, and exit-code behavior are unchanged.
Older manifests without these fields remain readable.

Older capture runs without ordering or threshold manifests remain comparable: missing ordering data uses alphabetical output and missing threshold data uses the command threshold. A present malformed manifest is treated as an artifact error and fails the command.
