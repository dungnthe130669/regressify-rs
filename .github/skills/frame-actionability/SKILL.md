---
name: frame-actionability
description: 'Use when fixing iframe/nested-frame actions, frame visibility and overlays, CDP coordinate offsets, frame waits, or cross-origin failures.'
---

# Frame Actionability

## Start Here

[CDP frame runtime](../../../src/test/runtime.rs), [frame/actionability helpers](../../../src/test/actionability.rs), [action dispatcher](../../../src/test/test_action.rs), and [local browser fixtures](../../../src/test/browser_tests.rs).

## Procedure and Invariants

1. Follow `Runtime::target` from the action dispatcher. It resolves frame IDs and isolated execution contexts, including cross-origin/out-of-process frames. Do not replace it with same-origin-only contentDocument traversal.
1. Preserve bounded waits for frames and their targets, including delayed navigation and disappearing/replaced documents. Do not retry beyond the action deadline.
1. Apply target visibility, stability, enabled-state, scroll, and hit-testing rules inside the frame. Also test covering elements in ancestor documents.
1. Convert local frame coordinates into top-level viewport coordinates before CDP pointer input. Include nested offsets, frame borders, and scrolling.
1. Preserve context-cache invalidation on navigation, detach, and event overflow. Distinguish missing/wrong-type/inaccessible targets from supported cross-origin frames; never silently execute in the parent document after resolution fails.

## Checks

```sh
cargo test --lib test::actionability::tests
cargo test --lib test::test_action::tests
cargo test --lib test::runtime::tests
```

Use the existing ignored Chromium fixtures for event/geometry changes. Cover nested/cross-origin frames, quotes/backslashes, wrong element types, delayed frames, overlays, scrolling, and invalidated contexts. Confirm cleanup on every failure.

## Maintain

Update frame examples in [action docs](../../../docs/create-action.md) and this skill. Explain coordinate conversion, session routing, and invalidation in comments.
