use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use chromiumoxide::Page;

use crate::config::ResolvedConfig;
use crate::diagnostics::{
    record::{DiagnosticEvent, Field, FieldValue, SafeValue, Severity, Stage},
    runtime as diagnostics,
};

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

pub async fn wait_for_screenshot_delay(delay: Option<f64>, _log_prefix: &str) {
    if let Some(delay) = delay
        && delay > 0.0
    {
        diagnostics::event(
            Stage::ScreenshotSettling,
            DiagnosticEvent::Waiting,
            Severity::Debug,
            [(Field::TimeoutMs, FieldValue::Number(delay as u64))],
        );
        tokio::time::sleep(Duration::from_millis(delay as u64)).await;
    }
}

pub async fn prepare_screenshot(
    page: &Page,
    config: &ResolvedConfig,
    log_prefix: &str,
) -> Result<()> {
    diagnostics::stage(
        Stage::ScreenshotSettling,
        [(
            Field::TimeoutMs,
            FieldValue::Number(config.settle_timeout_ms),
        )],
        prepare_screenshot_inner(page, config, log_prefix),
    )
    .await
}

async fn prepare_screenshot_inner(
    page: &Page,
    config: &ResolvedConfig,
    log_prefix: &str,
) -> Result<()> {
    wait_for_screenshot_delay(config.delay, log_prefix).await;

    if config.freeze_animations {
        stabilize_capture_animations(page, log_prefix).await?;
    }

    if config.settle_timeout_ms > 0 {
        match wait_for_page_settle(page, Duration::from_millis(config.settle_timeout_ms)).await {
            Ok(result) if result.settled => {
                diagnostics::event(
                    Stage::ScreenshotSettling,
                    DiagnosticEvent::Decision,
                    Severity::Debug,
                    [
                        (Field::State, FieldValue::Kind(SafeValue::Complete)),
                        (Field::ElapsedMs, FieldValue::Number(result.elapsed_ms)),
                    ],
                );
            }
            Ok(result) => {
                diagnostics::event(
                    Stage::ScreenshotSettling,
                    DiagnosticEvent::Decision,
                    Severity::Warn,
                    [
                        (Field::Reason, FieldValue::Kind(SafeValue::Timeout)),
                        (Field::ElapsedMs, FieldValue::Number(result.elapsed_ms)),
                    ],
                );
            }
            Err(_) => {
                diagnostics::event(
                    Stage::ScreenshotSettling,
                    DiagnosticEvent::Decision,
                    Severity::Warn,
                    [(Field::State, FieldValue::Kind(SafeValue::Failed))],
                );
            }
        }
    }

    if config.freeze_animations {
        // Resources that completed during settlement can start new animations.
        stabilize_capture_animations(page, log_prefix).await?;
    }

    diagnostics::stage(
        Stage::ScreenshotSettling,
        [(Field::Condition, FieldValue::Kind(SafeValue::Caret))],
        hide_text_carets(page),
    )
    .await?;

    Ok(())
}

async fn hide_text_carets(page: &Page) -> Result<()> {
    tokio::time::timeout(
        EVALUATION_TIMEOUT,
        page.evaluate(
            r#"(() => {
                const styleId = '__regressify_capture_stability__';
                if (!document.getElementById(styleId)) {
                    const style = document.createElement('style');
                    style.id = styleId;
                    style.textContent = '*, *::before, *::after { caret-color: transparent !important; }';
                    (document.head || document.documentElement).appendChild(style);
                }
            })()"#,
        ),
    )
    .await
    .map_err(|_| anyhow!("Timed out while hiding text carets before capture"))??;
    Ok(())
}

pub async fn stabilize_capture_animations(page: &Page, _log_prefix: &str) -> Result<()> {
    let (completed, frozen) = freeze_animations(page).await?;
    if completed > 0 || frozen > 0 {
        diagnostics::event(
            Stage::ScreenshotSettling,
            DiagnosticEvent::Decision,
            Severity::Debug,
            [
                (Field::Condition, FieldValue::Kind(SafeValue::Animation)),
                (Field::CompletedAnimations, FieldValue::Number(completed)),
                (Field::FrozenAnimations, FieldValue::Number(frozen)),
            ],
        );
    }
    Ok(())
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
        let result = tokio::time::timeout(
            evaluation_timeout,
            super::runtime::evaluate_main_document(page, "document.readyState"),
        )
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
    diagnostics::stage(
        Stage::ScreenshotSettling,
        [(Field::Condition, FieldValue::Kind(SafeValue::LazyLoading))],
        trigger_lazy_loading_inner(page),
    )
    .await
}

