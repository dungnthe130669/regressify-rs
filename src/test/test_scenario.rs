use std::{
    future::Future,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use chromiumoxide::Browser;
use colored::Colorize;

use crate::{
    common,
    config::{self, ResolvedConfig, WorkspaceConfig},
    helpers::{OnceLockExtensions, slugify},
    test::{
        capture::{save_screenshot, state_path, validate_state_name, write_error_screenshot},
        test_action::{ActionExecutionContext, exec_test_action},
    },
    types::{Needs, ScreenshotType, TestAction, TestScenario, TestSuite, UrlReplacement},
};

/// Collect all prerequisite actions from `needs` dependencies recursively.
/// Walks the dependency graph depth-first and returns actions in execution order.
fn collect_needs_actions<'a>(
    scenario: &'a TestScenario,
    all_scenarios: &'a [TestScenario],
    visited: &mut Vec<String>,
) -> Vec<&'a TestAction> {
    let mut actions = Vec::new();

    let needed_ids = match &scenario.needs {
        Some(Needs::String(id)) => vec![id.as_str()],
        Some(Needs::StringArray(ids)) => ids.iter().map(|s| s.as_str()).collect(),
        None => return actions,
    };

    for needed_id in needed_ids {
        if visited.contains(&needed_id.to_string()) {
            continue; // Already processed — avoid cycles
        }

        if let Some(dep_scenario) = all_scenarios.iter().find(|s| {
            s.id.as_deref()
                .map(|id| id.eq_ignore_ascii_case(needed_id))
                .unwrap_or(false)
        }) {
            visited.push(needed_id.to_string());
            // Recursively collect dependencies of this dependency first
            let dep_actions = collect_needs_actions(dep_scenario, all_scenarios, visited);
            actions.extend(dep_actions);
            // Then add this dependency's own actions
            if let Some(ref dep_acts) = dep_scenario.actions {
                actions.extend(dep_acts.iter());
            }
        } else {
            eprintln!(
                "    {} Dependency scenario '{}' not found",
                "WARNING:".yellow(),
                needed_id
            );
        }
    }

    actions
}

/// Build a yellow `[index of total]` log prefix, padding the index to the width
/// of `total` so prefixes line up. Mirrors the TS `actions.js` logPrefix so logs
/// from scenarios/viewports running in parallel can be filtered by their pair.
pub(crate) fn make_log_prefix(index: usize, total: usize) -> String {
    let width = total.to_string().len();
    format!("[{index:>width$} of {total}]").yellow().to_string()
}

/// Resolve the viewports a scenario captures, as a flat list of capture targets.
/// Returns `[None]` when no viewport file is configured (capture once at the
/// browser's default size) or one `Some(viewport)` per resolved viewport (in
/// viewport-file order). Each entry becomes an independent capture job with its
/// own browser and dedicated log index — mirroring the TS/BackstopJS model where
/// every scenario/viewport pair is scaffolded upfront and runs independently.
pub(crate) fn scenario_capture_viewports(
    workspace_config: &WorkspaceConfig,
    test_suite: &TestSuite,
    scenario: &TestScenario,
) -> Result<Vec<Option<config::viewports::ViewportDef>>> {
    let resolved = config::resolve_config(workspace_config, test_suite, scenario);
    let viewports = resolve_scenario_viewports(&resolved)?;
    if viewports.is_empty() {
        Ok(vec![None])
    } else {
        Ok(viewports.into_iter().map(Some).collect())
    }
}

/// Ordered screenshot name prefixes for a scenario — one `<scenario-id>_<viewport>`
/// per resolved viewport (in viewport-file order), or a single bare
/// `<scenario-id>` when no viewport file is configured. These mirror the filename
/// prefixes produced by `capture` (the part before the `__<step|FINAL>` suffix)
/// and feed the report ordering manifest so the comparison report lists
/// screenshots in scenario-then-viewport order instead of alphabetically.
pub(crate) fn scenario_capture_prefixes(
    workspace_config: &WorkspaceConfig,
    test_suite: &TestSuite,
    scenario: &TestScenario,
) -> Result<Vec<String>> {
    let Some(scenario_id) = scenario.id.as_deref() else {
        return Ok(Vec::new());
    };
    let resolved = config::resolve_config(workspace_config, test_suite, scenario);
    let viewports = resolve_scenario_viewports(&resolved)?;
    if viewports.is_empty() {
        Ok(vec![scenario_id.to_string()])
    } else {
        Ok(viewports
            .iter()
            .map(|vp| format!("{}_{}", scenario_id, slugify(&vp.label)))
            .collect())
    }
}

