# Skill: Implement or Fix a Test Action

## When to Use

Use this skill when:
- Creating a new browser automation action (e.g., `drag`, `type`, `doubleClick`)
- Fixing an existing action that violates real-user behavior simulation
- Adding pre-action validation (visibility, interactability, stability) to existing actions
- Modifying how an action handles iframes/frames

## Key Files

- `src/types/actions.rs` — Action struct definitions
- `src/test/test_action.rs` — Action execution (`exec_test_action`)
- `src/test/test_scenario.rs` — Scenario orchestration (calls `exec_test_action`)
- `src/test/test_suites.rs` — Top-level orchestration (runs all scenarios concurrently)
- `docs/actions.md` — Action documentation

## Error Handling Philosophy

Each scenario is an **independent test**:

- If an action fails → the scenario fails immediately (return `Err`, do NOT continue remaining actions).
- A failed scenario MUST NOT prevent other scenarios from running. The orchestrator catches errors per-scenario and logs them.
- **Never** use `panic!`, `.unwrap()` on fallible ops, or `.ok()` to swallow errors inside action handlers.
- Use `?` to propagate all errors. Use `anyhow::bail!()` for validation failures.
- The user expects a full pass/fail report for ALL scenarios every run.

## Implementation Checklist

### 1. Define the Action Type (`src/types/actions.rs`)

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAction {
    pub new_action: String,  // CSS selector (always the primary property)
    pub frame: Option<Frame>,
    // ... additional properties
}
```

- Add the variant to `TestAction` enum. **Order matters** — `#[serde(untagged)]` tries variants top-to-bottom.
- Place variants with MORE required fields BEFORE variants with fewer (avoids false matches).
- The primary property name should match the action name (e.g., `click` property for `ClickAction`).

### 2. Implement Pre-Action Validation (MANDATORY)

Every interactive action (click, hover, check, uncheck, focus, input, press, select) MUST implement these checks IN ORDER:

```rust
// Step 1: Wait for element to exist (30s timeout, 100ms poll)
// Step 2: Wait for element to be visible (non-zero bbox, not hidden, opacity > 0)
// Step 3: Wait for element to be stable (bbox not changing between 2 frames)
// Step 4: Scroll element into viewport (scrollIntoView({ block: 'center' }))
// Step 5: Check element is not obscured (elementFromPoint at center)
// Step 6: Check element is enabled (for form controls)
```

**JavaScript for visibility check:**
```javascript
(() => {
    const el = document.querySelector('{selector}');
    if (!el) return { exists: false };
    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    return {
        exists: true,
        visible: rect.width > 0 && rect.height > 0
            && style.visibility !== 'hidden'
            && style.display !== 'none'
            && parseFloat(style.opacity) > 0,
        rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
        disabled: el.disabled === true
    };
})()
```

**JavaScript for obscured check:**
```javascript
(() => {
    const el = document.querySelector('{selector}');
    const rect = el.getBoundingClientRect();
    const cx = rect.x + rect.width / 2;
    const cy = rect.y + rect.height / 2;
    const topEl = document.elementFromPoint(cx, cy);
    if (!topEl) return { obscured: true, by: null };
    if (el === topEl || el.contains(topEl)) return { obscured: false };
    return {
        obscured: true,
        by: topEl.tagName + (topEl.id ? '#' + topEl.id : '') + (topEl.className ? '.' + topEl.className.split(' ').join('.') : '')
    };
})()
```

**JavaScript for stability check:**
```javascript
// Call this twice with ~100ms gap, compare results
(() => {
    const el = document.querySelector('{selector}');
    if (!el) return null;
    const rect = el.getBoundingClientRect();
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
})()
```

### 3. Handle iframe Context

If `frame` is specified, all checks run inside the frame:

