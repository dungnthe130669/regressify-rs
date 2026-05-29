use anyhow::{Result, anyhow};
use chromiumoxide::Page;

use crate::types::Frame;

/// Information about an actionable element's position and size.
#[derive(Debug, Clone)]
pub struct ElementInfo {
    /// X coordinate of element center relative to its document (frame or main).
    pub center_x: f64,
    /// Y coordinate of element center relative to its document (frame or main).
    pub center_y: f64,
    /// Element width.
    pub width: f64,
    /// Element height.
    pub height: f64,
}

/// Configuration for actionability timeouts.
#[derive(Debug, Clone)]
pub struct ActionabilityConfig {
    /// Timeout (ms) for waiting for element to exist in the DOM.
    pub element_timeout_ms: u64,
    /// Timeout (ms) for actionability checks (visible, stable, not obscured).
    pub actionability_timeout_ms: u64,
    /// Polling interval (ms) between checks.
    pub poll_interval_ms: u64,
}

impl Default for ActionabilityConfig {
    fn default() -> Self {
        Self {
            element_timeout_ms: 30_000,
            actionability_timeout_ms: 5_000,
            poll_interval_ms: 100,
        }
    }
}

// ─── Escaping Helpers ────────────────────────────────────────────────────────

/// Escape a string for use inside a JS single-quoted string literal.
pub fn escape_js_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Escape a selector for safe insertion into CSS rule `innerHTML`.
/// Prevents breaking out of CSS context or injecting HTML.
pub fn escape_css_rule(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "\\<")
        .replace('>', "\\>")
}

// ─── Frame Helpers ───────────────────────────────────────────────────────────

/// Extract frame selectors from a Frame option.
pub fn get_frame_selectors(frame: &Option<Frame>) -> Option<Vec<String>> {
    match frame {
        Some(Frame::Single(s)) => Some(vec![s.clone()]),
        Some(Frame::Multiple(v)) => Some(v.clone()),
        None => None,
    }
}

/// Build a JS expression that navigates into nested iframes and returns
/// the target document at the deepest level.
pub fn build_frame_document_js(selectors: &[String]) -> String {
    let mut js = "document".to_string();
    for sel in selectors {
        js = format!(
            "{}.querySelector('{}').contentDocument",
            js,
            escape_js_string(sel)
        );
    }
    js
}

/// Build a JS expression that resolves the target element, optionally inside frames.
fn build_element_js(selector: &str, frame_selectors: &Option<Vec<String>>) -> String {
    let escaped_sel = escape_js_string(selector);
    if let Some(frames) = frame_selectors {
        let doc = build_frame_document_js(frames);
        format!("{}.querySelector('{}')", doc, escaped_sel)
    } else {
        format!("document.querySelector('{}')", escaped_sel)
    }
}

// ─── Actionability Checks ────────────────────────────────────────────────────

/// Wait for an element to exist in the DOM. Polls with interval until timeout.
pub async fn wait_for_selector(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<()> {
    let frame_selectors = get_frame_selectors(frame);
    let element_js = build_element_js(selector, &frame_selectors);
    let script = format!("(() => {{ return !!({element_js}); }})()");

    let timeout = std::time::Duration::from_millis(config.element_timeout_ms);
    let poll = std::time::Duration::from_millis(config.poll_interval_ms);
    let start = std::time::Instant::now();

    loop {
        let result = page.evaluate(script.as_str()).await?;
        if result.into_value::<bool>().unwrap_or(false) {
            return Ok(());
        }
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timed out after {}ms waiting for element '{}' to exist in the DOM",
                config.element_timeout_ms,
                selector
            ));
        }
        tokio::time::sleep(poll).await;
    }
}