/// Bundled inputs for executing one capture (a single scenario/viewport pair).
/// Grouping these avoids long argument lists across `exec_single_scenario` /
/// `exec_test_scenario`. Each capture is an independent unit with its own browser
/// and dedicated `index`, matching the TS/BackstopJS model.
#[derive(Clone, Copy)]
pub(crate) struct ScenarioRequest<'a> {
    pub test_suite: &'a TestSuite,
    pub scenario: &'a TestScenario,
    pub all_scenarios: &'a [TestScenario],
    pub workspace_config: &'a WorkspaceConfig,
    pub replacement_profile: &'a [UrlReplacement],
    pub apply_url_replacements: bool,
    /// The viewport to capture, or `None` to capture at the browser default size.
    pub viewport: Option<&'a config::viewports::ViewportDef>,
    /// 1-based dedicated index of this scenario/viewport pair in the global count.
    pub index: usize,
    /// Total number of scenario/viewport capture pairs across the whole run.
    pub total: usize,
}

/// Bundled inputs for a single scenario/viewport capture.
#[derive(Clone, Copy)]
struct CaptureContext<'a> {
    test_suite: &'a TestSuite,
    scenario: &'a TestScenario,
    all_scenarios: &'a [TestScenario],
    resolved: &'a ResolvedConfig,
    url: &'a str,
    url_replacements: &'a [UrlReplacement],
    viewport: Option<&'a config::viewports::ViewportDef>,
    log_prefix: &'a str,
}

pub async fn exec_test_scenario(browser: &Browser, req: ScenarioRequest<'_>) -> Result<()> {
    let ScenarioRequest {
        test_suite,
        scenario,
        all_scenarios,
        workspace_config,
        replacement_profile,
        apply_url_replacements: should_apply_url_replacements,
        viewport,
        index,
        total,
    } = req;

    let log_prefix = make_log_prefix(index, total);

    let label = scenario
        .label
        .as_deref()
        .or(scenario.id.as_deref())
        .unwrap_or(&scenario.url);

    eprintln!(
        "{} Executing scenario: {} ({})",
        log_prefix,
        label.cyan(),
        scenario.url
    );

    // Resolve configuration cascade: scenario -> suite -> workspace -> defaults
    let resolved = config::resolve_config(workspace_config, test_suite, scenario);

    let url_replacements = if should_apply_url_replacements {
        collect_url_replacements(test_suite, workspace_config, replacement_profile)
    } else {
        Vec::new()
    };
    let url = apply_url_replacements(&scenario.url, &url_replacements);

    // This request captures exactly one scenario/viewport pair. When a viewport
    // is present the full page lifecycle (navigation + actions) is replayed at
    // that size so layout and interactions reflect it; with no viewport we
    // capture once at the browser's default size. Each pair is an independent
    // task (own browser, own index), so a failure here only fails this pair.
    if let Some(vp) = viewport {
        eprintln!(
            "{} Capturing viewport '{}' ({}x{})...",
            log_prefix,
            vp.label.cyan(),
            vp.width,
            vp.height
        );
    }

    let capture_context = CaptureContext {
        test_suite,
        scenario,
        all_scenarios,
        resolved: &resolved,
        url: &url,
        url_replacements: &url_replacements,
        viewport,
        log_prefix: &log_prefix,
    };
    run_scenario_capture(browser, capture_context).await
}

/// Load and filter the viewports configured for this scenario.
/// Viewport definitions live in a YAML file resolved relative to the workspace
/// root (default `common/_viewports.yaml`). Returns an empty vec when no viewport
/// file exists, which means "capture once at the default browser size".
fn resolve_scenario_viewports(
    resolved: &ResolvedConfig,
) -> Result<Vec<config::viewports::ViewportDef>> {
    let root_dir = common::ROOT_DIR.get_required()?;
    let viewports_path = root_dir.join(&resolved.viewports_path);
    let all = config::viewports::load_viewports(&viewports_path)?;
    if all.is_empty() {
        return Ok(Vec::new());
    }
    config::viewports::resolve_viewports(resolved.viewport_names.as_deref(), &all)
}

/// Apply a viewport size to the page via CDP device-metrics emulation.
async fn apply_viewport(
    page: &chromiumoxide::Page,
    viewport: Option<&config::viewports::ViewportDef>,
) -> Result<()> {
    if let Some(vp) = viewport {
        use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
        let cmd = SetDeviceMetricsOverrideParams::builder()
            .width(vp.width as i64)
            .height(vp.height as i64)
            .device_scale_factor(1.0)
            .mobile(false)
            .build()
            .map_err(|e| anyhow!("Failed to build device metrics override: {:?}", e))?;
        page.execute(cmd).await?;
    }
    Ok(())
}

