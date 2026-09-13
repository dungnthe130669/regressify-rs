use std::{future::Future, time::Duration};

use anyhow::{Result, anyhow};
use chromiumoxide::Page;

use crate::types::Frame;

/// Information about an actionable element's position.
#[derive(Debug, Clone)]
pub struct ElementInfo {
    /// X coordinate of element center relative to its document (frame or main).
    pub center_x: f64,
    /// Y coordinate of element center relative to its document (frame or main).
    pub center_y: f64,
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

fn actionability_timeout(config: &ActionabilityConfig) -> Duration {
    Duration::from_millis(config.actionability_timeout_ms)
}

async fn cdp_with_timeout<T, E, F>(timeout: Duration, operation: &str, future: F) -> Result<T>
where
    E: std::fmt::Display,
    F: Future<Output = std::result::Result<T, E>>,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(result) => result.map_err(|error| anyhow!("{}: {}", operation, error)),
        Err(_) => Err(anyhow!(
            "Timed out after {}ms while {}",
            timeout.as_millis(),
            operation
        )),
    }
}

async fn evaluate_value(
    page: &Page,
    script: impl Into<chromiumoxide::js::Evaluation>,
    timeout: Duration,
    operation: &str,
) -> Result<serde_json::Value> {
    let result = cdp_with_timeout(timeout, operation, page.evaluate(script)).await?;
    Ok(result.into_value().unwrap_or(serde_json::Value::Null))
}

// ─── Escaping Helpers ────────────────────────────────────────────────────────

/// Escape a string for use inside a JS single-quoted string literal.
pub fn escape_js_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\0', "")
}

