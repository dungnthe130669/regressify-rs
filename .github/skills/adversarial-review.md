# Skill: Adversarial Code Review for Actions

## When to Use

Use this skill when:

- Reviewing action implementation code for correctness
- Auditing whether actions properly simulate real user behavior
- Identifying security issues (CSS injection, XSS via selector strings)
- Verifying that the implementation matches Playwright's actionability semantics

## Review Methodology

### Phase 1: Identify the Action Category

Actions fall into categories with different validation requirements:

| Category             | Actions                                                   | Required Checks                                  |
| -------------------- | --------------------------------------------------------- | ------------------------------------------------ |
| **Interactive**      | click, hover, check, uncheck, focus, input, press, select | ALL pre-action validations                       |
| **Scroll**           | scroll                                                    | Existence check only (no interactability needed) |
| **Navigation**       | goto, wait (URL)                                          | No element checks                                |
| **DOM Manipulation** | hide, remove                                              | Existence check only                             |
| **Utility**          | save, persist                                             | No element checks                                |
| **Waiting**          | wait (selector)                                           | Polling with timeout                             |

### Phase 2: Validate Each Check

For each interactive action, verify these are implemented **in this exact order**:

#### Check 1: Element Existence (30s timeout)

```text
❌ WRONG: page.find_element(selector).await?  // Fails immediately if not found
❌ WRONG: doc.querySelector(selector)  // One-shot, no retry
✅ RIGHT: Poll with 100ms interval, 30s timeout, clear error on timeout
```text

#### Check 2: Element Visibility

```text
❌ WRONG: Not checking visibility at all
❌ WRONG: Only checking display:none (misses visibility:hidden, opacity:0, zero-size)
✅ RIGHT: Check ALL of: bbox.width > 0, bbox.height > 0, visibility !== 'hidden',
          display !== 'none', opacity > 0
```text

#### Check 3: Element Stability

```text
❌ WRONG: Not checking stability (animated elements get clicked mid-animation)
✅ RIGHT: Get bbox at time T, wait 100ms, get bbox at time T+100ms, compare.
          If different, wait and retry until stable or timeout.
```text

#### Check 4: Scroll into View

```text
❌ WRONG: Not scrolling at all
❌ WRONG: scrollIntoView() without { block: 'center' } (element might be under sticky header)
✅ RIGHT: scrollIntoView({ block: 'center', inline: 'center' })
          Then re-check bounding rect is within viewport
```text

#### Check 5: Not Obscured

```text
❌ WRONG: Not checking at all (most common bug!)
❌ WRONG: Using JS el.click() which bypasses this check entirely
✅ RIGHT: elementFromPoint(centerX, centerY) === element || element.contains(topElement)
          If obscured, FAIL with identifying info about the covering element
```text

#### Check 6: Enabled State (form controls only)

```text
❌ WRONG: Clicking disabled buttons silently
✅ RIGHT: Check el.disabled !== true for input/button/select/textarea
          If disabled, FAIL with clear message
```text

### Phase 3: Verify Action Execution Method

| Action  | Correct Method                                 | Wrong Method                            | Why                                                      |
| ------- | ---------------------------------------------- | --------------------------------------- | -------------------------------------------------------- |
| click   | CDP `Input.dispatchMouseEvent` at coordinates  | JS `el.click()`                         | JS click bypasses overlay detection, doesn't move cursor |
| hover   | CDP `Input.dispatchMouseEvent` type=mouseMoved | JS `new MouseEvent('mouseover')`        | JS event doesn't trigger CSS `:hover`                    |
| check   | Verify state THEN click (if needed)            | Unconditional `el.click()`              | Double-clicking checked checkbox unchecks it             |
| uncheck | Verify state THEN click (if needed)            | Unconditional `el.click()`              | Clicking unchecked checkbox checks it                    |
| press   | CDP `Input.dispatchKeyEvent` sequence          | JS `dispatchEvent(new KeyboardEvent())` | JS keyboard events may not trigger browser shortcuts     |
| focus   | CDP DOM.focus or element.focus()               | —                                       | Both acceptable                                          |
| input   | element.type_str() with framework events       | Direct value set without events         | Frameworks won't react to programmatic value changes     |

### Phase 4: Error Handling Audit

```text
❌ CRITICAL: .await.ok()  — Silently swallows ALL errors
❌ BAD: .await?  with no context — "element not found" with no selector info
❌ BAD: Returning Ok(()) when action couldn't be performed
✅ GOOD: .await.map_err(|e| anyhow!("Click on '{}': {}", selector, e))?
✅ GOOD: Bail with timeout info, selector, and what check failed
```text

### Phase 5: Security Review

#### CSS Selector Injection

```text
❌ VULNERABLE:
format!("document.querySelector('{}')", user_selector)
// If selector contains: '); document.location='evil.com'//
// Result: document.querySelector(''); document.location='evil.com'//')

✅ SAFE:
selector.replace('\'', "\\'")  // Escape single quotes
// Also escape: \, newlines, special chars

✅ SAFER:
Pass selector as argument to page.evaluate() instead of string interpolation
```text

#### Frame Navigation Security

```text
❌ WRONG: Not validating frame selector targets an actual iframe
✅ RIGHT: Verify element.tagName === 'IFRAME' before accessing contentDocument
           Handle cross-origin frames gracefully (error, not crash)
```text

### Phase 6: iframe-Specific Issues

When actions run inside frames via JS evaluation:

1. All the same pre-action checks must still apply WITHIN the frame
2. `elementFromPoint` must use the frame's document, not the parent
3. Coordinates for CDP mouse events need to be offset by the frame's position in the parent
4. Cross-origin frames cannot be accessed via `contentDocument` — must use CDP execution contexts

## Red Flags Checklist

When reviewing code, immediately flag these patterns:

- [ ] `exec_test_action(...).await.ok()` — Error swallowing
- [ ] `page.find_element(x).await?.click().await?` without any pre-validation
- [ ] `el.click()` in JS for interactive actions (bypasses overlay check)
- [ ] `new MouseEvent('mouseover')` for hover (doesn't trigger CSS :hover)
- [ ] No polling/retry loop before action (immediate fail if element not ready)
- [ ] `.click()` without checking `.checked` state for check/uncheck
- [ ] `press` action bypasses CDP `Input.dispatchKeyEvent` or does not set documented modifier/key/code values
- [ ] String interpolation of selectors without escaping
- [ ] No timeout on any wait/poll loop
- [ ] Action returns Ok(()) even when element wasn't found/visible

## Expected Behavior Matrix

| Scenario                                     | Expected Behavior                                              |
| -------------------------------------------- | -------------------------------------------------------------- |
| Element doesn't exist after 30s              | Error: "Timed out waiting for element '{selector}' to exist"   |
| Element exists but hidden                    | Error: "Element '{selector}' is not visible (display: none)"   |
| Element is animating                         | Wait up to 5s for stability, then error if still moving        |
| Element is under modal                       | Error: "Element '{selector}' is obscured by div#modal.overlay" |
| Element is disabled                          | Error: "Element '{selector}' is disabled"                      |
| Element is in viewport but partially clipped | Action proceeds (center point is used)                         |
| Element becomes visible during wait          | Action proceeds once all checks pass                           |
| Frame is cross-origin                        | Error: "Cannot access cross-origin frame '{selector}'"         |
| Selector matches multiple elements           | First match is used (consistent with querySelector)            |
