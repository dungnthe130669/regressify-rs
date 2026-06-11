---
title: Update Regressify
description: Update Regressify in an existing Optimizely CMS test project by using the VS Code command or the CLI, then refresh generated files safely.
guid: 6b5406d9-57f3-4afa-ac8a-5e41b2282087
date: 2025-01-08T17:00:00+00:00
updated: 2026-04-19T17:00:00+00:00
draft: false
hidden: false
weight: 91000
sort_by: weight
template: docs/page.html

extra:
  toc: true
  top: false
---

Use this page when the test project already works and just needs newer tooling. If you are setting up Regressify for the first time, start with [Get Started With Regressify Testing](@/optimizely-cms/testing/get-started.md) instead.

## Update from Visual Studio Code

If you have the [Regressify](https://marketplace.visualstudio.com/items?itemName=tuyen.regressify) extension installed in Visual Studio Code, open the test project and:

1. Right-clicking any test suite file.
1. Selecting **Regressify - Update Library**.

Note: This updates the library used for running tests, not the Visual Studio Code extension itself. The extension is updated automatically by Visual Studio Code.

## Update from the command line

Use the command line when you want the update steps to stay explicit and repeatable.

1. Pull the latest CLI source and rebuild it:

```bash wrap
git pull
cargo build --release
```

1. If you installed the CLI with Cargo, reinstall the local build:

```bash wrap
cargo install --path .
```

The `init` command is non-destructive: it creates missing starter files but does not overwrite existing `visual_tests/` or `common/` files. Existing project-specific test data should still be reviewed and updated intentionally.