/// Wait for element to be visible: non-zero bounding box, not hidden, opacity > 0.
pub async fn wait_for_visible(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<()> {
    let frame_selectors = get_frame_selectors(frame);
    let element_js = build_element_js(selector, &frame_selectors);
    let script = format!(
        r#"(() => {{
            const el = {element_js};
            if (!el) return {{ visible: false, reason: 'not_found' }};
            const rect = el.getBoundingClientRect();
            const style = window.getComputedStyle(el);
            if (rect.width === 0 || rect.height === 0)
                return {{ visible: false, reason: 'zero_size' }};
            if (style.visibility === 'hidden')
                return {{ visible: false, reason: 'visibility_hidden' }};
            if (style.display === 'none')
                return {{ visible: false, reason: 'display_none' }};
            if (parseFloat(style.opacity) === 0)
                return {{ visible: false, reason: 'zero_opacity' }};
            return {{ visible: true }};
        }})()"#
    );

    let timeout = std::time::Duration::from_millis(config.actionability_timeout_ms);
    let poll = std::time::Duration::from_millis(config.poll_interval_ms);
    let start = std::time::Instant::now();

    loop {
        let result = page.evaluate(script.as_str()).await?;
        let val: serde_json::Value = result.into_value().unwrap_or(serde_json::Value::Null);

        if val
            .get("visible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Ok(());
        }

        if start.elapsed() > timeout {
            let reason = val
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(anyhow!(
                "Element '{}' is not visible after {}ms (reason: {})",
                selector,
                config.actionability_timeout_ms,
                reason
            ));
        }
        tokio::time::sleep(poll).await;
    }
}

/// Wait for element to be stable (bounding box not changing between 2 consecutive checks).
pub async fn wait_for_stable(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<()> {
    let frame_selectors = get_frame_selectors(frame);
    let element_js = build_element_js(selector, &frame_selectors);
    let script = format!(
        r#"(() => {{
            const el = {element_js};
            if (!el) return null;
            const rect = el.getBoundingClientRect();
            return {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }};
        }})()"#
    );

    let timeout = std::time::Duration::from_millis(config.actionability_timeout_ms);
    let poll = std::time::Duration::from_millis(config.poll_interval_ms);
    let start = std::time::Instant::now();

    let mut prev_rect: Option<(f64, f64, f64, f64)> = None;

    loop {
        let result = page.evaluate(script.as_str()).await?;
        let val: serde_json::Value = result.into_value().unwrap_or(serde_json::Value::Null);

        if let (Some(x), Some(y), Some(w), Some(h)) = (
            val.get("x").and_then(|v| v.as_f64()),
            val.get("y").and_then(|v| v.as_f64()),
            val.get("width").and_then(|v| v.as_f64()),
            val.get("height").and_then(|v| v.as_f64()),
        ) {
            if let Some((px, py, pw, ph)) = prev_rect
                && (x - px).abs() < 1.0
                && (y - py).abs() < 1.0
                && (w - pw).abs() < 1.0
                && (h - ph).abs() < 1.0
            {
                return Ok(());
            }
            prev_rect = Some((x, y, w, h));
        }

        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Element '{}' is not stable after {}ms (bounding box still changing)",
                selector,
                config.actionability_timeout_ms,
            ));
        }
        tokio::time::sleep(poll).await;
    }
}

/// Scroll element into view (center of viewport).
pub async fn scroll_into_view(page: &Page, selector: &str, frame: &Option<Frame>) -> Result<()> {
    let frame_selectors = get_frame_selectors(frame);
    let element_js = build_element_js(selector, &frame_selectors);
    let script = format!(
        r#"(() => {{
            const el = {element_js};
            if (!el) return false;
            el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }});
            return true;
        }})()"#
    );

    let result = page.evaluate(script).await?;
    if !result.into_value::<bool>().unwrap_or(false) {
        return Err(anyhow!(
            "Cannot scroll element '{}' into view — element not found",
            selector
        ));
    }
    // Brief wait for scroll to settle
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(())
}

