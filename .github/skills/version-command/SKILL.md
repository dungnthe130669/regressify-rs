---
name: version-command
description: 'Use when changing regressify version output, package-version sourcing, version flags, or CLI version behavior.'
---

# Version Output

## Start Here

[Package version](../../../Cargo.toml), [command definitions](../../../src/cli/command.rs),
and [application dispatch](../../../src/application.rs).

## Procedure and Invariants

1. Source runtime version text from Cargo package metadata; do not add another
   hardcoded version in dispatch, docs, or tests.
1. Keep version reporting independent of browser availability, website access,
   and report artifacts. Preserve zero exit on successful output.
1. A version-output change is not authorization to bump the release, build an
   installer, publish artifacts, or change update manifests.

## Checks

```sh
cargo run --bin regressify -- version
```

Compare output with `package.version`. For parser changes also run
`cargo test --lib cli::`; parser tests alone do not verify printed version text.

## Maintain

Update [command docs](../../../docs/commands.md) and this skill if the public
format changes. Keep release procedures in their own skills.