/// Run a single capture of the scenario at an optional viewport size.
///
/// On failure (the page under test errored before a screenshot was produced) an
/// error-placeholder image is written to the scenario's screenshot path so the
/// report and visual comparison still have an image to show. The original error
/// is then propagated so the scenario is still recorded as failed.
async fn run_scenario_capture(browser: &Browser, ctx: CaptureContext<'_>) -> Result<()> {
    let viewport_label = ctx.viewport.map(|v| v.label.as_str());
    let label = ctx
        .scenario
        .label
        .as_deref()
        .or(ctx.scenario.id.as_deref())
        .unwrap_or(&ctx.scenario.url);
    let timeout = std::time::Duration::from_millis(ctx.resolved.scenario_timeout_ms);

    match run_with_scenario_timeout(timeout, label, run_scenario_capture_inner(browser, ctx)).await
    {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Err(write_err) = write_error_screenshot(
                ctx.test_suite,
                ctx.scenario,
                viewport_label,
                &format!("{e:?}"),
            ) {
                eprintln!(
                    "{} {} Failed to write error placeholder screenshot: {:?}",
                    ctx.log_prefix,
                    "WARNING:".yellow(),
                    write_err
                );
            } else {
                eprintln!(
                    "{} Wrote error placeholder screenshot for failed scenario.",
                    ctx.log_prefix
                );
            }
            Err(e)
        }
    }
}

/// Run an entire scenario lifecycle under a hard deadline so an unresponsive
/// page or CDP operation cannot occupy a capture slot forever.
async fn run_with_scenario_timeout<T, F>(
    timeout: std::time::Duration,
    label: &str,
    operation: F,
) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    match tokio::time::timeout(timeout, operation).await {
        Ok(result) => result,
        Err(_) => Err(anyhow!(
            "Scenario '{}' exceeded its {}ms timeout",
            label,
            timeout.as_millis()
        )),
    }
}