async fn trigger_lazy_loading_inner(page: &Page) -> Result<()> {
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

                    window.scrollBy({ left: 0, top: STEP, behavior: 'instant' });
                    await sleep(100);
                    if (window.scrollY <= previousY) break;
                    previousY = window.scrollY;
                    steps++;
                }

                window.scrollTo({ left: 0, top: 0, behavior: 'instant' });
                // Lazy-load handlers are often debounced after scrolling stops.
                const deadline = performance.now() + 3000;
                let stableSince = null;
                while (performance.now() < deadline) {
                    const now = performance.now();
                    const atTop = Math.abs(window.scrollX) < 1 && Math.abs(window.scrollY) < 1;
                    if (atTop) {
                        if (stableSince === null) stableSince = now;
                        if (now - stableSince >= 500) return { steps };
                    } else {
                        stableSince = null;
                        window.scrollTo({ left: 0, top: 0, behavior: 'instant' });
                    }
                    await sleep(100);
                }
                throw new Error('Timed out waiting for lazy-load scrolling to return to the top');
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

/// Freeze active CSS/Web Animations before capture.
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
                    checkedGeneration = generationBeforeCheck;
                    if (checkedGeneration !== resourceGeneration) {{
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

    #[tokio::test]
    async fn screenshot_delay_waits_for_configured_milliseconds() {
        let start = Instant::now();

        wait_for_screenshot_delay(Some(25.0), "delay-test").await;

        assert!(start.elapsed() >= Duration::from_millis(25));
    }

    #[tokio::test]
    async fn screenshot_delay_skips_missing_or_non_positive_values() {
        for delay in [None, Some(0.0), Some(-1.0)] {
            tokio::time::timeout(
                Duration::from_millis(1),
                wait_for_screenshot_delay(delay, "delay-test"),
            )
            .await
            .expect("Missing or non-positive screenshot delays must not wait");
        }
    }

    #[tokio::test]
    #[ignore = "requires managed Chromium; run regressify install"]
    async fn screenshot_preparation_hides_carets_without_blurring_inputs() -> Result<()> {
        use chromiumoxide::{
            cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams,
        };

        let (mut browser, handler, directory) =
            crate::test::browser_tests::launch_browser("capture-caret", Default::default()).await?;
        let result: Result<()> = async {
            for (freeze_animations, tag) in [
                (false, "input"),
                (true, "input"),
                (false, "textarea"),
                (true, "textarea"),
                (false, "div"),
                (true, "div"),
            ] {
                let page = browser.new_page("about:blank").await?;
                page.evaluate(format!(
                    "window.fixtureTag = {}",
                    serde_json::to_string(tag)?
                ))
                .await?;
                page.evaluate(
                    r#"(() => {
                        const input = document.createElement(window.fixtureTag);
                        input.style.cssText = 'caret-color: rgb(255, 0, 255); width: 240px; height: 50px; font: 20px monospace';
                        if (window.fixtureTag === 'div') {
                            input.contentEditable = 'true';
                            input.textContent = 'test@example.com';
                        } else {
                            input.value = 'test@example.com';
                        }
                        document.body.appendChild(input);
                        input.focus();
                        if (window.fixtureTag === 'div') {
                            document.getSelection().setBaseAndExtent(input.firstChild, 4, input.firstChild, 4);
                        } else {
                            input.setSelectionRange(4, 4);
                        }
                        window.fixtureInput = input;
                        window.fixtureBlurEvents = 0;
                        input.addEventListener('blur', () => window.fixtureBlurEvents++);
                        const animated = document.createElement('div');
                        animated.style.cssText = 'position: fixed; left: -100px; width: 10px; height: 10px';
                        document.body.appendChild(animated);
                        window.fixtureAnimation = animated.animate(
                            [{ opacity: 1 }, { opacity: 0.5 }],
                            { duration: 60000, iterations: Infinity }
                        );
                    })()"#,
                )
                .await?;
                let config = ResolvedConfig {
                    freeze_animations,
                    delay: None,
                    settle_timeout_ms: 0,
                    ..Default::default()
                };
                let mut previous_pixels = None;
                for capture_index in 0..2 {
                    if capture_index > 0 {
                        tokio::time::sleep(Duration::from_millis(350)).await;
                    }
                    prepare_screenshot(&page, &config, "caret-test").await?;
                    let state: serde_json::Value = page
                        .evaluate(
                            r#"({
                                caret: getComputedStyle(window.fixtureInput).caretColor,
                                focused: document.activeElement === window.fixtureInput,
                                selection: window.fixtureTag === 'div'
                                    ? [document.getSelection().anchorOffset, document.getSelection().focusOffset]
                                    : [window.fixtureInput.selectionStart, window.fixtureInput.selectionEnd],
                                value: window.fixtureInput.value || window.fixtureInput.textContent,
                                blurEvents: window.fixtureBlurEvents,
                                animation: window.fixtureAnimation.playState,
                            })"#,
                        )
                        .await?
                        .into_value()?;
                    anyhow::ensure!(
                        state["caret"] == "rgba(0, 0, 0, 0)",
                        "{tag} caret remains visible with freezeAnimations={freeze_animations}: {state}"
                    );
                    anyhow::ensure!(
                        state["focused"] == true
                            && state["selection"] == serde_json::json!([4, 4])
                            && state["value"] == "test@example.com"
                            && state["blurEvents"] == 0,
                        "Screenshot preparation changed the focused input: {state}"
                    );
                    let expected_animation = if freeze_animations { "paused" } else { "running" };
                    anyhow::ensure!(state["animation"] == expected_animation);
                    let png = page
                        .screenshot(
                            ScreenshotParams::builder()
                                .format(CaptureScreenshotFormat::Png)
                                .from_surface(true)
                                .omit_background(false)
                                .build(),
                        )
                        .await?;
                    let image = lodepng::decode32(png)?;
                    anyhow::ensure!(
                        image.buffer.iter().any(|pixel| (pixel.r, pixel.g, pixel.b) != (255, 255, 255)),
                        "{tag} screenshot is blank"
                    );
                    anyhow::ensure!(
                        !image.buffer.iter().any(|pixel| (pixel.r, pixel.g, pixel.b) == (255, 0, 255)),
                        "{tag} screenshot contains caret pixels"
                    );
                    if let Some(previous) = &previous_pixels {
                        anyhow::ensure!(
                            previous == &image.buffer,
                            "{tag} screenshots changed across the caret blink interval"
                        );
                    }
                    previous_pixels = Some(image.buffer);
                }
                page.close().await?;
            }
            Ok(())
        }
        .await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }

    #[tokio::test]
    #[ignore = "requires managed Chromium; run regressify install"]
    async fn lazy_loading_waits_for_stable_offsets_and_bounds_stalled_scroll() -> Result<()> {
        let (mut browser, handler, directory) =
            crate::test::browser_tests::launch_browser("scroll-completion", Default::default())
                .await?;
        let result: Result<()> = async {
            for mode in ["short", "delayed", "interrupted", "stalled"] {
                let page = browser.new_page("about:blank").await?;
                page.evaluate(format!(
                    r#"(() => {{
                        const mode = {};
                        document.body.style.cssText = mode === 'short'
                            ? 'margin: 0; height: 50px; width: 50px'
                            : 'margin: 0; height: 2500px; width: 5000px';
                        window.fixture = {{ lastMove: null }};
                        const scrollTo = window.scrollTo.bind(window);
                        let requested = false;
                        if (mode === 'delayed') {{
                            window.scrollTo = (...args) => {{
                                if (requested) return;
                                requested = true;
                                setTimeout(() => {{
                                    scrollTo(...args);
                                    window.fixture.lastMove = performance.now();
                                }}, 700);
                            }};
                        }} else if (mode === 'interrupted') {{
                            window.scrollTo = (...args) => {{
                                scrollTo(...args);
                                if (requested) return;
                                requested = true;
                                setTimeout(() => {{
                                    scrollTo({{ left: 200, top: 150, behavior: 'instant' }});
                                    window.fixture.lastMove = performance.now();
                                }}, 250);
                            }};
                        }} else if (mode === 'stalled') {{
                            window.scrollTo = () => {{}};
                        }}
                    }})()"#,
                    serde_json::to_string(mode)?
                ))
                .await?;
                let start = Instant::now();
                let scrolling = trigger_lazy_loading(&page).await;
                if mode == "stalled" {
                    let error = scrolling
                        .err()
                        .ok_or_else(|| anyhow!("Blocked scrolling must fail instead of capturing"))?;
                    anyhow::ensure!(
                        format!("{error:#}").contains("return to the top"),
                        "Unexpected blocked-scroll error: {error:#}"
                    );
                    anyhow::ensure!(start.elapsed() < LAZY_LOAD_TIMEOUT);
                } else {
                    scrolling?;
                    let state: serde_json::Value = page
                        .evaluate(
                            "({ x: window.scrollX, y: window.scrollY, quietMs: window.fixture.lastMove === null ? null : performance.now() - window.fixture.lastMove })",
                        )
                        .await?
                        .into_value()?;
                    anyhow::ensure!(
                        state["x"] == 0 && state["y"] == 0,
                        "{mode} returned before reaching the top: {state}"
                    );
                    anyhow::ensure!(start.elapsed() >= Duration::from_millis(500));
                    if mode != "short" {
                        anyhow::ensure!(
                            state["quietMs"].as_f64().is_some_and(|quiet| quiet >= 500.0),
                            "{mode} returned without a quiet window after movement: {state}"
                        );
                    }
                }
                page.close().await?;
            }
            Ok(())
        }
        .await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }

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
        assert!(script.contains("checkedGeneration = generationBeforeCheck"));
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
