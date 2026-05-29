# Outstanding Issues

Issues identified during adversarial code review (May 2026) that were NOT auto-fixed. Each requires a design decision or significant implementation effort.

---

## 1. Press Action Uses JS Events Instead of CDP `Input.dispatchKeyEvent`

**Severity:** Medium  
**File:** `src/test/test_action.rs` (Press action handler)

**Problem:** The `press` action dispatches `keydown`/`keypress`/`keyup` via JavaScript `dispatchEvent`. This means:
- Browser-native behaviors don't trigger (Enter submitting forms, Tab moving focus, Escape closing dialogs)
- Only framework event listeners (React/Vue/Angular) will fire
- Does not match real user keypresses

**Correct approach:** Use CDP `Input.dispatchKeyEvent` with proper `key`, `code`, `windowsVirtualKeyCode`, and `nativeVirtualKeyCode` fields. Requires a key-name-to-keycode mapping table (e.g., "Enter" → code 13, "Tab" → code 9, "Escape" → code 27, etc.).

**Effort:** Medium — need a key mapping table (~50 keys) and CDP dispatch logic.

---

## 2. Actionability Checks in Iframes: `getComputedStyle` Cross-Document Edge Case

**Severity:** Low  
**File:** `src/test/actionability.rs` (`wait_for_visible`)

**Problem:** `wait_for_visible` calls `window.getComputedStyle(el)` where `window` is the main page's window and `el` is from a sub-frame's document (accessed via `contentDocument` traversal). For same-origin iframes this works. For cross-origin iframes, `contentDocument` access would throw before reaching `getComputedStyle`, so this is not exploitable — but the error message would be confusing ("evaluation failed" rather than "cross-origin frame not accessible").

**Impact:** Cosmetic error reporting only. Cross-origin iframes already fail at the frame traversal step.

**Fix (optional):** Add a try/catch in the JS evaluation to produce a clear "cross-origin frame" error message.

---

## 3. Replacement Profiles Loaded But Not Wired Into Execution

**Severity:** Medium  
**File:** `src/test/test_suites.rs`, `src/config/replacements.rs`

**Problem:** The `--replacement-profile` CLI arg is accepted, `load_replacement_profiles()` exists in `src/config/replacements.rs`, but the profiles are never actually applied during test execution. The `_replacement_profile` variable in `test_suites.rs` is destructured but unused.

**What needs to happen:**
1. In `exec_test_suites` / `exec_ref`, call `load_replacement_profiles(visual_tests_dir)`
2. Call `get_profile_name(cli_arg)` to resolve the profile
3. Pass the `Vec<UrlReplacement>` to `exec_single_scenario` or attach it to `WorkspaceConfig`
4. In `apply_url_replacements`, apply the profile replacements in addition to suite-level ones

**Effort:** Small — wiring only, all building blocks exist.

---

## 4. Resolved Config's `hide_selectors`/`remove_selectors` Computed But Not Used

**Severity:** Low  
**File:** `src/test/test_scenario.rs`, `src/config/mod.rs`

**Problem:** `resolve_config()` computes `resolved.hide_selectors` and `resolved.remove_selectors` with proper cascade. But `exec_test_scenario` still applies `test_suite.hide_selectors` and `scenario.hide_selectors` separately (lines ~102-125). This causes:
- Double-injection if both suite and scenario define selectors (CSS rules are idempotent, so visually correct but wasteful)
- Config cascade from `WorkspaceConfig` level is ignored for hide/remove

**Fix:** Replace the 4 separate hide/remove blocks with:
```rust
for sel in &resolved.hide_selectors {
    inject_css(&page, sel, "visibility: hidden !important").await?;
}
for sel in &resolved.remove_selectors {
    inject_css(&page, sel, "display: none !important").await?;
}
```

---

## 5. `load_cookie_path` Has No Path Confinement

**Severity:** Low (user controls their own YAML)  
**File:** `src/test/test_scenario.rs` (`load_cookie_path`)

**Problem:** `cookie_path` from test suite YAML is used directly as a file path without validation. A malicious YAML file could read arbitrary files (though the output is parsed as YAML cookies, so exfiltration is limited).

**Risk Assessment:** Low — the user writes their own YAML test files. This is not an attack surface unless test files come from untrusted sources.

**Fix (if needed):** Canonicalize and verify the path is within the project root.

---

## 6. Iframe Frame Offset Calculation Assumes Unscrolled Iframes

**Severity:** Low  
**File:** `src/test/actionability.rs` (`get_frame_offset`)

**Problem:** `get_frame_offset` uses `getBoundingClientRect()` on iframe elements to calculate viewport offset. If an iframe's *parent* has been scrolled such that the iframe is partially off-screen, `getBoundingClientRect()` correctly accounts for this (it gives viewport-relative coords). However, if the *iframe itself* is scrollable and has been scrolled, elements inside it report `getBoundingClientRect()` relative to the iframe's own viewport — which is correct for `elementFromPoint` within that frame.

**Actual impact:** After re-analysis, this is correct. The coordinates work as:
- `check_not_obscured` returns element center relative to the iframe's viewport
- `get_frame_offset` returns the iframe's top-left corner in the main viewport
- Sum gives correct main-viewport coordinates for CDP mouse events

**Status:** Not a bug — the current logic is correct for standard cases. Only fails if an iframe has CSS transforms (rotation/scaling), which is an extreme edge case.

---

## 7. Wait URL Glob Matching Is Simplistic

**Severity:** Low  
**File:** `src/test/test_action.rs` (Wait action, URL branch)

**Problem:** The glob matching only supports `**` (splits into prefix/suffix). It doesn't support `*` (single-segment wildcard), `?`, or multiple `**` patterns.

**Current behavior:** `url: "https://example.com/**/dashboard"` matches any URL starting with `https://example.com/` and ending with `dashboard`. But `url: "https://example.com/*/page"` does NOT do single-segment wildcard matching (it looks for literal `*`).

**Impact:** Minor — the Node.js version only used `**` pattern. Document the limitation or add proper glob support via the `glob` crate.