/// Escape a selector for safe insertion into CSS rule `innerHTML`.
/// Prevents breaking out of CSS context or injecting HTML. Also escapes
/// control characters (newline/CR/tab/NUL) which would otherwise produce a JS
/// syntax error when the selector is interpolated into a single-quoted string.
pub fn escape_css_rule(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\0', "")
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

fn frame_selectors_js(selectors: &[String]) -> String {
    selectors
        .iter()
        .map(|selector| format!("'{}'", escape_js_string(selector)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, PartialEq, Eq)]
enum FrameAccess {
    Ready,
    WaitingForFrame(String),
}

fn parse_frame_access(value: &serde_json::Value) -> Result<FrameAccess> {
    if value.get("ok").and_then(|field| field.as_bool()) == Some(true) {
        return Ok(FrameAccess::Ready);
    }

    let error = value
        .get("error")
        .and_then(|field| field.as_str())
        .unwrap_or("unknown");
    let selector = value
        .get("selector")
        .and_then(|field| field.as_str())
        .unwrap_or("unknown");

    match error {
        "frame_not_found" => Ok(FrameAccess::WaitingForFrame(selector.to_string())),
        "not_a_frame" => {
            let tag = value
                .get("tag")
                .and_then(|field| field.as_str())
                .unwrap_or("unknown")
                .to_lowercase();
            Err(anyhow!(
                "Frame selector '{}' matches a <{}>, not an <iframe>",
                selector,
                tag
            ))
        }
        "cross_origin" => Err(anyhow!(
            "Cannot access cross-origin frame '{}' (its document is not reachable from the parent)",
            selector
        )),
        _ => Err(anyhow!("Frame '{}' is not accessible: {}", selector, error)),
    }
}

/// Check whether every frame in the chain is ready for target polling.
async fn frame_access(
    page: &Page,
    frame: &Option<Frame>,
    timeout: Duration,
) -> Result<FrameAccess> {
    let Some(frame_selectors) = get_frame_selectors(frame) else {
        return Ok(FrameAccess::Ready);
    };
    if frame_selectors.is_empty() {
        return Err(anyhow!("Frame selector chain cannot be empty"));
    }

    let selectors_js = frame_selectors_js(&frame_selectors);
    let script = format!(
        r#"(() => {{
            let doc = document;
            const selectors = [{selectors_js}];
            for (const sel of selectors) {{
                const frame = doc.querySelector(sel);
                if (!frame) return {{ error: 'frame_not_found', selector: sel }};
                if (frame.tagName !== 'IFRAME' && frame.tagName !== 'FRAME')
                    return {{ error: 'not_a_frame', selector: sel, tag: frame.tagName }};
                let inner = null;
                try {{ inner = frame.contentDocument; }} catch (e) {{ inner = null; }}
                if (!inner) return {{ error: 'cross_origin', selector: sel }};
                doc = inner;
            }}
            return {{ ok: true }};
        }})()"#
    );

    let value = evaluate_value(page, script, timeout, "checking iframe accessibility").await?;
    parse_frame_access(&value)
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
    let exists_operation = format!("checking whether element '{}' exists", selector);
    let mut waiting_for_frame = None;

    loop {
        if start.elapsed() >= timeout {
            if let Some(frame_selector) = waiting_for_frame {
                return Err(anyhow!(
                    "Timed out after {}ms waiting for frame '{}' before finding element '{}'",
                    config.element_timeout_ms,
                    frame_selector,
                    selector
                ));
            }
            return Err(anyhow!(
                "Timed out after {}ms waiting for element '{}' to exist in the DOM",
                config.element_timeout_ms,
                selector
            ));
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        let command_timeout = remaining.min(actionability_timeout(config));
        waiting_for_frame = match frame_access(page, frame, command_timeout).await? {
            FrameAccess::Ready => {
                let value =
                    evaluate_value(page, script.as_str(), command_timeout, &exists_operation)
                        .await?;
                if value.as_bool().unwrap_or(false) {
                    return Ok(());
                }
                None
            }
            FrameAccess::WaitingForFrame(frame_selector) => Some(frame_selector),
        };
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
            const view = el.ownerDocument.defaultView;
            if (!view) return {{ visible: false, reason: 'no_window' }};
            if (rect.width === 0 || rect.height === 0)
                return {{ visible: false, reason: 'zero_size' }};
            for (let current = el; current; current = current.parentElement) {{
                const style = current.ownerDocument.defaultView.getComputedStyle(current);
                if (style.visibility === 'hidden' || style.visibility === 'collapse')
                    return {{ visible: false, reason: 'visibility_hidden' }};
                if (style.display === 'none')
                    return {{ visible: false, reason: 'display_none' }};
                if (parseFloat(style.opacity) === 0)
                    return {{ visible: false, reason: 'zero_opacity' }};
            }}
            return {{ visible: true }};
        }})()"#
    );

    let timeout = std::time::Duration::from_millis(config.actionability_timeout_ms);
    let poll = std::time::Duration::from_millis(config.poll_interval_ms);
    let start = std::time::Instant::now();
    let visible_operation = format!("checking visibility of element '{}'", selector);
    let mut last_reason = "unknown".to_string();

    loop {
        if start.elapsed() >= timeout {
            return Err(anyhow!(
                "Element '{}' is not visible after {}ms (reason: {})",
                selector,
                config.actionability_timeout_ms,
                last_reason
            ));
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        let val = evaluate_value(page, script.as_str(), remaining, &visible_operation).await?;

        if val
            .get("visible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Ok(());
        }

        last_reason = val
            .get("reason")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();
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
    let stable_operation = format!("checking stability of element '{}'", selector);

    let mut prev_rect: Option<(f64, f64, f64, f64)> = None;
    let mut stable_streak: u32 = 0;
    const REQUIRED_STABLE_STREAK: u32 = 2;

    loop {
        if start.elapsed() >= timeout {
            return Err(anyhow!(
                "Element '{}' is not stable after {}ms (bounding box still changing)",
                selector,
                config.actionability_timeout_ms,
            ));
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        let val = evaluate_value(page, script.as_str(), remaining, &stable_operation).await?;

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
                stable_streak += 1;
                if stable_streak >= REQUIRED_STABLE_STREAK {
                    return Ok(());
                }
            } else {
                stable_streak = 0;
            }
            prev_rect = Some((x, y, w, h));
        } else {
            stable_streak = 0;
        }

        tokio::time::sleep(poll).await;
    }
}

/// Scroll element into view (center of viewport).
pub async fn scroll_into_view(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<()> {
    let frame_selectors = get_frame_selectors(frame);
    let selectors_js = frame_selectors
        .as_deref()
        .map(frame_selectors_js)
        .unwrap_or_default();
    let target_selector = escape_js_string(selector);
    let script = format!(
        r#"(() => {{
            const options = {{ block: 'center', inline: 'center', behavior: 'instant' }};
            const selectors = [{selectors_js}];
            let doc = document;
            for (const frameSelector of selectors) {{
                const frame = doc.querySelector(frameSelector);
                if (!frame) return {{ error: 'frame_not_found', selector: frameSelector }};
                frame.scrollIntoView(options);
                if (!frame.contentDocument)
                    return {{ error: 'cross_origin', selector: frameSelector }};
                doc = frame.contentDocument;
            }}
            const el = doc.querySelector('{target_selector}');
            if (!el) return {{ error: 'not_found' }};
            el.scrollIntoView(options);
            return {{ ok: true }};
        }})()"#
    );

    let operation = format!("scrolling element '{}' into view", selector);
    let value = evaluate_value(page, script, actionability_timeout(config), &operation).await?;
    if value.get("ok").and_then(|field| field.as_bool()) != Some(true) {
        let error = value
            .get("error")
            .and_then(|field| field.as_str())
            .unwrap_or("unknown");
        let frame_selector = value
            .get("selector")
            .and_then(|field| field.as_str())
            .unwrap_or("unknown");
        return Err(anyhow!(
            "Cannot scroll element '{}' into view: {} ('{}')",
            selector,
            error,
            frame_selector
        ));
    }
    // Brief wait for scroll to settle
    tokio::time::sleep(Duration::from_millis(50).min(actionability_timeout(config))).await;
    Ok(())
}

