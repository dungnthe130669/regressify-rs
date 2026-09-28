---
name: chromiumoxide-patterns
description: 'Use when changing Chromium launch, CDP commands, browser handler polling, page ownership, debug/headless behavior, or browser cleanup.'
---

# Chromium and CDP

## Start Here

- [Browser lifecycle](../../../src/test/browsers.rs): launch options and managed cache.
- [Scenario ownership](../../../src/test/test_scenario.rs): browser/page cleanup.
- [Actionability helpers](../../../src/test/actionability.rs): CDP input helpers.
- [Selector/frame runtime](../../../src/test/runtime.rs): isolated contexts and targets.
- [Browser fixtures](../../../src/test/browser_tests.rs): isolated loopback tests.

## Procedure and Invariants

1. Reuse launch helpers and the pinned chromiumoxide API. Chromium is the only supported engine; reject unsupported browser settings before starting work.
1. Keep the CDP handler stream polled while using the browser. Retain ownership of the browser, handler task, and temporary profile through success and failure.
1. Preserve CI headless behavior even when suite/global debug is enabled. Do not add display workarounds or weaken sandbox settings to hide a launch failure.
1. `Page::find_element` is not an existence wait or an actionability guarantee. Use project helpers before interacting; inspect the pinned implementation instead of assuming what high-level click/hover methods dispatch.
1. Construct typed CDP parameters with their builders. Decode evaluation results to the expected type and propagate protocol, timeout, and JS execution errors.
1. Keep profile isolation and bounded launch/navigation shutdown. Distinguish a browser fetch/launch infrastructure failure from a failing website scenario.
1. Read wait URLs from CDP frame metadata, not `location.href` evaluation: redirects can destroy cached execution contexts before invalidation arrives. Preserve query/fragment matching, target frame identity, and out-of-process frame session routing.

The runtime uses a vendored selector engine for CSS/text/XPath/shadow DOM and CDP contexts for cross-origin frames. Preserve that contract rather than substituting raw querySelector. The optional Node hook layer attaches Playwright to an existing Chromium session; it does not replace the Rust CDP action engine.

## Checks

```sh
cargo test --lib test::browsers::tests
cargo test --lib test::actionability::tests
cargo test --lib test::runtime::tests
```

For live browser changes inspect the relevant ignored fixture and its managed Chromium prerequisite. Verify headed local behavior, headless CI, launch failure, and cleanup after action error/cancellation; never use a personal browser profile.

For URL reads, explicitly run `url_reads_do_not_require_a_live_execution_context` and `url_waits_follow_delayed_redirects_and_same_document_navigation` with `--ignored`. Run the frame interaction fixture as well; the main frame tree does not include out-of-process iframe URLs.

## Maintain

Update [command docs](../../../docs/commands.md), relevant [action docs](../../../docs/create-action.md), and this skill when launch or API contracts change. Explain lifecycle ownership and platform-specific flags in code.