/// Inner capture routine: drives the full page lifecycle and saves the
/// screenshot. Errors are handled by the `run_scenario_capture` wrapper.
async fn run_scenario_capture_inner(browser: &Browser, ctx: CaptureContext<'_>) -> Result<()> {
    let CaptureContext {
        test_suite,
        scenario,
        all_scenarios,
        resolved,
        url,
        url_replacements,
        viewport,
        log_prefix,
    } = ctx;
    let viewport_label = viewport.map(|v| v.label.as_str());

    // Create page with about:blank first, apply the viewport + CSP bypass, then
    // navigate so layout is computed at the intended size from the first paint.
    let page = browser.new_page("about:blank").await?;
    if resolved.bypass_csp {
        let cmd = chromiumoxide::cdp::browser_protocol::page::SetBypassCspParams::builder()
            .enabled(true)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build SetBypassCspParams: {}", e))?;
        page.execute(cmd).await?;
    }
    apply_viewport(&page, viewport).await?;

    // Load cookies before the first navigation so authenticated/session state
    // can affect the initial request.
    if let Some(ref state) = resolved.state {
        load_state_cookies(&page, state, log_prefix).await?;
    }
    let cookie_path = resolved
        .cookie_path
        .as_deref()
        .unwrap_or("common/_cookies.yaml");
    load_cookie_path(&page, cookie_path, log_prefix).await?;

    page.goto(url).await?;

    // localStorage requires an origin, so it is restored after navigation. If
    // anything was restored, reload so app code observes it during startup.
    if let Some(ref state) = resolved.state
        && load_state_local_storage(&page, state, log_prefix).await?
    {
        eprintln!(
            "{} Reloading page after restoring localStorage...",
            log_prefix
        );
        page.goto(url).await?;
    }

    // Apply CSS override if configured
    apply_css_override(&page, resolved, log_prefix).await?;

    for sel in &resolved.hide_selectors {
        inject_css(&page, sel, "visibility: hidden !important").await?;
    }

    for sel in &resolved.remove_selectors {
        inject_css(&page, sel, "display: none !important").await?;
    }

    expose_scenario_context(&page, scenario, resolved).await?;

    // Execute jsOnReadyPath script (defaults to common/_on-ready.js; skipped if
    // the file does not exist)
    apply_js_on_ready(&page, resolved, log_prefix).await?;

    // Execute prerequisite actions from `needs` dependencies
    let mut visited = Vec::new();
    let needs_actions = collect_needs_actions(scenario, all_scenarios, &mut visited);
    if !needs_actions.is_empty() {
        eprintln!(
            "{} Executing {} prerequisite actions from dependencies...",
            log_prefix,
            needs_actions.len()
        );
        for action in &needs_actions {
            exec_test_action(
                ActionExecutionContext {
                    page: &page,
                    test_suite,
                    scenario,
                    config: resolved,
                    url_replacements,
                    viewport_label,
                    log_prefix,
                },
                action,
            )
            .await?;
        }
    }

    // Execute this scenario's own actions
    if let Some(actions) = &scenario.actions {
        eprintln!("{} Executing {} actions...", log_prefix, actions.len());
        for action in actions {
            exec_test_action(
                ActionExecutionContext {
                    page: &page,
                    test_suite,
                    scenario,
                    config: resolved,
                    url_replacements,
                    viewport_label,
                    log_prefix,
                },
                action,
            )
            .await?;
        }
    }

    // Execute clickSelector if configured (clicks the element after all actions)
    if let Some(ref click_sel) = scenario.click_selector {
        eprintln!("{} Clicking selector: {}", log_prefix, click_sel);
        use crate::test::actionability::{self as act, ActionabilityConfig};
        let ac = ActionabilityConfig {
            element_timeout_ms: resolved.element_timeout_ms,
            actionability_timeout_ms: resolved.actionability_timeout_ms,
            ..Default::default()
        };
        let info = act::ensure_actionable(&page, click_sel, &None, &ac).await?;
        let frame_offset = act::get_frame_offset(&page, &None).await?;
        act::cdp_click(
            &page,
            frame_offset.0 + info.center_x,
            frame_offset.1 + info.center_y,
        )
        .await?;
    }

    // Apply postInteractionWait from resolved config
    if let Some(wait_ms) = resolved.post_interaction_wait
        && wait_ms > 0.0
    {
        eprintln!(
            "{} Waiting for {} ms after interactions...",
            log_prefix, wait_ms
        );
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms as u64)).await;
    }

    // Apply delay before screenshot
    if let Some(delay) = resolved.delay
        && delay > 0.0
    {
        eprintln!(
            "{} Delay: waiting {} ms before screenshot...",
            log_prefix, delay
        );
        tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
    }

    // Scroll page top-to-bottom then back to top to trigger lazy-loaded images
    if !resolved.no_scroll_top {
        eprintln!("{} Scrolling page to load lazy images...", log_prefix);
        let scroll_start = std::time::Instant::now();
        page.evaluate(
            r#"(async () => {
                const SCROLL_DISTANCE = 100;
                const SCROLL_DOWN_MAX = 100;
                const SCROLL_TOP_MAX = 100;

                if (!!window.visualTestScrollingBottom) return;

                const scrollToBottom = async () => {
                    window.visualTestScrollingBottom = true;
                    let counter = 0;
                    let lastScrollY = 0;
                    await new Promise((resolve) => {
                        const timer = setInterval(() => {
                            if (window.innerHeight + Math.ceil(window.scrollY) >= document.body.offsetHeight || counter > SCROLL_DOWN_MAX) {
                                clearInterval(timer);
                                window.visualTestScrollingBottom = false;
                                resolve();
                                return;
                            }
                            if (lastScrollY >= Math.ceil(window.scrollY)) {
                                counter++;
                            } else {
                                lastScrollY = Math.ceil(window.scrollY);
                                counter = 0;
                            }
                            window.scrollBy(0, SCROLL_DISTANCE);
                        }, 100);
                    });
                };

                const scrollToTop = async () => {
                    await new Promise((resolve) => {
                        const timer = setInterval(() => {
                            if (!window.visualTestScrollingBottom) {
                                clearInterval(timer);
                                resolve();
                                return;
                            }
                        }, 100);
                    });
                    await new Promise((resolve) => {
                        let counter = 0;
                        const timer = setInterval(() => {
                            if (window.scrollY === 0 || counter > SCROLL_TOP_MAX) {
                                clearInterval(timer);
                                resolve();
                                return;
                            }
                            counter++;
                            window.scrollTo(0, 0);
                        }, 100);
                    });
                };

                await scrollToBottom();
                await scrollToTop();

                // Wait for all images to be fully loaded or errored
                const IMG_TIMEOUT = 30000;
                const start = Date.now();
                const images = Array.from(document.querySelectorAll('img'));
                if (images.length > 0) {
                    await Promise.all(images.map(img => {
                        if (img.complete) return Promise.resolve();
                        return new Promise((resolve) => {
                            img.addEventListener('load', resolve, { once: true });
                            img.addEventListener('error', resolve, { once: true });
                            // Per-image timeout fallback
                            const remaining = IMG_TIMEOUT - (Date.now() - start);
                            if (remaining <= 0) { resolve(); return; }
                            setTimeout(resolve, remaining);
                        });
                    }));
                }
            })()"#,
        )
        .await?;
        eprintln!(
            "{} Lazy-load/scroll settled in {} ms.",
            log_prefix,
            scroll_start.elapsed().as_millis()
        );
    }

    // Wait for the page to settle before capturing: fonts ready, then layout +
    // DOM quiet for a short window. This prevents capturing while async content
    // (lazy widgets, populated dropdowns, animations re-triggered by the scroll)
    // is still rendering, which otherwise produces small non-deterministic diffs
    // between runs. `settle_timeout_ms == 0` disables it.
    if resolved.settle_timeout_ms > 0 {
        let settle_start = std::time::Instant::now();
        // Required quiet window: no DOM mutations and stable layout for this long.
        let quiet_window_ms: u64 = 500;
        let settle_js = format!(
            r#"(async () => {{
                const SETTLE_TIMEOUT = {settle_timeout};
                const QUIET_WINDOW = {quiet_window};
                const POLL = 100;
                const start = Date.now();
                const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

                // 1. Wait for web fonts to finish loading (bounded by timeout).
                if (document.fonts && document.fonts.ready) {{
                    try {{
                        await Promise.race([
                            document.fonts.ready,
                            sleep(SETTLE_TIMEOUT),
                        ]);
                    }} catch (e) {{ /* ignore */ }}
                }}

                // 2. Track DOM mutations to detect a quiet window.
                let lastMutation = Date.now();
                const mo = new MutationObserver(() => {{ lastMutation = Date.now(); }});
                mo.observe(document.documentElement, {{
                    attributes: true, childList: true, subtree: true, characterData: true,
                }});

                const sig = () => {{
                    const de = document.documentElement;
                    const b = document.body || de;
                    return de.scrollHeight + 'x' + b.scrollWidth + 'x' + b.scrollHeight;
                }};

                // 3. Wait until layout is stable AND no mutations for QUIET_WINDOW,
                //    confirmed across two consecutive polls. The loop is driven by
                //    setTimeout (not requestAnimationFrame): rAF callbacks never
                //    fire when the renderer is throttled/occluded, which would hang
                //    this promise forever. Timers always fire, so the loop is
                //    guaranteed to terminate within SETTLE_TIMEOUT.
                let lastSig = '';
                let stableCount = 0;
                let settled = false;
                while (Date.now() - start < SETTLE_TIMEOUT) {{
                    await sleep(POLL);
                    const s = sig();
                    const quiet = (Date.now() - lastMutation) >= QUIET_WINDOW;
                    if (s === lastSig && quiet) {{
                        stableCount++;
                        if (stableCount >= 2) {{ settled = true; break; }}
                    }} else {{
                        stableCount = 0;
                    }}
                    lastSig = s;
                }}

                mo.disconnect();
                return {{ settled, elapsed: Date.now() - start }};
            }})()"#,
            settle_timeout = resolved.settle_timeout_ms,
            quiet_window = quiet_window_ms,
        );

        // Hard safety net: never let a stuck evaluate hang the scenario task (and
        // therefore the whole `ref`/`test` run). Even though the JS is bounded by
        // SETTLE_TIMEOUT, a stalled CDP connection or throttled renderer could
        // prevent the promise from resolving, so cap the wait on the Rust side too.
        let settle_budget =
            std::time::Duration::from_millis(resolved.settle_timeout_ms.saturating_add(2_000));
        match tokio::time::timeout(settle_budget, page.evaluate(settle_js.as_str())).await {
            Ok(Ok(result)) => {
                let settled = result
                    .value()
                    .and_then(|v| v.get("settled"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if settled {
                    eprintln!(
                        "{} Page settled in {} ms before capture.",
                        log_prefix,
                        settle_start.elapsed().as_millis()
                    );
                } else {
                    eprintln!(
                        "{} Page did not settle within {} ms; capturing anyway.",
                        log_prefix, resolved.settle_timeout_ms
                    );
                }
            }
            Ok(Err(e)) => {
                eprintln!(
                    "{} Settle wait failed ({e}); capturing without settle.",
                    log_prefix
                );
            }
            Err(_) => {
                eprintln!(
                    "{} Settle wait exceeded {} ms budget; capturing anyway.",
                    log_prefix,
                    settle_budget.as_millis()
                );
            }
        }
    }

    eprintln!("{} Capturing final screenshot...", log_prefix);
    let capture_start = std::time::Instant::now();
    save_screenshot(
        test_suite,
        scenario,
        &page,
        ScreenshotType::Final,
        viewport_label,
    )
    .await?;
    eprintln!(
        "{} Screenshot captured in {} ms.",
        log_prefix,
        capture_start.elapsed().as_millis()
    );

    Ok(())
}

