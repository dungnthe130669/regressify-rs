---
name: desktop-schema-feedback
description: 'Use when changing generated JSON Schema, desktop validation messages, source ranges, hover help, YAML/JSON completion, or field metadata.'
---

# Schema, Diagnostics, and Completion

## Start Here

[Schema metadata](../../../src/desktop/schema.rs), [diagnostics](../../../src/desktop/validation.rs), [completion/source locations](../../../src/desktop/completion.rs), and [source UI](../../../src/desktop/ui/source.rs).

## Procedure and Invariants

1. Derive constraints from real Rust/serde types and generated schema. Update CLI parsing, desktop semantics, and field help together instead of adding a second independently maintained schema.
1. Validate kind-aware `Document::value()` results: omitted workspace YAML uses defaults without loosening the generated object schema. Preserve the [lossless editing contract](../desktop-lossless-editing/SKILL.md).
1. Preserve useful diagnostics for nested untagged actions: identify the intended branch/property and a concrete repair, not every failing enum alternative.
1. Do not echo secrets or arbitrary values in messages/hover help. Escape literal Markdown/HTML characters in selectable diagnostic text without breaking copying.
1. Compute valid byte ranges for Unicode, quoted keys, lists, missing values, and incomplete syntax. Completion must not replace neighboring source text. Kit 0.7.3 uses Unicode scalar columns in its LSP-shaped `Position` API, not UTF-16; verify byte/position round-trips with `RopeExt` and actual completion/issue navigation after non-BMP characters.
1. Keep source parseability separate from schema validity. Do not block all editing or Fields navigation because a parseable draft contains an invalid property.
1. Action viewport choices use the effective consumer selection and the existing list picker. Workspace-aware diagnostics reuse CLI ID assignment, dependency checks, and `validate_action_viewport_names`, retaining source action pointers. Refresh after both Fields and Source edits off-thread; key results to workspace/file/exact source and retain the combined 50-issue bound, reserving space for existing issues. Leave syntax errors to Source inspection to avoid duplicate issues.
1. Viewport value completion uses the workspace resolver on the background provider and parsed value spans for both scalar strings and list items, including quoted spaces/hyphens and YAML block lists. Preserve scalar/list representation and neighboring text; keep unset action templates free of optional-field noise.
1. `Format::Text` must not pass through YAML/JSON source indexing, completion, or schema validation. Clear source completion/hover providers when opening plain text; validate only the text-editing limits and content guard.
1. Route Enter and Tab through the open completion menu; retain normal Tab indentation and Shift+Tab outdent otherwise. Clear both `hover_popover` (`clear_hover_state`) and `diagnostic_popover` (`clear_diagnostic_popover`) when reconfiguring source, updating diagnostics, leaving the Source tab, or pressing Escape without an open completion menu; skip redundant background diagnostic resets when `source_issues` is unchanged. On window reactivation, restore lost source focus only for the same visible document that previously held it, without displacing another control.

## Checks

```sh
cargo test --features desktop --lib desktop::schema::tests
cargo test --features desktop --lib desktop::validation::tests
cargo test --features desktop --lib desktop::completion::tests
cargo test --features gui-tests --lib desktop::ui::tests::source_ -- --test-threads=1
```

Cover YAML and JSON, incomplete/unknown keys, ambiguous actions, numeric limits, Unicode replacement ranges, secret-free help, and preservation of adjacent text. Keyboard/focus changes also require the [native UI checks](../desktop-native-ui/SKILL.md), including `-SchemaFlows` on an isolated unlocked Windows desktop.

For action viewport authoring run `cargo test --features desktop --lib action_viewport`, the native `action_viewport_picker_filters_choices_and_preserves_invalid_source` test with `gui-tests`, and Windows `-SchemaFlows`. Preserve invalid drafts, save rejection, and stale-diagnostic clearing; do not weaken the smoke's single syntax-issue assertion.

## Maintain

Update [desktop docs](../../../docs/desktop.md), schema-derived user examples, and this skill. Explain parser offset or diagnostic-branch heuristics with comments.
