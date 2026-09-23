---
name: lifecycle-hooks
description: 'Use when changing CSS overrides, jsOnReady, document navigation hooks, Node onBefore/onReady scripts, Playwright attachment, or hook timeouts.'
---

# Document and Node Hooks

## Start Here

[Document hooks](../../../src/test/hooks.rs), [Node runner](../../../src/test/node_hook.rs),
[Node adapter](../../../src/test/node_hook.cjs), and
[scenario sequencing](../../../src/test/test_scenario.rs).

## Procedure and Invariants

1. Resolve hook files through workspace containment checks. Distinguish absent
   optional default scripts from invalid explicit paths.
1. Apply document CSS/on-ready code once per document, including after navigation.
   Preserve ready-context data and propagate script errors instead of marking an
   incomplete hook successful. Review selector CSS and JS serialization separately.
1. Optional Node hooks require Node and workspace Playwright/playwright-core.
   They attach to the existing Chromium target; they must not close its browser.
1. Pass the structured payload through stdin, never a shell command string. Bound
   execution and retained stderr while continuing to drain the pipe.
1. Scripts are executable project code: use trusted fixtures and synthetic data.
   Review timeout/cancellation for descendants as well as the immediate process;
   kill-on-drop of one child is not proof of process-tree cleanup.

## Checks

```sh
cargo test --lib test::hooks::tests -- --list
cargo test --lib test::node_hook::tests -- --list
```

These fixtures are ignored without managed Chromium; the Node fixture also names
its package prerequisite in `#[ignore]`. A listing is not a passing behavior test.
Run the selected fixture with `--ignored` only when prerequisites are available.

## Maintain

Update [scenario docs](../../../docs/create-scenario.md),
[suite docs](../../../docs/create-test-suite.md), and this skill. Comment lifecycle
ownership and script-order decisions, not routine payload assignments.