/// Apply urlReplacements from the test suite to a URL.
/// Supports both literal string replacement and regex replacement.
fn collect_url_replacements(
    test_suite: &TestSuite,
    workspace_config: &WorkspaceConfig,
    replacement_profile: &[UrlReplacement],
) -> Vec<UrlReplacement> {
    let mut combined = replacement_profile.to_vec();
    if let Some(ref workspace_replacements) = workspace_config.url_replacements {
        combined.extend(workspace_replacements.clone());
    }
    if let Some(ref suite_replacements) = test_suite.url_replacements {
        combined.extend(suite_replacements.clone());
    }
    combined
}

/// Apply URL replacements in declaration order.
fn apply_url_replacements(url: &str, replacements: &[UrlReplacement]) -> String {
    config::replacements::apply_replacements(url, replacements)
}

fn resolve_work_dir_path(path: &str) -> Result<Option<PathBuf>> {
    let root_dir = common::ROOT_DIR.get_required()?;
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root_dir.join(path)
    };

    if !candidate.exists() {
        return Ok(None);
    }

    let canonical_root = root_dir.canonicalize()?;
    let canonical_path = candidate.canonicalize()?;
    if !canonical_path.starts_with(&canonical_root) {
        anyhow::bail!("Path '{}' escapes the workspace root", path);
    }

    Ok(Some(canonical_path))
}

