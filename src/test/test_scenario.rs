use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chromiumoxide::Browser;
use colored::Colorize;

use crate::{
    common,
    config::{self, ResolvedConfig, WorkspaceConfig},
    helpers::OnceLockExtensions,
    test::{
        capture::{save_screenshot, write_error_screenshot},
        test_action::exec_test_action,
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

        if let Some(dep_scenario) = all_scenarios
            .iter()
            .find(|s| s.id.as_deref() == Some(needed_id))
        {
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

/// Count how many independent captures a scenario produces (one per resolved
/// viewport, or 1 when no viewport file is configured). Each scenario/viewport
/// pair is counted independently for the global log index/total.
pub(crate) fn scenario_capture_count(
    workspace_config: &WorkspaceConfig,
    test_suite: &TestSuite,
    scenario: &TestScenario,
) -> usize {
    let resolved = config::resolve_config(workspace_config, test_suite, scenario);
    resolve_scenario_viewports(&resolved)
        .map(|v| v.len().max(1))
        .unwrap_or(1)
}

pub async fn exec_test_scenario(
    browser: &Browser,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    all_scenarios: &[TestScenario],
    workspace_config: &WorkspaceConfig,
    replacement_profile: &[UrlReplacement],
    base_index: usize,
    total: usize,
) -> Result<()> {
    let label = scenario
        .label
        .as_deref()
        .or(scenario.id.as_deref())
        .unwrap_or(&scenario.url);

    eprintln!(
        "{} Executing scenario: {} ({})",
        make_log_prefix(base_index, total),
        label.cyan(),
        scenario.url
    );

    // Resolve configuration cascade: scenario -> suite -> workspace -> defaults
    let resolved = config::resolve_config(workspace_config, test_suite, scenario);

    // Apply urlReplacements to the scenario URL
    let url = apply_url_replacements(
        &scenario.url,
        test_suite,
        workspace_config,
        replacement_profile,
    );

    // Resolve the viewports to capture. When viewport config is present, each
    // viewport is captured as a separate screenshot (BackstopJS-style) and the
    // full page lifecycle (navigation + actions) is replayed per viewport so the
    // layout and interactions reflect that size. With no viewport config we
    // capture once at the browser's default size (backwards compatible).
    let viewports = resolve_scenario_viewports(&resolved)?;

    if viewports.is_empty() {
        run_scenario_capture(
            browser,
            test_suite,
            scenario,
            all_scenarios,
            &resolved,
            &url,
            None,
            &make_log_prefix(base_index, total),
        )
        .await?;
    } else {
        // Each viewport is an independent capture (BackstopJS-style). A failure
        // in one viewport (e.g. a timeout) must NOT stop the others from running,
        // so errors are collected and reported after all viewports execute. Each
        // failed viewport already gets its own error-placeholder screenshot.
        // Each scenario/viewport pair is counted independently for the log prefix.
        let mut failures: Vec<String> = Vec::new();
        for (i, vp) in viewports.iter().enumerate() {
            let log_prefix = make_log_prefix(base_index + i, total);
            eprintln!(
                "{} Capturing viewport '{}' ({}x{})...",
                log_prefix,
                vp.label.cyan(),
                vp.width,
                vp.height
            );
            if let Err(e) = run_scenario_capture(
                browser,
                test_suite,
                scenario,
                all_scenarios,
                &resolved,
                &url,
                Some(vp),
                &log_prefix,
            )
            .await
            {
                eprintln!(
                    "{} {} Viewport '{}' failed: {:?}",
                    log_prefix,
                    "ERROR:".red(),
                    vp.label,
                    e
                );
                failures.push(format!("{}: {:?}", vp.label, e));
            }
        }

        if !failures.is_empty() {
            return Err(anyhow!(
                "{} of {} viewport(s) failed:\n  {}",
                failures.len(),
                viewports.len(),
                failures.join("\n  ")
            ));
        }
    }

    Ok(())
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
    Ok(config::viewports::resolve_viewports(
        resolved.viewport_names.as_deref(),
        &all,
    ))
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
async fn run_scenario_capture(
    browser: &Browser,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    all_scenarios: &[TestScenario],
    resolved: &ResolvedConfig,
    url: &str,
    viewport: Option<&config::viewports::ViewportDef>,
    log_prefix: &str,
) -> Result<()> {
    let viewport_label = viewport.map(|v| v.label.as_str());
    match run_scenario_capture_inner(
        browser,
        test_suite,
        scenario,
        all_scenarios,
        resolved,
        url,
        viewport,
        log_prefix,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Err(write_err) =
                write_error_screenshot(test_suite, scenario, viewport_label, &format!("{e:?}"))
            {
                eprintln!(
                    "{} {} Failed to write error placeholder screenshot: {:?}",
                    log_prefix,
                    "WARNING:".yellow(),
                    write_err
                );
            } else {
                eprintln!(
                    "{} Wrote error placeholder screenshot for failed scenario.",
                    log_prefix
                );
            }
            Err(e)
        }
    }
}

/// Inner capture routine: drives the full page lifecycle and saves the
/// screenshot. Errors are handled by the `run_scenario_capture` wrapper.
#[allow(clippy::too_many_arguments)]
async fn run_scenario_capture_inner(
    browser: &Browser,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    all_scenarios: &[TestScenario],
    resolved: &ResolvedConfig,
    url: &str,
    viewport: Option<&config::viewports::ViewportDef>,
    log_prefix: &str,
) -> Result<()> {
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
    page.goto(url).await?;

    // Load state (cookies + localStorage) if configured at suite level
    if let Some(ref state) = test_suite.state {
        load_state(&page, state, log_prefix).await?;
    }

    // Load cookies from cookiePath (defaults to common/_cookies.yaml; skipped
    // if the file does not exist)
    let cookie_path = resolved
        .cookie_path
        .as_deref()
        .unwrap_or("common/_cookies.yaml");
    load_cookie_path(&page, cookie_path, log_prefix).await?;

    // Apply CSS override if configured
    apply_css_override(&page, resolved, log_prefix).await?;

    for sel in &resolved.hide_selectors {
        inject_css(&page, sel, "visibility: hidden !important").await?;
    }

    for sel in &resolved.remove_selectors {
        inject_css(&page, sel, "display: none !important").await?;
    }

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
                &page,
                test_suite,
                scenario,
                action,
                resolved,
                viewport_label,
                log_prefix,
            )
            .await?;
        }
    }

    // Execute this scenario's own actions
    if let Some(actions) = &scenario.actions {
        eprintln!("{} Executing {} actions...", log_prefix, actions.len());
        for action in actions {
            exec_test_action(
                &page,
                test_suite,
                scenario,
                action,
                resolved,
                viewport_label,
                log_prefix,
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
        eprintln!("{} Waiting for {} ms after interactions...", log_prefix, wait_ms);
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms as u64)).await;
    }

    // Apply delay before screenshot
    if let Some(delay) = resolved.delay
        && delay > 0.0
    {
        eprintln!("{} Delay: waiting {} ms before screenshot...", log_prefix, delay);
        tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
    }

    // Scroll page top-to-bottom then back to top to trigger lazy-loaded images
    if !resolved.no_scroll_top {
        eprintln!("{} Scrolling page to load lazy images...", log_prefix);
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
    }

    eprintln!("{} Capturing final screenshot...", log_prefix);
    save_screenshot(
        test_suite,
        scenario,
        &page,
        ScreenshotType::Final,
        viewport_label,
    )
    .await?;

    Ok(())
}

