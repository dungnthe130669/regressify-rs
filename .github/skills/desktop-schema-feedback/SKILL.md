---
name: desktop-schema-feedback
description: 'Use when changing generated JSON Schema, desktop validation messages, source ranges, hover help, YAML/JSON completion, or field metadata.'
---

# Schema, Diagnostics, and Completion

## Start Here

[Schema metadata](../../../src/desktop/schema.rs),
[diagnostics](../../../src/desktop/validation.rs),
[completion/source locations](../../../src/desktop/completion.rs), and
[source UI](../../../src/desktop/ui/source.rs).

## Procedure and Invariants

1. Derive constraints from real Rust/serde types and generated schema. Update CLI
   parsing, desktop semantics, and field help together instead of adding a second
   independently maintained schema.
1. Preserve useful diagnostics for nested untagged actions: identify the intended
   branch/property and a concrete repair, not every failing enum alternative.
1. Do not echo secrets or arbitrary values in messages/hover help. Escape literal
   Markdown/HTML characters in selectable diagnostic text without breaking copying.
1. Compute valid byte ranges for Unicode, quoted keys, lists, missing values, and
   incomplete syntax. Completion must not replace neighboring source text.
1. Keep source parseability separate from schema validity. Do not block all editing
   or Fields navigation because a parseable draft contains an invalid property.

## Checks

```sh
cargo test --features desktop --lib desktop::schema::tests
cargo test --features desktop --lib desktop::validation::tests
cargo test --features desktop --lib desktop::completion::tests
```

Cover YAML and JSON, incomplete/unknown keys, ambiguous actions, numeric limits,
Unicode replacement ranges, secret-free help, and preservation of adjacent text.

## Maintain

Update [desktop docs](../../../docs/desktop.md), schema-derived user examples, and
this skill. Explain parser offset or diagnostic-branch heuristics with comments.