```rust
fn get_frame_selectors(frame: &Option<Frame>) -> Option<Vec<String>> {
    match frame {
        Some(Frame::Single(s)) => Some(vec![s.clone()]),
        Some(Frame::Multiple(v)) => Some(v.clone()),
        None => None,
    }
}

fn build_frame_document_js(selectors: &[String]) -> String {
    let mut js = "document".to_string();
    for sel in selectors {
        js = format!("{}.querySelector('{}').contentDocument", js, sel.replace('\'', "\\'"));
    }
    js
}
```

When building validation JS for frames, replace `document` with the frame document expression.

### 4. Error Messages

Errors must be descriptive and actionable:

```rust
// BAD
anyhow::bail!("timeout");

// GOOD
anyhow::bail!(
    "Action '{}' timed out after {}ms waiting for element '{}' to be visible. \
     Element exists but has display:none.",
    action_name, timeout_ms, selector
);
```

### 5. CDP vs JS Actions

- Use **CDP commands** for actions that affect browser state in ways JS cannot:
  - `Input.dispatchMouseEvent` for click/hover (real pointer movement)
  - `Input.dispatchKeyEvent` for press (real keyboard input)
  - `DOM.focus` for focus
  - `DOM.setFileInputFiles` for file uploads

- Use **JS evaluation** for:
  - Visibility/interactability checks
  - Hide/remove (style injection)
  - Scroll into view
  - Select option manipulation

**NEVER use JS `el.click()` for the click action** — it bypasses overlay detection and doesn't simulate real user behavior. Use CDP `Input.dispatchMouseEvent` at the element's coordinates.

### 6. Update Documentation (`docs/actions.md`)

Add a section following the existing format:

```markdown
## action_name

Brief description.

| Property | Type     | Required | Description  |
| -------- | -------- | -------- | ------------ |
| `prop`   | `string` | yes      | What it does |

\```yaml
actions:
  - action_name: selector
\```

**Implementation:** Describe behavior.

**Rust types:** `ActionNameAction` in `src/types/actions.rs`, handled in `src/test/test_action.rs`.
```

## Action-Specific Implementation Notes

### click
- Get element center coordinates from bounding rect
- Use CDP `Input.dispatchMouseEvent` with type `mouseMoved` to center
- Then `mousePressed` (button: 0, clickCount: 1)
- Then `mouseReleased` (button: 0, clickCount: 1)
- Do NOT use JS `.click()` — it bypasses overlay detection

### hover
- Get element center coordinates
- Use CDP `Input.dispatchMouseEvent` with type `mouseMoved`
- This properly triggers CSS `:hover` states
- Do NOT use JS `mouseover` event dispatch — it won't activate `:hover` CSS

### check / uncheck
- Verify element is `<input type="checkbox">` or `<input type="radio">`
- For `check`: only click if `el.checked === false`
- For `uncheck`: only click if `el.checked === true`
- If already in desired state, the action is a no-op (not an error)

### input
- Focus element first
- If `append` is not true, clear value: `el.value = ''`
- Use CDP `Input.dispatchKeyEvent` for each character (simulates real typing)
- OR use `element.type_str()` from chromiumoxide
- Dispatch `input` and `change` events for framework compatibility

### press
- Must dispatch full key sequence: `keyDown` → `char` (for printable) → `keyUp`
- Handle modifier keys (Shift, Ctrl, Alt, Meta)
- Map key names to proper `code`, `key`, and `keyCode` values

### select
- Verify element is `<select>`
- Set option selected state via JS
- Dispatch `input` and `change` events

## Common Mistakes to Avoid

1. **Using `.ok()` to swallow errors** — Errors indicate test failures, not noise
2. **Using JS `.click()` for click action** — Bypasses all real-user checks
3. **Not waiting for element existence** — `find_element` fails immediately if not in DOM
4. **Not escaping selector strings** — CSS selector injection vulnerability
5. **Assuming frame contentDocument is accessible** — Cross-origin frames will fail
6. **Not checking if element moved between check and action** — TOCTOU race condition
7. **Hardcoding timeouts** — Should be configurable or at least constants