/// Load persisted state cookies before the first navigation.
async fn load_state_cookies(
    page: &chromiumoxide::Page,
    state: &Needs,
    log_prefix: &str,
) -> Result<()> {
    let state_names = match state {
        Needs::String(s) => vec![s.clone()],
        Needs::StringArray(v) => v.clone(),
    };

    for name in &state_names {
        validate_state_name(name)?;
        let state_path = state_path(name)?;

        if !state_path.exists() {
            eprintln!(
                "{} {} State file not found: {}",
                log_prefix,
                "WARNING:".yellow(),
                state_path.display()
            );
            continue;
        }

        eprintln!("{} Loading state: {}", log_prefix, name);
        let content = std::fs::read_to_string(&state_path)?;
        let state: serde_json::Value = serde_json::from_str(&content)?;

        // Warn if state has unexpected shape
        if let Some(c) = state.get("cookies")
            && !c.is_array()
        {
            eprintln!(
                "{} {} State '{}': 'cookies' is not an array; ignoring",
                log_prefix,
                "WARNING:".yellow(),
                name
            );
        }
        // Restore cookies
        if let Some(cookies) = state.get("cookies").and_then(|c| c.as_array()) {
            use chromiumoxide::cdp::browser_protocol::network::{CookieParam, CookieSameSite};
            for cookie in cookies {
                if let (Some(name), Some(value)) = (
                    cookie.get("name").and_then(|v| v.as_str()),
                    cookie.get("value").and_then(|v| v.as_str()),
                ) {
                    let mut param = CookieParam::builder().name(name).value(value);

                    if let Some(url) = cookie_url_from(cookie) {
                        param = param.url(url);
                    }
                    if let Some(domain) = cookie.get("domain").and_then(|v| v.as_str()) {
                        param = param.domain(domain);
                    }
                    if let Some(path) = cookie.get("path").and_then(|v| v.as_str()) {
                        param = param.path(path);
                    }
                    if let Some(secure) = cookie.get("secure").and_then(|v| v.as_bool()) {
                        param = param.secure(secure);
                    }
                    if let Some(http_only) = cookie.get("httpOnly").and_then(|v| v.as_bool()) {
                        param = param.http_only(http_only);
                    }
                    if let Some(expires) = cookie.get("expires").and_then(|v| v.as_f64()) {
                        use chromiumoxide::cdp::browser_protocol::network::TimeSinceEpoch;
                        param = param.expires(TimeSinceEpoch::new(expires));
                    }
                    if let Some(same_site) = cookie.get("sameSite").and_then(|v| v.as_str())
                        && let Some(ss) = parse_same_site(same_site)
                    {
                        param = param.same_site(ss);
                    }
                    let _ = CookieSameSite::Strict; // ensure import is exercised even if no sameSite

                    let cookie_param = param
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to build cookie param: {:?}", e))?;
                    page.set_cookie(cookie_param).await.map_err(|e| {
                        anyhow!("Failed to restore state cookie '{}': {:?}", name, e)
                    })?;
                }
            }
        }
    }

    Ok(())
}

