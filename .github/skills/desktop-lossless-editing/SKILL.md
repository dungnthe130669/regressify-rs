---
name: desktop-lossless-editing
description: 'Use when changing desktop YAML/JSON document edits, Source/Fields transitions, undo/redo, invalid drafts, batch edits, or asynchronous save acknowledgement.'
---

# Lossless Desktop Editing

## Start Here

[Document model](../../../src/desktop/document.rs), [session state](../../../src/desktop/session.rs), and [editor orchestration](../../../src/desktop/ui/mod.rs).

## Procedure and Invariants

1. Preserve editable invalid source. Syntax parsing, schema diagnostics, and the save gate are distinct; schema-invalid but parseable source can still use Fields.
1. Treat blank/comment-only workspace YAML as defaults through kind-aware document parsing, without rewriting its source. First-field edits and resetting the last field must round-trip and undo exactly. Explicit nulls and blank non-workspace or JSON documents remain invalid.
1. For YAML structural edits preserve comments, quoting, anchors, and untouched formatting. Compare the resulting parsed value with the expected semantic edit; reject a lossy library rewrite without changing the user's draft.
1. Apply multi-field operations as one undo entry. Keep history byte/count limits, no-op edits, redo invalidation, and dirty-state behavior consistent.
1. An asynchronous save acknowledges the exact bytes written, not the latest source that may have changed while I/O ran. Newer edits must remain dirty.
1. Guard document/workspace switches and destructive actions with existing session transitions; do not silently discard source when a derived view cannot parse it.

## Checks

```sh
cargo test --features desktop --lib desktop::document::tests
cargo test --features desktop --lib desktop::session::tests
```

Cover invalid drafts, Unicode/comments/line endings, plain selectors, anchors, empty collections, list bounds, undo/redo, all shipped suites, and late save results. Use native UI tests when changing tab/navigation behavior.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill. Explain semantic round-trip guards and YAML library workarounds with concise comments. Keep a known unsupported edit separate from a failure to protect the original draft.
