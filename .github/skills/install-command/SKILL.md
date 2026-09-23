---
name: install-command
description: 'Use when changing regressify install, managed Chromium fetching/cache reuse, installation failures, or fetch-browser configuration.'
---

# Install Managed Chromium

## Start Here

[Application dispatch](../../../src/application.rs),
[managed browser lifecycle](../../../src/test/browsers.rs), and
[capture CLI flags](../../../src/cli/args.rs).

## Procedure and Invariants

1. Use the managed Chromium cache; do not fall back to a personal browser/profile.
1. Preserve cached-browser reuse and `fetchBrowser`/`--fetch-browser` precedence.
   Disabling fetch must fail clearly when the managed executable is missing.
1. Propagate network, extraction, filesystem, and executable validation errors.
   Do not report successful installation before a usable browser exists.
1. Keep CLI installation separate from desktop application self-update. Tests
   must not install an application release or modify the user's installation.

## Checks

```sh
cargo test --lib test::browsers::tests
cargo test --lib cli::args::tests::capture_commands_accept_managed_browser_download_settings
```

Inspect ignored browser tests and cache prerequisites before live checks. Download
Chromium only with task authorization; otherwise report that installation was untested.

## Maintain

Update [command docs](../../../docs/commands.md),
[getting started](../../../docs/get-started.md), and this skill when cache or fetch
behavior changes. Document non-obvious platform executable handling in code.
