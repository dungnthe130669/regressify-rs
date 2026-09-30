---
name: implement-action
description: 'Use when adding or fixing click, hover, check, uncheck, focus, input, press, select, or other browser actions, their types, events, and error propagation.'
---

# Implement an Action

## Start Here

- [Action types](../../../src/types/actions.rs): serde shapes and `TestAction` order.
- [Action execution](../../../src/test/test_action.rs): `exec_test_action`.
- [Actionability](../../../src/test/actionability.rs): validated interaction helpers.
- [Browser fixtures](../../../src/test/browser_tests.rs): local pages and cleanup.

## Procedure and Invariants

1. Add or update the type and dispatcher together. Preserve `camelCase`, unknown field rejection, and untagged variant precedence; test ambiguous action shapes.
1. Reuse pre-action helpers for interactive actions: wait for existence, visibility, stable geometry across at least two observations, centered scrolling, unobscured targeting, and enabled controls. Preserve configured deadlines and poll bounds.
1. Use CDP pointer movement/press/release for click and hover, and CDP key events with key/code/modifiers/virtual codes for press. JS event dispatch alone does not reproduce browser interaction or CSS hover.
1. Check checkbox/radio type and current state before check/uncheck. Focus before input, clear unless `append`, and preserve framework input/change notification. Select must reject missing options and multiple values on a single-select.
1. Propagate failures with selector/action context. A failing action stops its scenario, not peers; do not panic, exit, or discard meaningful errors.
1. For `save`, validate the name before waiting, then reuse the shared screenshot preparation in [page stability](../../../src/test/page_stability.rs). Keep delay, animation freezing, and settling aligned with final captures; see the [capture skill](../capture-stability/SKILL.md) for the checkpoint regression.

Utilities (save, persist, hide/remove) and waits/navigation do not require the interactive actionability sequence. Scroll still validates its target. File input paths remain confined to `visual_tests`; preserve chooser and direct-input paths. Frame actions need the same validation within each frame and correct CDP coordinates.

## Checks

```sh
cargo test --lib types::actions::tests
cargo test --lib test::test_action::tests
cargo test --lib test::actionability::tests
```

Add a local-browser regression for changed event behavior: missing, delayed, hidden, moving, disabled, covered, nested-frame, quoted-selector, and success cases as relevant. Browser tests are ignored by default; inspect their `#[ignore]` prerequisites before explicitly running the selected test with `--ignored`.

## Maintain

Update [action docs](../../../docs/create-action.md), applicable [examples](../../../visual_tests), and this skill in the same change. Comment non-obvious event ordering, native setters, or platform workarounds, not each step.
