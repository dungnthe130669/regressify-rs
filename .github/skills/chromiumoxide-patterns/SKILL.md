---
name: chromiumoxide-patterns
description: 'Use when changing Chromium launch, CDP commands, browser handler polling, page ownership, debug/headless behavior, or browser cleanup.'
---

# Chromium and CDP

## Start Here

- [Browser lifecycle](../../../src/test/browsers.rs): launch options and managed cache.
- [Scenario ownership](../../../src/test/test_scenario.rs): browser/page cleanup.
- [Tab ownership](../../../src/test/tabs.rs): popup initialization and selection.
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
1. Initial scenario navigation defaults to 60000 ms, independently of `elementTimeout`. Preserve positive `engineOptions.gotoParameters.timeout` overrides; omitted or zero values use the bounded default. Pass the navigation budget through CDP `Page.navigate` so the ordinary 30-second command timeout cannot shorten it. The overall scenario timeout still applies.
1. Read wait URLs from CDP frame metadata, not `location.href` evaluation: redirects can destroy cached execution contexts before invalidation arrives. Preserve query/fragment matching, target frame identity, and out-of-process frame session routing.
1. DOM readiness and document hooks use `runtime::evaluate_main_document`, which sends typed `Runtime.evaluate` without a cached context ID. Let Chromium select its current main document; `Page::evaluate` and `Page::evaluate_expression` can retain destroyed contexts during navigation. Preserve promise awaiting and JavaScript exception propagation, not blanket error retries.
1. Long lazy-load evaluation uses `Runtime::evaluate_main_document_with_timeout` on the existing scenario client with the resolved budget. Chromiumoxide evicts expired commands periodically, so a 31-second success does not prove support beyond its ordinary 30-second timeout. The ignored `lazy_loading_honors_timeouts_beyond_the_cdp_default` fixture waits 61 seconds to cross that interval.
1. Track popup targets before triggering actions. Use CDP opener identity even when `window.opener` is null. Queue paused-target setup before `Runtime.runIfWaitingForDebugger`, then await replies; awaiting `Page.enable` first can deadlock. Install storage/auth listeners before releasing the target.
1. Keep launch-time emulation aligned with the capture viewport: chromiumoxide applies it to every new page and can overwrite a separately configured popup. Disable `chromiumoxide`'s automatic `session_recovery` and child-target domain initialization (`disable_session_recovery().disable_child_target_init()`) so closing popup tabs or loading cross-origin iframes never triggers background CDP sessions against Regressify's per-session state.
1. Touch capability does not propagate to OOPIF targets. Touch jobs install the tab tracker even without switchTab; recursively pause/configure iframe targets before scripts, including popup children. Keep failed setup visible through tracker health rather than enabling touch only when an action starts.

The runtime uses a vendored selector engine for CSS/text/XPath/shadow DOM and CDP contexts for cross-origin frames. Preserve that contract rather than substituting raw querySelector. All user scripts execute inside Chromium; do not launch Node or attach Playwright. Use explicit expression evaluation for classic scripts rather than implicit function-call detection.

## Checks

```sh
cargo test --lib test::browsers::tests
cargo test --lib test::actionability::tests
cargo test --lib test::runtime::tests
```

Run `cargo test --lib test::browser_tests::navigation_ -- --ignored` with managed Chromium to cover a 31-second document response, explicit timeout expiry, full load, and network idle. The slow-document fixture must cross the ordinary CDP command deadline; do not shorten its delay below 30 seconds.

For live browser changes inspect the relevant ignored fixture and its managed Chromium prerequisite. Verify headed local behavior, headless CI, launch failure, and cleanup after action error/cancellation; never use a personal browser profile.

For URL reads, explicitly run `url_reads_do_not_require_a_live_execution_context` and `url_waits_follow_delayed_redirects_and_same_document_navigation` with `--ignored`. Run the frame interaction fixture as well; the main frame tree does not include out-of-process iframe URLs.

## Maintain

Update [command docs](../../../docs/commands.md), relevant [action docs](../../../docs/create-action.md), and this skill when launch or API contracts change. Explain lifecycle ownership and platform-specific flags in code.
