# Outstanding Issues

Issues identified during adversarial review that remain after the June 2026 high-priority fix pass.

## 1. Scenario `selectors` Is Accepted But Not Implemented

**Severity:** Medium  
**File:** `src/types/regressify.rs`, `src/test/capture.rs`, `src/test/test_scenario.rs`

**Problem:** `TestScenario` includes a `selectors` property, but screenshot capture always captures the full page. Users can accidentally configure `selectors` and receive full-page screenshots without a configuration error.

**Fix options:** Implement element/viewport selector capture, or reject `selectors` during suite validation until selector capture is implemented.

## 2. `asyncCompareLimit` Is Parsed But Not Used

**Severity:** Low  
**File:** `src/config/mod.rs`, `src/compare/report_gen.rs`

**Problem:** Workspace/suite config supports `asyncCompareLimit`, but comparison currently runs through screenshots sequentially inside `generate_comparison_report`.

**Fix options:** Parallelize image comparison with a bounded worker pool, or remove/deprecate the option.

## 3. Wait URL Glob Matching Is Simplistic

**Severity:** Low  
**File:** `src/test/test_action.rs` (Wait action URL branch)

**Problem:** URL waits only support a single `**` prefix/suffix pattern. They do not support `*`, `?`, or multiple glob segments.

**Current behavior:** `url: "https://example.com/**/dashboard"` matches any URL starting with `https://example.com/` and ending with `dashboard`. `url: "https://example.com/*/page"` treats `*` literally.

**Fix options:** Document the limitation only, or use a real glob matcher for URL patterns.

## 4. Cross-Origin Iframe Errors Could Be Clearer

**Severity:** Low  
**File:** `src/test/actionability.rs`

**Problem:** Frame traversal uses `contentDocument`, so cross-origin iframes fail as expected. The resulting JavaScript evaluation error can be less helpful than a direct message such as "cross-origin frame is not accessible".

**Fix option:** Add explicit try/catch handling around frame traversal expressions and return a clearer error.

## Resolved In June 2026 Fix Pass

- `press` now uses CDP `Input.dispatchKeyEvent` and supports modifier combinations like `Ctrl+A`, `Shift+Tab`, and `Meta+K`.
- Replacement profiles are loaded, validated, and applied during `test`; invalid regexes fail early as configuration errors.
- Resolved `hideSelectors`/`removeSelectors` are applied through the config cascade.
- `cookiePath` and persisted state paths are confined to the project/output directories; state respects `--site-env`.
- Unknown viewport names, duplicate viewport labels, empty labels, and zero dimensions fail before browser launch.
- Visual mismatches and website/action failures are report results, not process failures.
- `.tests.yml` and `.tests.json` files are parsed from their discovered paths instead of being incorrectly remapped to `.tests.yaml`.
- `needs` dependencies now fail early as configuration errors when missing, duplicated, cyclic, or deeper than 100 levels.
- Duplicate suite formats are deduplicated by suite name with `.tests.yaml` > `.tests.yml` > `.tests.json` precedence.
- `postInteractionWait` defaults to `1000` milliseconds through the workspace/suite/scenario cascade.
- `wait.url` uses active URL replacements in `test` mode; `ref` captures and waits against raw URLs.
- `restore`, `requiredLogin` / `--requiredLogin`, `init`, `install`, and `version` are implemented in the Rust CLI where they fit the Chromium/CDP design.