/// Load persisted localStorage after navigation, once an origin exists.
async fn load_state_local_storage(
    page: &chromiumoxide::Page,
    state: &Needs,
    log_prefix: &str,
) -> Result<bool> {
    let state_names = match state {
        Needs::String(s) => vec![s.clone()],
        Needs::StringArray(v) => v.clone(),
    };

    let mut restored = false;

    for name in &state_names {
        validate_state_name(name)?;
        let state_path = state_path(name)?;

        if !state_path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&state_path)?;
        let state: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(ls) = state.get("localStorage")
            && !ls.is_object()
        {
            eprintln!(
                "{} {} State '{}': 'localStorage' is not an object; ignoring",
                log_prefix,
                "WARNING:".yellow(),
                name
            );
        }

        // Restore localStorage
        if let Some(local_storage) = state.get("localStorage").and_then(|v| v.as_object()) {
            if !local_storage.is_empty() {
                eprintln!("{} Restoring localStorage from state: {}", log_prefix, name);
                restored = true;
            }
            for (key, value) in local_storage {
                if let Some(val_str) = value.as_str() {
                    let script = format!(
                        "localStorage.setItem({}, {})",
                        serde_json::to_string(key)?,
                        serde_json::to_string(val_str)?
                    );
                    page.evaluate(script).await.map_err(|e| {
                        anyhow!("Failed to restore localStorage key '{}': {:?}", key, e)
                    })?;
                } else {
                    eprintln!(
                        "    {} State '{}': localStorage entry '{}' is not a string; skipping",
                        "WARNING:".yellow(),
                        name,
                        key
                    );
                }
            }
        }
    }

    Ok(restored)
}

/// Load cookies from a YAML file.
async fn load_cookie_path(
    page: &chromiumoxide::Page,
    cookie_path: &str,
    log_prefix: &str,
) -> Result<()> {
    let Some(cookie_path) = resolve_work_dir_path(cookie_path)? else {
        return Ok(()); // No cookie file — that's fine
    };

    eprintln!(
        "{} Loading cookies from: {}",
        log_prefix,
        cookie_path.display()
    );
    let content = std::fs::read_to_string(&cookie_path)?;
    let cookies: Vec<serde_json::Value> = serde_yaml::from_str(&content)?;

    use chromiumoxide::cdp::browser_protocol::network::{CookieParam, CookieSameSite};
    for cookie in &cookies {
        if let (Some(name), Some(value)) = (
            cookie.get("name").and_then(|v| v.as_str()),
            cookie.get("value").and_then(|v| v.as_str()),
        ) {
            let mut param = CookieParam::builder().name(name).value(value);

            if let Some(url) = cookie_url_from(cookie) {
                param = param.url(url);
            }
            if let Some(domain) = cookie.get("domain").and_then(|v| v.as_str()) {
                param = param.domain(domain);
            }
            if let Some(path) = cookie.get("path").and_then(|v| v.as_str()) {
                param = param.path(path);
            }
            if let Some(secure) = cookie.get("secure").and_then(|v| v.as_bool()) {
                param = param.secure(secure);
            }
            if let Some(http_only) = cookie.get("httpOnly").and_then(|v| v.as_bool()) {
                param = param.http_only(http_only);
            }
            if let Some(expires) = cookie.get("expires").and_then(|v| v.as_f64()) {
                use chromiumoxide::cdp::browser_protocol::network::TimeSinceEpoch;
                param = param.expires(TimeSinceEpoch::new(expires));
            }
            if let Some(same_site) = cookie.get("sameSite").and_then(|v| v.as_str())
                && let Some(ss) = parse_same_site(same_site)
            {
                param = param.same_site(ss);
            }
            let _ = CookieSameSite::Strict;

            let cookie_param = param
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build cookie param: {:?}", e))?;
            page.set_cookie(cookie_param)
                .await
                .map_err(|e| anyhow!("Failed to load cookie '{}': {:?}", name, e))?;
        }
    }

    Ok(())
}

