use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use chromiumoxide::Page;

const DOM_READY_POLL: Duration = Duration::from_millis(100);
const EVALUATION_TIMEOUT: Duration = Duration::from_secs(5);
const LAZY_LOAD_TIMEOUT: Duration = Duration::from_secs(20);
const SETTLE_QUIET_WINDOW_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettleResult {
    pub settled: bool,
    pub elapsed_ms: u64,
}

pub fn is_dom_ready(state: &str) -> bool {
    matches!(state, "interactive" | "complete")
}

/// Wait until the page has an interactive DOM after a navigation response.
pub async fn wait_for_dom_ready(page: &Page, timeout: Duration) -> Result<()> {
    let start = Instant::now();

    loop {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(anyhow!(
                "Timed out after {}ms waiting for the document to become interactive",
                timeout.as_millis()
            ));
        }
        let evaluation_timeout = remaining.min(EVALUATION_TIMEOUT);
        let result = tokio::time::timeout(evaluation_timeout, page.evaluate("document.readyState"))
            .await
            .map_err(|_| anyhow!("Timed out reading document readiness"))??;
        let state: String = result.into_value().unwrap_or_default();
        if is_dom_ready(&state) {
            return Ok(());
        }

        tokio::time::sleep(remaining.min(DOM_READY_POLL)).await;
    }
}

/// Scroll through the document to trigger viewport-based lazy loading.
pub async fn trigger_lazy_loading(page: &Page) -> Result<()> {
    tokio::time::timeout(
        LAZY_LOAD_TIMEOUT,
        page.evaluate(
            r#"(async () => {
                const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
                const MAX_STEPS = 150;
                const STEP = Math.max(100, Math.floor(window.innerHeight * 0.8));
                let previousY = -1;
                let steps = 0;

                while (steps < MAX_STEPS) {
                    const bodyHeight = document.body ? document.body.scrollHeight : 0;
                    const root = document.scrollingElement || document.documentElement;
                    const bottom = Math.max(0, bodyHeight, document.documentElement.scrollHeight,
                        root.scrollHeight) - window.innerHeight;
                    if (window.scrollY >= bottom) break;

                    window.scrollBy(0, STEP);
                    await sleep(100);
                    if (window.scrollY <= previousY) break;
                    previousY = window.scrollY;
                    steps++;
                }

                window.scrollTo(0, 0);
                // Lazy-load handlers are often debounced after scrolling stops.
                await sleep(500);
                return { steps };
            })()"#,
        ),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "Timed out after {}ms while triggering lazy loading",
            LAZY_LOAD_TIMEOUT.as_millis()
        )
    })??;
    Ok(())
}

/// Freeze active CSS/Web Animations and hide the text caret before capture.
pub async fn freeze_animations(page: &Page) -> Result<(u64, u64)> {
    let result = tokio::time::timeout(
        EVALUATION_TIMEOUT,
        page.evaluate(
            r#"(() => {
                let completed = 0;
                let frozen = 0;
                for (const animation of document.getAnimations({ subtree: true })) {
                    if (animation.playState !== 'running' && animation.playState !== 'pending') continue;
                    const timing = animation.effect && animation.effect.getTiming();
                    if (timing && timing.iterations === Infinity) {
                        try {
                            animation.pause();
                            animation.currentTime = 0;
                            frozen++;
                        } catch (_) {}
                        continue;
                    }
                    try {
                        animation.finish();
                        completed++;
                    } catch (_) {
                        animation.pause();
                        frozen++;
                    }
                }

                const styleId = '__regressify_capture_stability__';
                if (!document.getElementById(styleId)) {
                    const style = document.createElement('style');
                    style.id = styleId;
                    style.textContent = '*, *::before, *::after { caret-color: transparent !important; }';
                    (document.head || document.documentElement).appendChild(style);
                }
                return { completed, frozen };
            })()"#,
        ),
    )
    .await
    .map_err(|_| anyhow!("Timed out while freezing page animations"))??;
    let value: serde_json::Value = result.into_value().unwrap_or(serde_json::Value::Null);
    let completed = value
        .get("completed")
        .and_then(|field| field.as_u64())
        .unwrap_or(0);
    let frozen = value
        .get("frozen")
        .and_then(|field| field.as_u64())
        .unwrap_or(0);
    Ok((completed, frozen))
}

/// Wait for fonts, image decoding, DOM quiet, and stable document dimensions.
pub async fn wait_for_page_settle(page: &Page, timeout: Duration) -> Result<SettleResult> {
    let script = build_settle_script(timeout.as_millis() as u64, SETTLE_QUIET_WINDOW_MS);
    let budget = timeout.saturating_add(Duration::from_secs(2));
    let result = tokio::time::timeout(budget, page.evaluate(script.as_str()))
        .await
        .map_err(|_| anyhow!("Settle wait exceeded {}ms budget", budget.as_millis()))??;
    let value = result.value().unwrap_or(&serde_json::Value::Null);

    Ok(SettleResult {
        settled: value
            .get("settled")
            .and_then(|field| field.as_bool())
            .unwrap_or(false),
        elapsed_ms: value
            .get("elapsed")
            .and_then(|field| field.as_u64())
            .unwrap_or(0),
    })
}

