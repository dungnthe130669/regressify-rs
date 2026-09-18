# Copilot Instructions

## Project Overview

Regressify CLI is a visual regression testing tool written in Rust. It is a **Rust port** of the Node.js version at `~/git/regressify-ts` (which wraps BackstopJS + Playwright). This Rust version uses `chromiumoxide` (a CDP client) directly, giving full control over browser automation.

It launches a headless Chromium browser, executes test scenarios defined in YAML/JSON files, captures screenshots, and generates comparison reports.

## Project Structure

- `src/types/actions.rs` — All test action type definitions (serializable structs/enums)
- `src/types/regressify.rs` — `TestSuite`, `TestScenario`, and related config types
- `src/types/mod.rs` — Re-exports types (gated behind `local` feature)
- `src/test/test_action.rs` — Action execution logic (`exec_test_action`)
- `src/test/test_scenario.rs` — Per-scenario orchestration (`exec_test_scenario`)
- `src/test/test_suite.rs` — Per-suite orchestration, YAML parsing, ID assignment
- `src/test/test_suites.rs` — Top-level entry for running all test suites
- `src/test/browsers.rs` — Browser launch/config (Chromium via CDP)
- `src/test/capture.rs` — Screenshot capture & PNG metadata stripping
- `src/common/globals.rs` — Global constants and statics (`ROOT_DIR`, `REGRESSIFY_DIR`, etc.)
- `visual_tests/` — YAML test suite files (e.g., `basic.tests.yaml`)
- `src/compare/` — Image comparison and report generation
- `src/snapshot/` — Snapshot report generation (HTML reports, diff analysis)
- `src/optimizer/` — Image optimization (oxipng, imagequant)
- `src/cli/` — CLI argument parsing (clap derive)
- `src/helpers/` — Utility modules (IO, HTTP, slugify, locks, etc.)
- `src/init.rs` — Non-destructive starter-file generation for `regressify init`

## Documentation

- **[docs/commands.md](docs/commands.md)** — CLI command behavior, flags, outputs, and exit-code semantics
- **[docs/create-action.md](docs/create-action.md)** — Guide for creating new actions
- **[docs/create-scenario.md](docs/create-scenario.md)** — Guide for creating scenarios
- **[docs/create-test-suite.md](docs/create-test-suite.md)** — Guide for creating test suites

## Key Conventions

- The browser is `chromiumoxide` (Rust CDP client), NOT Playwright. Use `Page::find_element`, `Element::click()`, `Page::evaluate()`, etc.
- Only Chromium is currently supported. `firefox` and `webkit` config values must fail early as unsupported.
- Actions inside iframes are handled by building JS expressions that traverse `contentDocument` chains.
- All action structs use `#[serde(rename_all = "camelCase")]` for YAML/JSON compatibility.
- The `TestAction` enum is `#[serde(untagged)]` — variant order matters for deserialization.
- File paths in `input` file uploads are relative to the `visual_tests/` directory.
- The `select` action uses JS evaluation since `chromiumoxide` has no native select API.
- Persisted state is saved to `{REGRESSIFY_DIR}/states/{name}.json`, so `--site-env staging` uses `.regressify__staging/states/`.
- Feature flags: `local` (default, enables browser testing), `cloud`.

## Command Exit Semantics

- Config and infrastructure errors are process failures: malformed YAML/JSON, invalid viewport names, invalid replacement regexes, unsupported browser settings, browser fetch/install failures, browser launch failures, and report/artifact write failures.
- Website/scenario/visual failures are report results with zero exit: navigation timeout, missing/obscured/disabled elements, failed actions, failed scenario captures, missing references, and visual mismatches.
- `ref` captures baselines and promotes successful screenshots. Failed capture placeholders must not be promoted, and existing references should not be deleted before a successful capture.
- `test` captures current screenshots and compares unless `--skip-compare` is set. Visual mismatch must not make the command non-zero.
- `compare` regenerates comparison data from latest test screenshots. Visual failures are report data; artifact/report generation errors are process errors.
- `approve` promotes only successful latest test screenshots and skips failed capture placeholders.
- `init` creates missing starter files only and must not overwrite project-specific test data.
- `install` fetches the Chromium browser binary and fails non-zero on fetch/install errors.
- `version` prints the Cargo package version.

## Core Design Philosophy: Real-User Behavior Simulation

This tool simulates **real human interactions** with a browser. Every action implementation MUST follow these principles:

### Pre-Action Validation (MANDATORY for all interactive actions)

Before performing any interactive action (click, hover, check, uncheck, focus, input, press, select), the implementation MUST:

1. **Wait for element to exist** — Poll/wait for the selector to match a DOM element (with timeout).
1. **Wait for element to be visible** — The element must have non-zero bounding box, not be `visibility: hidden`, not be `display: none`, and have `opacity > 0`.
1. **Wait for element to be stable** — The element's bounding box must not be changing (no animations in progress). Check at least 2 consecutive frames.
1. **Scroll element into viewport** — A real user must see an element before interacting with it. Use `scrollIntoView({ block: 'center' })` behavior.
1. **Check element is not obscured** — Use `document.elementFromPoint(x, y)` at the element's center to verify no other element (modal, overlay, tooltip, sticky header/footer) is covering it. If obscured, the action MUST fail with a clear error message identifying the covering element.
1. **Check element is enabled** — For form elements (input, button, select), verify `disabled` is not set.