/// Derive a cookie `url` from its JSON definition.
///
/// Cookies are loaded before the first navigation, while the page is still at
/// `about:blank`. chromiumoxide falls back to the current page URL when a
/// cookie has no explicit `url`, and rejects `about:blank` with
/// "Blank page can not have cookie". Supplying a `url` derived from the
/// cookie's `domain` avoids that fallback. An explicit `url` field wins.
fn cookie_url_from(cookie: &serde_json::Value) -> Option<String> {
    if let Some(url) = cookie.get("url").and_then(|v| v.as_str()) {
        return Some(url.to_string());
    }
    let domain = cookie.get("domain").and_then(|v| v.as_str())?;
    let host = domain.trim_start_matches('.');
    if host.is_empty() {
        return None;
    }
    let secure = cookie
        .get("secure")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let scheme = if secure { "https" } else { "http" };
    let path = cookie.get("path").and_then(|v| v.as_str()).unwrap_or("/");
    Some(format!("{scheme}://{host}{path}"))
}

/// Parse a SameSite string from cookie JSON into the CDP enum.
fn parse_same_site(
    s: &str,
) -> Option<chromiumoxide::cdp::browser_protocol::network::CookieSameSite> {
    use chromiumoxide::cdp::browser_protocol::network::CookieSameSite;
    match s {
        "Strict" | "strict" => Some(CookieSameSite::Strict),
        "Lax" | "lax" => Some(CookieSameSite::Lax),
        "None" | "none" => Some(CookieSameSite::None),
        _ => None,
    }
}

/// Apply CSS override from file if configured.
async fn apply_css_override(
    page: &chromiumoxide::Page,
    resolved: &ResolvedConfig,
    log_prefix: &str,
) -> Result<()> {
    if !resolved.use_css_override {
        return Ok(());
    }

    let css_path = resolved
        .css_override_path
        .as_deref()
        .unwrap_or("common/_override.css");

    let Some(css_path) = resolve_work_dir_path(css_path)? else {
        return Ok(()); // No override file — that's fine
    };

    eprintln!(
        "{} Applying CSS override: {}",
        log_prefix,
        css_path.display()
    );
    let css_content = std::fs::read_to_string(&css_path)?;
    let css_json = serde_json::to_string(&css_content)?;
    let script = format!(
        "(() => {{
            var style = document.createElement('style');
            style.type = 'text/css';
            style.textContent = {};
            document.head.appendChild(style);
        }})()",
        css_json
    );
    page.evaluate(script).await?;

    Ok(())
}

/// Execute the jsOnReady script from file if it exists.
/// Defaults to `common/_on-ready.js` (resolved relative to the workspace root).
/// Missing files are silently skipped, matching the TS engine-script behavior.
async fn apply_js_on_ready(
    page: &chromiumoxide::Page,
    resolved: &ResolvedConfig,
    log_prefix: &str,
) -> Result<()> {
    let js_path = resolved
        .js_on_ready_path
        .as_deref()
        .unwrap_or("common/_on-ready.js");

    let Some(js_path) = resolve_work_dir_path(js_path)? else {
        return Ok(()); // No script file — that's fine
    };

    eprintln!(
        "{} Executing jsOnReady script: {}",
        log_prefix,
        js_path.display()
    );
    let js_content = std::fs::read_to_string(&js_path)?;
    page.evaluate(js_content).await?;

    Ok(())
}

async fn expose_scenario_context(
    page: &chromiumoxide::Page,
    scenario: &TestScenario,
    resolved: &ResolvedConfig,
) -> Result<()> {
    let payload = serde_json::json!({
        "requiredLogin": resolved.required_login,
        "scenario": {
            "id": scenario.id.as_deref(),
            "label": scenario.label.as_deref(),
            "description": scenario.description.as_deref(),
            "url": scenario.url.as_str(),
            "requiredLogin": resolved.required_login,
        }
    });
    let payload_json = serde_json::to_string(&payload)?;
    let script = format!(
        "(() => {{ window.__REGRESSIFY__ = {payload}; window.regressify = window.__REGRESSIFY__; }})()",
        payload = payload_json
    );
    page.evaluate(script).await?;
    Ok(())
}

/// Inject a CSS rule for a selector.
async fn inject_css(page: &chromiumoxide::Page, selector: &str, rule: &str) -> Result<()> {
    use crate::test::actionability::escape_css_rule;
    let safe_selector = escape_css_rule(selector);
    let css_text = format!("{} {{ {} }}", safe_selector, rule);
    let css_json = serde_json::to_string(&css_text)?;
    let script = format!(
        "(() => {{
            var style = document.createElement('style');
            style.type = 'text/css';
            style.textContent = {};
            document.head.appendChild(style);
        }})()",
        css_json
    );
    page.evaluate(script).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scenario_timeout_cancels_stalled_work() {
        let result = run_with_scenario_timeout(
            std::time::Duration::from_millis(1),
            "stalled scenario",
            std::future::pending::<Result<()>>(),
        )
        .await;

        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("stalled scenario' exceeded its 1ms timeout")
        );
    }
}
