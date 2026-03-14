# Copilot Instructions

## Project Overview

Regressify CLI is a visual regression testing tool written in Rust. It launches a headless Chromium browser (via `chromiumoxide`), executes test scenarios defined in YAML/JSON files, captures screenshots, and generates comparison reports.

## Project Structure

- `src/types/actions.rs` — All test action type definitions (serializable structs/enums)
- `src/types/regressify.rs` — `TestSuite`, `TestScenario`, and related config types
- `src/types/mod.rs` — Re-exports types (gated behind `local` feature)
- `src/test/test_action.rs` — Action execution logic (`exec_test_action`)
- `src/test/test_scenario.rs` — Per-scenario orchestration (`exec_test_scenario`)
- `src/test/test_suite.rs` — Per-suite orchestration, YAML parsing, ID assignment
- `src/test/test_suites.rs` — Top-level entry for running all test suites
- `src/test/browsers.rs` — Browser launch/config (Chromium via CDP)
- `src/test/capture.rs` — Screenshot capture
- `src/common/globals.rs` — Global constants and statics (`ROOT_DIR`, `BACKSTOP_DIR`, etc.)
- `visual_tests/` — YAML test suite files (e.g., `basic.tests.yaml`)

## Documentation

- **[docs/actions.md](docs/actions.md)** — Complete reference for all test actions (check, click, focus, goto, hide, hover, input, persist, press, remove, save, scroll, select, uncheck, wait) including properties, examples, shared `frame` property, and implementation notes.

## Key Conventions

- The browser is `chromiumoxide` (Rust CDP client), not Playwright. Use `Page::find_element`, `Element::click()`, `Page::evaluate()`, etc.
- Actions inside iframes are handled by building JS expressions that traverse `contentDocument` chains.
- All action structs use `#[serde(rename_all = "camelCase")]` for YAML/JSON compatibility.
- The `TestAction` enum is `#[serde(untagged)]` — variant order matters for deserialization.
- File paths in `input` file uploads are relative to the `visual_tests/` directory.
- The `select` action uses JS evaluation since `chromiumoxide` has no native select API.
- Persisted state is saved to `.regressify/states/{name}.json`.
- Feature flags: `local` (default, enables browser testing), `cloud`.

## When Modifying Actions

1. Add/update the struct in `src/types/actions.rs`.
2. Add the variant to the `TestAction` enum (order matters for untagged deserialization).
3. Implement the handler in `src/test/test_action.rs` inside `exec_test_action`.
4. Update `docs/actions.md` with the new action's properties, examples, and implementation notes.
