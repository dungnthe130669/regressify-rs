---
name: init-command
description: 'Use when changing regressify init, starter files, legacy migration, editor settings/schema setup, or non-destructive workspace initialization.'
---

# Initialize a Workspace

## Start Here

[Initialization](../../../src/init.rs) owns starter generation, migration, workspace-directory validation, and write-if-missing behavior. Desktop [session connection](../../../src/desktop/session.rs) invokes runtime-only setup.

## Procedure and Invariants

1. Create only missing starter files. Preserve project suites, config, scripts, and unrelated editor settings; a second invocation must be non-destructive.
1. Both CLI and desktop setup create `regressify.yaml` with inherited defaults. Respect `.yml` alternatives for workspace config and replacement profiles; do not generate a higher-priority default that shadows existing settings.
1. Automatic desktop setup creates only runtime defaults and the suite folder. Keep examples, exported schemas, VS Code files, and legacy migration in explicit CLI init. Do not generate Node helpers, report artifacts, or persisted states.
1. Migrate legacy files only when the new destination is absent. Do not replace project-specific content with defaults or erase old data on a failed write.
1. Validate each workspace path and reject symlinked directories/unsafe entries before creating files, including dangling links at the destination. Existence is not proof an entry is a regular file. Publish complete starter files using same-directory temporary files and no-clobber persistence.
1. Treat permission, conflict, and write failures as command failures with context.

## Checks

```sh
cargo test --lib init::tests
cargo test --features desktop --lib init::tests
cargo test --features desktop --lib desktop::session::tests
```

Cover first/repeated init, deleted defaults, existing custom settings and `.yml` files, legacy migration, concurrent creation, read-only files, symlink rejection, and failed opens in temporary directories. Do not run init in the user's working project as a test. Unix symlink cases require a Unix test host.

## Maintain

Update [getting started](../../../docs/get-started.md), [command docs](../../../docs/commands.md), and this skill when generated content or migration behavior changes. Explain preservation rules in code comments.
