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
1. Keep desktop choices, CLI capture expansion, and effective configuration in agreement. Test scenario overrides independently from suite/workspace choices.
1. Pass the selected dimensions into browser launch emulation as well as page setup, so chromiumoxide's popup initialization cannot restore default dimensions.

## Checks

```sh
cargo test --lib config::viewports::tests
cargo test --lib test::test_suites::tests
cargo test --features desktop --lib desktop::workspace::tests
```

Include multiple viewports, case variants, invalid sizes, label collisions, unknown names, explicit/missing files, and only-the-selected-viewport capture behavior.

## Maintain

Update [viewport examples](../../../docs/viewports-test-sample.md), [scenario docs](../../../docs/create-scenario.md), and this skill when selection or naming changes. Comment non-obvious identity/collision rules.