/// Apply urlReplacements from the test suite to a URL.
/// Supports both literal string replacement and regex replacement.
fn apply_url_replacements(
    url: &str,
    test_suite: &TestSuite,
    workspace_config: &WorkspaceConfig,
    replacement_profile: &[UrlReplacement],
) -> String {
    let mut result = url.to_string();
    result = config::replacements::apply_replacements(&result, replacement_profile);
    if let Some(ref replacements) = workspace_config.url_replacements {
        result = config::replacements::apply_replacements(&result, replacements);
    }
    if let Some(ref replacements) = test_suite.url_replacements {
        result = config::replacements::apply_replacements(&result, replacements);
    }
    result
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

/// Load persisted state (cookies + localStorage) into the page.
async fn load_state(page: &chromiumoxide::Page, state: &Needs, log_prefix: &str) -> Result<()> {
    let state_names = match state {
        Needs::String(s) => vec![s.clone()],
        Needs::StringArray(v) => v.clone(),
    };

    for name in &state_names {
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            anyhow::bail!(
                "Invalid state name '{}': must not contain path separators or '..'",
                name
            );
        }

        let root_dir = common::ROOT_DIR.get_required()?;
        let state_path = root_dir
            .join(".regressify")
            .join("states")
            .join(format!("{}.json", name));

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

        // Restore cookies
        if let Some(cookies) = state.get("cookies").and_then(|c| c.as_array()) {
            use chromiumoxide::cdp::browser_protocol::network::{CookieParam, CookieSameSite};
            for cookie in cookies {
                if let (Some(name), Some(value)) = (
                    cookie.get("name").and_then(|v| v.as_str()),
                    cookie.get("value").and_then(|v| v.as_str()),
                ) {
                    let mut param = CookieParam::builder().name(name).value(value);

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

        // Restore localStorage
        if let Some(local_storage) = state.get("localStorage").and_then(|v| v.as_object()) {
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

    Ok(())
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

    eprintln!("{} Loading cookies from: {}", log_prefix, cookie_path.display());
    let content = std::fs::read_to_string(&cookie_path)?;
    let cookies: Vec<serde_json::Value> = serde_yaml::from_str(&content)?;

    use chromiumoxide::cdp::browser_protocol::network::{CookieParam, CookieSameSite};
    for cookie in &cookies {
        if let (Some(name), Some(value)) = (
            cookie.get("name").and_then(|v| v.as_str()),
            cookie.get("value").and_then(|v| v.as_str()),
        ) {
            let mut param = CookieParam::builder().name(name).value(value);

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

    eprintln!("{} Applying CSS override: {}", log_prefix, css_path.display());
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

    eprintln!("{} Executing jsOnReady script: {}", log_prefix, js_path.display());
    let js_content = std::fs::read_to_string(&js_path)?;
    page.evaluate(js_content).await?;

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