### Action-Specific Requirements

- **click**: Must fail if element is covered. Must scroll into view first. Must dispatch proper pointer events sequence (`pointerdown` → `mousedown` → `pointerup` → `mouseup` → `click`).
- **hover**: Must move to element center coordinates using CDP `Input.dispatchMouseEvent` with `mouseMoved` type, not just a JS event dispatch. Must scroll into view first.
- **check/uncheck**: Must verify the element is a checkbox/radio. `check` must only click if NOT already checked. `uncheck` must only click if currently checked.
- **input**: Must focus the element first. Must clear existing value (unless `append: true`). Must dispatch `input` and `change` events to trigger frameworks (React, Vue, Angular).
- **focus**: Must scroll into view. Must use CDP `DOM.focus` or equivalent.
- **press**: Must target the correct element. Use CDP `Input.dispatchKeyEvent` with `key`, `code`, modifier bitfield, and virtual key codes where known. Support documented combos like `Ctrl+A` and `Shift+Tab`.
- **select**: Must verify element is a `<select>`. Must fail when no option matches. Multiple values require `<select multiple>`. Must dispatch `change` event after selection.
- **scroll**: Does NOT require interactability checks (scrolling is not a user click).
- **wait**: No pre-action validation needed. Can wait for time, selector, or URL.
- **goto**: No pre-action validation needed.
- **hide/remove**: No interactability checks — these are test utilities, not user actions.
- **save**: No interactability checks — screenshot utility.
- **persist**: No interactability checks — state persistence utility.

### Scenario Isolation

Each scenario is an **independent test**. The execution model:

- If an action within a scenario **fails**, that scenario fails immediately (no further actions in that scenario execute).
- A failed scenario MUST NOT stop other scenarios from executing. All scenarios always run to completion (pass or fail) so the user sees the full test report.
- Scenarios run concurrently (controlled by `asyncCaptureLimit`). Each gets its own browser instance.
- Error propagation: `exec_test_action` returns `Err` → `exec_test_scenario` writes an error placeholder screenshot and returns `Err` → the spawn block logs the page failure and continues.
- Never use `panic!`, `unwrap()` on fallible operations, or `process::exit()` inside action/scenario code — these would kill the entire process.

### Timeout & Retry

- Default timeout for element existence: 30 seconds
- Default timeout for actionability (visible, stable, not obscured): 5 seconds
- Polling interval: 100ms
- On timeout, provide clear error: which check failed, what selector, what was found instead

### iframe Handling

When `frame` is specified:

- Validate each frame selector exists and is an `<iframe>` element
- Navigate through the frame chain via `contentDocument`
- All pre-action validations apply WITHIN the frame context (visibility checks use frame's document)
- For CDP-level actions inside frames, resolve the correct execution context

## When Modifying Actions

1. Add/update the struct in `src/types/actions.rs`.
1. Add the variant to the `TestAction` enum (order matters for untagged deserialization).
1. Implement the handler in `src/test/test_action.rs` inside `exec_test_action`.
1. Ensure all pre-action validations are implemented per the rules above.
1. Update `docs/create-action.md` with the new action's properties, examples, and implementation notes.

## Adversarial Code Review Checklist

When reviewing or modifying action implementations, verify:

- [ ] Element existence is verified (not just assumed from selector)
- [ ] Element visibility is checked (bounding box > 0, not hidden)
- [ ] Element is scrolled into viewport before interaction
- [ ] Element is not obscured by overlays/modals/sticky elements
- [ ] Element is enabled (for form controls)
- [ ] Correct event sequence is dispatched (matching real browser behavior)
- [ ] Timeout produces actionable error message (not just "timeout")
- [ ] CSS selector injection is properly escaped (no XSS in selector strings)
- [ ] Frame navigation validates each step
- [ ] The action doesn't silently succeed when it should fail (e.g., clicking a covered element via JS `el.click()` bypasses overlay checks)

## Known Remaining Issues

See `OUTSTANDING_ISSUES.md` for the current list. As of the June 2026 fix pass, notable remaining gaps are: scenario `selectors` is accepted but full-page capture is always used; `asyncCompareLimit` is parsed but not used; wait URL matching only supports simple `**` prefix/suffix patterns; cross-origin iframe errors could be clearer.

## Reference: Node.js Version

The original TypeScript version is at `~/git/regressify-ts`. Key differences:

- Uses Playwright (via BackstopJS) instead of raw CDP
- Actions are in `.engine_scripts/playwright/actions.js`
- Uses `page.waitForSelector()` before every action (existence check)
- Uses Playwright's built-in actionability checks for click/hover/check
- `persist` uses `browserContext.storageState()` (Playwright API)

The Rust version should match or exceed the TypeScript version's behavior, adding the real-user validations that Playwright provides natively but chromiumoxide does not.