fn build_settle_script(timeout_ms: u64, quiet_window_ms: u64) -> String {
    format!(
        r#"(async () => {{
            const SETTLE_TIMEOUT = {timeout_ms};
            const QUIET_WINDOW = {quiet_window_ms};
            const POLL = 100;
            const start = Date.now();
            const deadline = start + SETTLE_TIMEOUT;
            const remaining = () => Math.max(0, deadline - Date.now());
            const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
            const waitWithinDeadline = (promise) => new Promise((resolve) => {{
                const ms = remaining();
                if (ms === 0) {{ resolve(false); return; }}
                let done = false;
                const finish = (completed) => {{
                    if (done) return;
                    done = true;
                    clearTimeout(timer);
                    resolve(completed);
                }};
                const timer = setTimeout(() => finish(false), ms);
                Promise.resolve(promise).then(() => finish(true), () => finish(true));
            }});

            let lastMutation = Date.now();
            let resourceGeneration = 0;
            const relevantAttributes = new Set([
                'class', 'style', 'src', 'srcset', 'href', 'width', 'height', 'hidden', 'open',
            ]);
            const observer = new MutationObserver((mutations) => {{
                const relevant = mutations.some((mutation) =>
                    mutation.type !== 'attributes' || relevantAttributes.has(mutation.attributeName)
                );
                if (relevant) {{
                    lastMutation = Date.now();
                    resourceGeneration++;
                }}
            }});
            observer.observe(document.documentElement, {{
                attributes: true, childList: true, subtree: true, characterData: true,
                attributeFilter: Array.from(relevantAttributes),
            }});

            const waitForImage = async (image) => {{
                if (!image.complete) {{
                    const loaded = new Promise((resolve) => {{
                        image.addEventListener('load', resolve, {{ once: true }});
                        image.addEventListener('error', resolve, {{ once: true }});
                    }});
                    if (!await waitWithinDeadline(loaded)) return false;
                }}
                if (image.complete && image.naturalWidth > 0 && typeof image.decode === 'function')
                    return waitWithinDeadline(image.decode());
                return true;
            }};
            const waitForResources = async () => {{
                let fontsReady = true;
                if (document.fonts && document.fonts.ready)
                    fontsReady = await waitWithinDeadline(document.fonts.ready);
                const imageResults = await Promise.all(Array.from(document.images).map(waitForImage));
                return fontsReady && imageResults.every(Boolean);
            }};

            const signature = () => {{
                const root = document.documentElement;
                const body = document.body || root;
                return [root.scrollWidth, root.scrollHeight, body.scrollWidth, body.scrollHeight,
                    window.innerWidth, window.innerHeight].join('x');
            }};

            let lastSignature = '';
            let stableCount = 0;
            let settled = false;
            let checkedGeneration = -1;
            let resourcesReady = false;
            while (remaining() > 0) {{
                if (checkedGeneration !== resourceGeneration) {{
                    const generationBeforeCheck = resourceGeneration;
                    resourcesReady = await waitForResources();
                    checkedGeneration = resourceGeneration;
                    if (generationBeforeCheck !== checkedGeneration) {{
                        resourcesReady = false;
                        continue;
                    }}
                }}
                await sleep(Math.min(POLL, remaining()));
                const currentSignature = signature();
                const quiet = Date.now() - lastMutation >= QUIET_WINDOW;
                if (resourcesReady && currentSignature === lastSignature && quiet) {{
                    stableCount++;
                    if (stableCount >= 2) {{ settled = true; break; }}
                }} else {{
                    stableCount = 0;
                }}
                lastSignature = currentSignature;
            }}

            observer.disconnect();
            return {{ settled, elapsed: Date.now() - start }};
        }})()"#
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use super::*;

    #[test]
    fn dom_ready_accepts_interactive_and_complete_states() {
        assert!(!is_dom_ready("loading"));
        assert!(is_dom_ready("interactive"));
        assert!(is_dom_ready("complete"));
    }

    #[test]
    fn settle_script_has_one_deadline_and_decodes_images() {
        let script = build_settle_script(5_000, 500);

        assert!(script.contains("const deadline = start + SETTLE_TIMEOUT"));
        assert!(script.contains("document.images"));
        assert!(script.contains("image.decode"));
        assert!(script.contains("resourceGeneration"));
        assert!(script.contains("attributeFilter"));
        assert!(!script.contains("requestAnimationFrame"));
    }

    #[test]
    fn settle_script_is_valid_javascript_when_node_is_available() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let script_path = temp.path().join("settle.js");
        fs::write(&script_path, build_settle_script(5_000, 500)).unwrap();
        let output = Command::new("node")
            .arg("--check")
            .arg(&script_path)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
