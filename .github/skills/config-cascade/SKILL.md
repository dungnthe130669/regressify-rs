---
name: config-cascade
description: 'Use when changing workspace, suite, or scenario configuration, defaults, URL replacements, basic auth, numeric validation, or serde/schema compatibility.'
---

# Configuration Cascade

## Start Here

[Configuration](../../../src/config/mod.rs), [types](../../../src/types/regressify.rs), [URL replacements](../../../src/config/replacements.rs), and [basic authentication](../../../src/config/basic_auth.rs).

## Procedure and Invariants

1. Trace parsing and resolution to the consumer. Preserve scenario -> suite -> workspace -> defaults, explicit false/zero, and list replacement semantics. Snapshot report text is workspace-only; follow [snapshot precedence](../snapshot-command/SKILL.md) instead of the capture cascade.
1. Keep serde names, unknown-field rejection, schema constraints/descriptions, desktop validation, and effective configuration aligned when adding a field.
1. Validate finite thresholds, positive concurrency, non-negative delays, browser support, and replacement regexes before fetch/capture. Do not clamp bad input silently or defer configuration errors to individual scenarios.
1. Preserve reference versus test URL replacement behavior and precedence of CLI options/environment settings. Compile regexes once at the owning boundary.
1. Resolve configured files through the existing canonical containment helpers. An absolute configured file is allowed only inside the canonical workspace; missing optional defaults and missing explicit files have different semantics.
1. Keep authentication origin-scoped and secrets out of serialized effective configuration, diagnostics, completion examples, and progress events.

## Checks

```sh
cargo test --lib config::
cargo test --lib types::
cargo test --features desktop --lib desktop::validation::tests
```

Cover all cascade levels, omitted versus explicit values, malformed regexes, numeric boundaries, unknown fields, escaping, and external/symlink file paths.

## Maintain

Update [suite docs](../../../docs/create-test-suite.md), [scenario docs](../../../docs/create-scenario.md), examples, and this skill. Explain exceptions to normal precedence with concise code comments.