/// Check that the element is not obscured by another element at its center point.
/// Returns the element's bounding rect info on success.
pub async fn check_not_obscured(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
) -> Result<ElementInfo> {
    let frame_selectors = get_frame_selectors(frame);
    let element_js = build_element_js(selector, &frame_selectors);

    // Determine which document to call elementFromPoint on
    let doc_js = if let Some(frames) = &frame_selectors {
        build_frame_document_js(frames)
    } else {
        "document".to_string()
    };

    let script = format!(
        r#"(() => {{
            const el = {element_js};
            if (!el) return {{ error: 'not_found' }};
            const rect = el.getBoundingClientRect();
            const cx = rect.x + rect.width / 2;
            const cy = rect.y + rect.height / 2;
            const topEl = {doc_js}.elementFromPoint(cx, cy);
            if (!topEl) return {{ error: 'no_element_at_point' }};
            if (el === topEl || el.contains(topEl)) {{
                return {{
                    ok: true,
                    center_x: cx,
                    center_y: cy,
                    width: rect.width,
                    height: rect.height
                }};
            }}
            // Element is obscured — report what's covering it
            const tag = topEl.tagName.toLowerCase();
            const id = topEl.id ? '#' + topEl.id : '';
            const cls = topEl.className && typeof topEl.className === 'string'
                ? '.' + topEl.className.trim().split(/\s+/).join('.')
                : '';
            return {{ error: 'obscured', by: tag + id + cls }};
        }})()"#
    );

    let result = page.evaluate(script).await?;
    let val: serde_json::Value = result.into_value().unwrap_or(serde_json::Value::Null);

    if val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Ok(ElementInfo {
            center_x: val.get("center_x").and_then(|v| v.as_f64()).unwrap_or(0.0),
            center_y: val.get("center_y").and_then(|v| v.as_f64()).unwrap_or(0.0),
            width: val.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0),
            height: val.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0),
        });
    }

    let error = val
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match error {
        "obscured" => {
            let by = val.get("by").and_then(|v| v.as_str()).unwrap_or("unknown");
            Err(anyhow!(
                "Element '{}' is obscured by '{}' at its center point",
                selector,
                by
            ))
        }
        "not_found" => Err(anyhow!(
            "Element '{}' not found for obscured check",
            selector
        )),
        "no_element_at_point" => Err(anyhow!(
            "No element found at center point of '{}' (element may be outside viewport)",
            selector
        )),
        _ => Err(anyhow!(
            "Obscured check failed for '{}': {}",
            selector,
            error
        )),
    }
}

/// Check that a form element is enabled (not disabled).
pub async fn check_enabled(page: &Page, selector: &str, frame: &Option<Frame>) -> Result<()> {
    let frame_selectors = get_frame_selectors(frame);
    let element_js = build_element_js(selector, &frame_selectors);
    let script = format!(
        r#"(() => {{
            const el = {element_js};
            if (!el) return {{ error: 'not_found' }};
            if (el.disabled === true) return {{ disabled: true }};
            return {{ disabled: false }};
        }})()"#
    );

    let result = page.evaluate(script).await?;
    let val: serde_json::Value = result.into_value().unwrap_or(serde_json::Value::Null);

    if val
        .get("disabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Err(anyhow!("Element '{}' is disabled", selector));
    }
    if val.get("error").is_some() {
        return Err(anyhow!(
            "Element '{}' not found for enabled check",
            selector
        ));
    }
    Ok(())
}

