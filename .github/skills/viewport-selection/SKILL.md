---
name: viewport-selection
description: 'Use when changing viewport files, labels, dimensions, viewportNames filtering, capture job expansion, or desktop viewport choices.'
---

# Viewport Selection

## Start Here

[Viewport parsing](../../../src/config/viewports.rs), [capture job expansion](../../../src/test/test_suites.rs), and [desktop choices](../../../src/desktop/workspace.rs).

## Procedure and Invariants

1. Distinguish a missing optional default viewport file from an explicit missing path. Resolve files inside the workspace before parsing.
1. Reject empty/duplicate labels and zero dimensions before browsers start. Keep case-insensitive label selection and stable capture filename behavior.
1. Omitted or empty viewport selection means all configured viewports; an unknown requested name must not silently broaden selection or drop a job.
1. Action `viewportNames` accepts a string or string array, like scenario selection, but is not a config override. Validate every own/inherited filter against the consumer's full effective set before browser preparation, using `validate_action_viewport_names`; blank/unknown/out-of-scope labels fail, duplicates/case variants do not. Unnamed default captures accept only unset/null/empty-list filters. Preserve prerequisite replay order and never mutate shared actions per job.
1. Keep desktop choices, CLI capture expansion, and effective configuration in agreement. Test scenario overrides independently from suite/workspace choices.
1. Pass the selected dimensions into browser launch emulation as well as page setup, so chromiumoxide's popup initialization cannot restore default dimensions.

## Checks

```sh
cargo test --lib config::viewports::tests
cargo test --lib test::test_suites::tests
cargo test --features desktop --lib desktop::workspace::tests
```

Include multiple viewports, case variants, invalid sizes, label collisions, unknown names, explicit/missing files, and only-the-selected-viewport capture behavior.

`cargo test --lib action_viewport` covers shared membership and strict-subset/cascade validation. `viewport_capture_inventories_match_order_and_progress` checks independently filtered per-viewport inventories, including nested/shared prerequisite saves.

## Maintain

Update [viewport examples](../../../docs/viewports-test-sample.md), [scenario docs](../../../docs/create-scenario.md), and this skill when selection or naming changes. Comment non-obvious identity/collision rules.

Viewport schema derivation is available without desktop features for the [MCP catalog](../mcp-command/SKILL.md). Keep its guide/examples and default-feature `cargo test --lib mcp` checks aligned when changing viewport contracts.
