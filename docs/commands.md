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

| Flag                          | Purpose                                                                                                                      |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `--root`, `-r`                | Project root. Defaults to the current directory.                                                                             |
| `--site-env`                  | Writes output to `.regressify__<env>` instead of `.regressify`. Persisted state is also scoped to that directory.            |
| `--run-id`                    | Accepted by the CLI for run identification.                                                                                  |
| `--async-capture-limit`       | Maximum scenario/viewport captures running at once. Defaults to `5`.                                                         |
| `--async-capture-ramp-ms`     | Delay between adding concurrency slots. Defaults to `1000`.                                                                  |
| `--debug`                     | Runs Chromium with a visible window. Suite-level `debug: true` also enables this. Automatically disabled in CI (no display). |
| `--requiredLogin`             | Forces `requiredLogin` true in the resolved scenario context exposed to custom `jsOnReadyPath` scripts.                      |
| `--proxy`                     | Proxy URL used by HTTP clients where supported.                                                                              |
| `--pause`, `--pause-if-error` | Wait for input after execution or after errors.                                                                              |

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

All suite filters accept exact names or glob patterns. Use `all`, `*`, or omit `--test-suite` to process every discovered suite.

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

Set `scenarioTimeout` in `regressify.yaml` or `regressify.yml` to bound one complete scenario/viewport capture in milliseconds. It defaults to `600000` (ten minutes). When the deadline expires, Regressify records a failed-capture placeholder and continues the remaining scenarios instead of allowing one unresponsive page or CDP operation to stall the run.