/// Check that the element is not obscured by another element at its center point.
/// Returns the element's bounding rect info on success.
pub async fn check_not_obscured(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
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
                    center_y: cy
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

    let operation = format!("checking whether element '{}' is obscured", selector);
    let val = evaluate_value(page, script, actionability_timeout(config), &operation).await?;

    if val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Ok(ElementInfo {
            center_x: val.get("center_x").and_then(|v| v.as_f64()).unwrap_or(0.0),
            center_y: val.get("center_y").and_then(|v| v.as_f64()).unwrap_or(0.0),
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
pub async fn check_enabled(
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
            if (!el) return {{ error: 'not_found' }};
            const disabled = el.matches(':disabled')
                || el.getAttribute('aria-disabled') === 'true';
            if (disabled) return {{ disabled: true }};
            return {{ disabled: false }};
        }})()"#
    );

    let operation = format!("checking whether element '{}' is enabled", selector);
    let val = evaluate_value(page, script, actionability_timeout(config), &operation).await?;

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

/// Calculate the cumulative content-box offset of nested iframes relative to the
/// top-level viewport. Each frame has already been centered by `scroll_into_view`.
pub async fn get_frame_offset(
    page: &Page,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<(f64, f64)> {
    let frame_selectors = match get_frame_selectors(frame) {
        Some(s) => s,
        None => return Ok((0.0, 0.0)),
    };

    if frame_selectors.is_empty() {
        return Err(anyhow!("Frame selector chain cannot be empty"));
    }

    let script = format!(
        r#"(() => {{
            let totalX = 0, totalY = 0;
            let currentDoc = document;
            const selectors = [{}];
            for (const sel of selectors) {{
                const frame = currentDoc.querySelector(sel);
                if (!frame) return {{ error: 'frame_not_found', selector: sel }};
                const rect = frame.getBoundingClientRect();
                const style = currentDoc.defaultView.getComputedStyle(frame);
                if (rect.width === 0 || rect.height === 0 || style.display === 'none'
                    || style.visibility === 'hidden' || style.visibility === 'collapse'
                    || parseFloat(style.opacity) === 0) {{
                    return {{ error: 'frame_not_visible', selector: sel }};
                }}
                totalX += rect.x + frame.clientLeft;
                totalY += rect.y + frame.clientTop;
                currentDoc = frame.contentDocument;
                if (!currentDoc) return {{ error: 'cross_origin_frame', selector: sel }};
            }}
            return {{ x: totalX, y: totalY }};
        }})()"#,
        frame_selectors_js(&frame_selectors)
    );

    let val = evaluate_value(
        page,
        script,
        actionability_timeout(config),
        "calculating iframe offset",
    )
    .await?;

    if let Some(error) = val.get("error").and_then(|v| v.as_str()) {
        let sel = val
            .get("selector")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        if error == "frame_not_visible" {
            return Err(anyhow!("Frame '{}' is not visible", sel));
        }
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

/// When the target element lives inside an iframe, verify that the iframe itself
/// is not obscured in the *parent* document at the point where the element will
/// be clicked. `check_not_obscured` only inspects the frame's own document, so a
/// parent-level overlay covering the whole iframe would otherwise go undetected
/// and a CDP click would land on the overlay.
///
/// `center_x`/`center_y` are the element's center relative to its own frame
/// document; this projects them to top-level viewport coordinates via the frame
/// offset and checks `document.elementFromPoint` at the top level.
async fn check_frame_not_obscured(
    page: &Page,
    frame: &Option<Frame>,
    center_x: f64,
    center_y: f64,
    config: &ActionabilityConfig,
) -> Result<()> {
    let frame_selectors = match get_frame_selectors(frame) {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(()),
    };

    let offset = get_frame_offset(page, frame, config).await?;
    let abs_x = offset.0 + center_x;
    let abs_y = offset.1 + center_y;

    // The top-level document only hit-tests down to the first-level iframe.
    let top_frame = escape_js_string(&frame_selectors[0]);
    let script = format!(
        r#"(() => {{
            const frame = document.querySelector('{top_frame}');
            if (!frame) return {{ error: 'frame_not_found' }};
            const topEl = document.elementFromPoint({abs_x}, {abs_y});
            if (!topEl) return {{ error: 'no_element_at_point' }};
            if (topEl === frame || frame.contains(topEl)) return {{ ok: true }};
            const tag = topEl.tagName.toLowerCase();
            const id = topEl.id ? '#' + topEl.id : '';
            const cls = topEl.className && typeof topEl.className === 'string'
                ? '.' + topEl.className.trim().split(/\s+/).join('.')
                : '';
            return {{ error: 'obscured', by: tag + id + cls }};
        }})()"#
    );

    let val = evaluate_value(
        page,
        script,
        actionability_timeout(config),
        "checking whether iframe is obscured",
    )
    .await?;

    if val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Ok(());
    }

    match val
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
    {
        "obscured" => {
            let by = val.get("by").and_then(|v| v.as_str()).unwrap_or("unknown");
            Err(anyhow!(
                "Element inside frame '{}' is obscured by parent-document element '{}'",
                frame_selectors[0],
                by
            ))
        }
        "no_element_at_point" => Err(anyhow!(
            "Frame '{}' is outside the viewport at the target point",
            frame_selectors[0]
        )),
        other => Err(anyhow!(
            "Parent-frame obscured check failed for '{}': {}",
            frame_selectors[0],
            other
        )),
    }
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
    scroll_into_view(page, selector, frame, config).await?;

    // 5. Check element is not obscured (also returns position info)
    let info = check_not_obscured(page, selector, frame, config).await?;

    // 5b. If inside a frame, also ensure the frame isn't obscured in the parent.
    check_frame_not_obscured(page, frame, info.center_x, info.center_y, config).await?;

    // 6. Check element is enabled (for form controls)
    check_enabled(page, selector, frame, config).await?;

    Ok(info)
}

