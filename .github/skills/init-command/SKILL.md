---
name: init-command
description: 'Use when changing regressify init, starter files, legacy migration, editor settings/schema setup, or non-destructive workspace initialization.'
---

# Initialize a Workspace

## Start Here

[Initialization](../../../src/init.rs) owns starter generation, migration,
workspace-directory validation, and write-if-missing behavior.

## Procedure and Invariants

1. Create only missing starter files. Preserve project suites, config, scripts,
   and unrelated editor settings; a second invocation must be non-destructive.
1. Migrate legacy files only when the new destination is absent. Do not replace
   project-specific content with defaults or erase old data on a failed write.
1. Validate each workspace path and reject symlinked directories/unsafe entries
   before creating files. Existence is not proof an entry is a regular file.
1. Treat permission, conflict, and write failures as command failures with context.

## Checks

```sh
cargo test --lib init::tests
```

Cover first/repeated init, existing custom settings, legacy migration, symlink
rejection, and failed writes in temporary directories. Do not run init in the
user's working project as a test.

## Maintain

Update [getting started](../../../docs/get-started.md),
[command docs](../../../docs/commands.md), and this skill when generated content
or migration behavior changes. Explain preservation rules in code comments.