/// Calculate the cumulative offset of nested iframes relative to the top-level viewport.
/// Returns (offset_x, offset_y) that should be added to element coordinates within the frame.
pub async fn get_frame_offset(page: &Page, frame: &Option<Frame>) -> Result<(f64, f64)> {
    let frame_selectors = match get_frame_selectors(frame) {
        Some(s) => s,
        None => return Ok((0.0, 0.0)),
    };

    if frame_selectors.is_empty() {
        return Ok((0.0, 0.0));
    }

    // Build JS that calculates cumulative offset through nested iframes
    let mut js_parts = Vec::new();
    let mut doc = "document".to_string();

    for sel in &frame_selectors {
        let escaped = escape_js_string(sel);
        js_parts.push(format!(
            r#"(() => {{
                const frame = {doc}.querySelector('{escaped}');
                if (!frame) return null;
                const rect = frame.getBoundingClientRect();
                return {{ x: rect.x, y: rect.y }};
            }})()"#
        ));
        doc = format!("{}.querySelector('{}').contentDocument", doc, escaped);
    }

    // We need to evaluate each frame offset sequentially and sum them
    let script = format!(
        r#"(() => {{
            let totalX = 0, totalY = 0;
            let currentDoc = document;
            const selectors = [{}];
            for (const sel of selectors) {{
                const frame = currentDoc.querySelector(sel);
                if (!frame) return {{ error: 'frame_not_found', selector: sel }};
                const rect = frame.getBoundingClientRect();
                totalX += rect.x;
                totalY += rect.y;
                currentDoc = frame.contentDocument;
                if (!currentDoc) return {{ error: 'cross_origin_frame', selector: sel }};
            }}
            return {{ x: totalX, y: totalY }};
        }})()"#,
        frame_selectors
            .iter()
            .map(|s| format!("'{}'", escape_js_string(s)))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let result = page.evaluate(script).await?;
    let val: serde_json::Value = result.into_value().unwrap_or(serde_json::Value::Null);

    if let Some(error) = val.get("error").and_then(|v| v.as_str()) {
        let sel = val
            .get("selector")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        return Err(anyhow!(
            "Frame offset calculation failed: {} ('{}')",
            error,
            sel
        ));
    }

    let x = val.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = val.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    Ok((x, y))
}

/// Full actionability check: wait for element to exist, be visible, stable,
/// scroll into view, verify not obscured, and check enabled.
/// Returns element center coordinates (relative to its document context).
pub async fn ensure_actionable(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<ElementInfo> {
    // 1. Wait for element to exist
    wait_for_selector(page, selector, frame, config).await?;

    // 2. Wait for element to be visible
    wait_for_visible(page, selector, frame, config).await?;

    // 3. Wait for element to be stable
    wait_for_stable(page, selector, frame, config).await?;

    // 4. Scroll element into viewport
    scroll_into_view(page, selector, frame).await?;

    // Brief pause after scroll for re-render
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 5. Check element is not obscured (also returns position info)
    let info = check_not_obscured(page, selector, frame).await?;

    // 6. Check element is enabled (for form controls)
    check_enabled(page, selector, frame).await?;

    Ok(info)
}

/// Partial actionability check for focus: existence, visibility, scroll, enabled.
/// No obscured check needed (focus doesn't require clicking).
pub async fn ensure_focusable(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<()> {
    wait_for_selector(page, selector, frame, config).await?;
    wait_for_visible(page, selector, frame, config).await?;
    scroll_into_view(page, selector, frame).await?;
    check_enabled(page, selector, frame).await?;
    Ok(())
}

/// Minimal check: just wait for element to exist (for scroll, hide, remove).
pub async fn ensure_exists(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<()> {
    wait_for_selector(page, selector, frame, config).await
}

// ─── CDP Mouse Helpers ───────────────────────────────────────────────────────

/// Perform a CDP click at the given viewport coordinates.
pub async fn cdp_click(page: &Page, x: f64, y: f64) -> Result<()> {
    use chromiumoxide::cdp::browser_protocol::input::{
        DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
    };

    // Move to position
    let move_cmd = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseMoved)
        .x(x)
        .y(y)
        .build()
        .map_err(|e| anyhow!("Failed to build mouse move: {:?}", e))?;
    page.execute(move_cmd).await?;

    // Press
    let press_cmd = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MousePressed)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| anyhow!("Failed to build mouse press: {:?}", e))?;
    page.execute(press_cmd).await?;

    // Release
    let release_cmd = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseReleased)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| anyhow!("Failed to build mouse release: {:?}", e))?;
    page.execute(release_cmd).await?;

    Ok(())
}

/// Perform a CDP hover (mouse move) at the given viewport coordinates.
pub async fn cdp_hover(page: &Page, x: f64, y: f64) -> Result<()> {
    use chromiumoxide::cdp::browser_protocol::input::{
        DispatchMouseEventParams, DispatchMouseEventType,
    };

    let move_cmd = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseMoved)
        .x(x)
        .y(y)
        .build()
        .map_err(|e| anyhow!("Failed to build mouse move: {:?}", e))?;
    page.execute(move_cmd).await?;

    Ok(())
}