/// Full actionability check for focus, typing, key presses, and selection.
/// These interactions must meet the same visibility, stability, viewport,
/// coverage, and enabled-state requirements as a pointer interaction.
pub async fn ensure_focusable(
    page: &Page,
    selector: &str,
    frame: &Option<Frame>,
    config: &ActionabilityConfig,
) -> Result<()> {
    ensure_actionable(page, selector, frame, config)
        .await
        .map(|_| ())
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
pub async fn cdp_click(page: &Page, x: f64, y: f64, config: &ActionabilityConfig) -> Result<()> {
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
    cdp_with_timeout(
        actionability_timeout(config),
        "dispatching mouse move for click",
        page.execute(move_cmd),
    )
    .await?;

    // Press
    let press_cmd = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MousePressed)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| anyhow!("Failed to build mouse press: {:?}", e))?;
    cdp_with_timeout(
        actionability_timeout(config),
        "dispatching mouse press",
        page.execute(press_cmd),
    )
    .await?;

    // Release
    let release_cmd = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseReleased)
        .x(x)
        .y(y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| anyhow!("Failed to build mouse release: {:?}", e))?;
    cdp_with_timeout(
        actionability_timeout(config),
        "dispatching mouse release",
        page.execute(release_cmd),
    )
    .await?;

    Ok(())
}

/// Perform a CDP hover (mouse move) at the given viewport coordinates.
pub async fn cdp_hover(page: &Page, x: f64, y: f64, config: &ActionabilityConfig) -> Result<()> {
    use chromiumoxide::cdp::browser_protocol::input::{
        DispatchMouseEventParams, DispatchMouseEventType,
    };

    let move_cmd = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseMoved)
        .x(x)
        .y(y)
        .build()
        .map_err(|e| anyhow!("Failed to build mouse move: {:?}", e))?;
    cdp_with_timeout(
        actionability_timeout(config),
        "dispatching mouse move for hover",
        page.execute(move_cmd),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cdp_timeout_bounds_stalled_operations() {
        let result = cdp_with_timeout(
            Duration::from_millis(1),
            "evaluating actionability",
            std::future::pending::<std::result::Result<(), std::io::Error>>(),
        )
        .await;

        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Timed out after 1ms while evaluating actionability")
        );
    }

    #[test]
    fn frame_access_distinguishes_retryable_missing_frame() {
        let value = serde_json::json!({
            "error": "frame_not_found",
            "selector": "#application-frame"
        });

        assert_eq!(
            parse_frame_access(&value).unwrap(),
            FrameAccess::WaitingForFrame("#application-frame".to_string())
        );
    }

    #[test]
    fn frame_access_rejects_non_frame_and_cross_origin_targets() {
        let non_frame = serde_json::json!({
            "error": "not_a_frame",
            "selector": ".panel",
            "tag": "DIV"
        });
        let cross_origin = serde_json::json!({
            "error": "cross_origin",
            "selector": "#external"
        });

        assert!(
            parse_frame_access(&non_frame)
                .unwrap_err()
                .to_string()
                .contains("matches a <div>")
        );
        assert!(
            parse_frame_access(&cross_origin)
                .unwrap_err()
                .to_string()
                .contains("cross-origin")
        );
    }

    #[test]
    fn frame_selector_literals_are_escaped() {
        assert_eq!(
            frame_selectors_js(&["iframe[name='checkout']".to_string()]),
            "'iframe[name=\\'checkout\\']'"
        );
    }
}
