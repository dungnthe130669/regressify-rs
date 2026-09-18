use std::{future::Future, path::PathBuf};

use anyhow::{Result, anyhow};
use chromiumoxide::{Browser, Page};
use colored::Colorize;

use crate::{
    common,
    config::{self, ResolvedConfig, WorkspaceConfig},
    helpers::OnceLockExtensions,
    test::{
        capture::{
            final_screenshot_order_key, final_screenshot_types, load_persisted_state,
            save_screenshot, saved_screenshot_types, screenshot_type_description,
            validate_cookie_value, validate_screenshot_types, write_error_screenshots,
        },
        page_stability::{
            freeze_animations, trigger_lazy_loading, wait_for_dom_ready, wait_for_page_settle,
        },
        test_action::{ActionExecutionContext, exec_test_action},
    },
    types::{Needs, ScreenshotType, TestAction, TestScenario, TestSuite, UrlReplacement},
};

async fn navigate_and_wait_for_dom(
    page: &chromiumoxide::Page,
    url: &str,
    element_timeout_ms: u64,
) -> Result<()> {
    let timeout = std::time::Duration::from_millis(element_timeout_ms);
    crate::test::actionability::cdp_with_timeout(
        timeout,
        "navigating scenario URL",
        page.goto(url),
    )
    .await?;
    wait_for_dom_ready(page, timeout).await
}

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
            crate::progress::scenario_log!(
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

/// Ordered capture keys for a scenario. Saved steps from prerequisite and
/// current actions precede final targets, matching their execution order.
pub(crate) fn scenario_capture_order_keys(
    workspace_config: &WorkspaceConfig,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    all_scenarios: &[TestScenario],
) -> Result<Vec<String>> {
    let Some(scenario_id) = scenario.id.as_deref() else {
        return Ok(Vec::new());
    };
    let resolved = config::resolve_config(workspace_config, test_suite, scenario);
    let viewports = resolve_scenario_viewports(&resolved)?;
    let screenshot_types = scenario_screenshot_types(scenario, all_scenarios)?;
    if viewports.is_empty() {
        Ok(screenshot_types
            .iter()
            .map(|screenshot_type| final_screenshot_order_key(scenario_id, None, screenshot_type))
            .collect())
    } else {
        Ok(viewports
            .iter()
            .flat_map(|viewport| {
                screenshot_types.iter().map(|screenshot_type| {
                    final_screenshot_order_key(scenario_id, Some(&viewport.label), screenshot_type)
                })
            })
            .collect())
    }
}

pub(crate) fn capture_progress_keys(
    scenario: &TestScenario,
    all_scenarios: &[TestScenario],
    viewport: Option<&str>,
) -> Result<Vec<String>> {
    let Some(scenario_id) = scenario.id.as_deref() else {
        return Ok(Vec::new());
    };
    Ok(scenario_screenshot_types(scenario, all_scenarios)?
        .iter()
        .map(|screenshot_type| final_screenshot_order_key(scenario_id, viewport, screenshot_type))
        .collect())
}

fn scenario_screenshot_types(
    scenario: &TestScenario,
    all_scenarios: &[TestScenario],
) -> Result<Vec<ScreenshotType>> {
    let mut visited = Vec::new();
    let prerequisite_actions = collect_needs_actions(scenario, all_scenarios, &mut visited);
    let mut screenshot_types = saved_screenshot_types(prerequisite_actions);
    screenshot_types.extend(saved_screenshot_types(
        scenario.actions.as_deref().unwrap_or_default().iter(),
    ));
    screenshot_types.extend(final_screenshot_types(scenario)?);
    validate_screenshot_types(&screenshot_types)?;
    Ok(screenshot_types)
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

    crate::progress::scenario_log!(
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
    let url = apply_url_replacements(&scenario.url, &url_replacements)?;

    // This request captures exactly one scenario/viewport pair. When a viewport
    // is present the full page lifecycle (navigation + actions) is replayed at
    // that size so layout and interactions reflect it; with no viewport we
    // capture once at the browser's default size. Each pair is an independent
    // task (own browser, own index), so a failure here only fails this pair.
    if let Some(vp) = viewport {
        crate::progress::scenario_log!(
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
    let Some(viewports_path) = resolve_viewports_file(
        root_dir,
        &resolved.viewports_path,
        resolved.viewports_path_is_explicit,
    )?
    else {
        return Ok(Vec::new());
    };
    let all = config::viewports::load_viewports(&viewports_path)?;
    if all.is_empty() {
        return Ok(Vec::new());
    }
    config::viewports::resolve_viewports(resolved.viewport_names.as_deref(), &all)
}

fn resolve_viewports_file(
    root_dir: &std::path::Path,
    configured_path: &str,
    is_explicit: bool,
) -> Result<Option<PathBuf>> {
    if is_explicit {
        config::resolve_workspace_file(root_dir, configured_path, "viewportsPath").map(Some)
    } else {
        config::resolve_optional_workspace_file(root_dir, configured_path, "default viewportsPath")
    }
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
    let final_screenshot_types = final_screenshot_types(ctx.scenario)?;
    let error_screenshot_types = scenario_screenshot_types(ctx.scenario, ctx.all_scenarios)?;

    match run_with_scenario_timeout(
        timeout,
        label,
        run_scenario_capture_inner(browser, ctx, &final_screenshot_types),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Err(write_err) = write_error_screenshots(
                ctx.test_suite,
                ctx.scenario,
                &error_screenshot_types,
                viewport_label,
                &format!("{e:?}"),
            ) {
                crate::progress::scenario_log!(
                    "{} {} Failed to write error placeholder screenshot: {:?}",
                    ctx.log_prefix,
                    "WARNING:".yellow(),
                    write_err
                );
                return Err(write_err);
            } else {
                crate::progress::scenario_log!(
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

async fn stabilize_capture_animations(page: &Page, log_prefix: &str) -> Result<()> {
    let (completed, frozen) = freeze_animations(page).await?;
    if completed > 0 || frozen > 0 {
        crate::progress::scenario_log!(
            "{} Stabilized {} finite and {} infinite animation(s).",
            log_prefix,
            completed,
            frozen
        );
    }
    Ok(())
}

/// Inner capture routine: drives the full page lifecycle and saves the
/// screenshot. Errors are handled by the `run_scenario_capture` wrapper.
async fn run_scenario_capture_inner(
    browser: &Browser,
    ctx: CaptureContext<'_>,
    screenshot_types: &[ScreenshotType],
) -> Result<()> {
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

    navigate_and_wait_for_dom(&page, url, resolved.element_timeout_ms).await?;

    // localStorage requires an origin, so it is restored after navigation. If
    // anything was restored, reload so app code observes it during startup.
    if let Some(ref state) = resolved.state
        && load_state_local_storage(&page, state, log_prefix).await?
    {
        crate::progress::scenario_log!(
            "{} Reloading page after restoring localStorage...",
            log_prefix
        );
        navigate_and_wait_for_dom(&page, url, resolved.element_timeout_ms).await?;
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
        crate::progress::scenario_log!(
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
        crate::progress::scenario_log!("{} Executing {} actions...", log_prefix, actions.len());
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
        crate::progress::scenario_log!("{} Clicking selector: {}", log_prefix, click_sel);
        use crate::test::actionability::{self as act, ActionabilityConfig};
        let ac = ActionabilityConfig {
            element_timeout_ms: resolved.element_timeout_ms,
            actionability_timeout_ms: resolved.actionability_timeout_ms,
            ..Default::default()
        };
        let info = act::ensure_actionable(&page, click_sel, &None, &ac).await?;
        let frame_offset = act::get_frame_offset(&page, &None, &ac).await?;
        act::cdp_click(
            &page,
            frame_offset.0 + info.center_x,
            frame_offset.1 + info.center_y,
            &ac,
        )
        .await?;
    }

    // Apply postInteractionWait from resolved config
    if let Some(wait_ms) = resolved.post_interaction_wait
        && wait_ms > 0.0
    {
        crate::progress::scenario_log!(
            "{} Waiting for {} ms after interactions...",
            log_prefix,
            wait_ms
        );
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms as u64)).await;
    }

    // Apply delay before screenshot
    if let Some(delay) = resolved.delay
        && delay > 0.0
    {
        crate::progress::scenario_log!(
            "{} Delay: waiting {} ms before screenshot...",
            log_prefix,
            delay
        );
        tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
    }

    if !resolved.no_scroll_top {
        crate::progress::scenario_log!("{} Scrolling page to load lazy images...", log_prefix);
        let scroll_start = std::time::Instant::now();
        trigger_lazy_loading(&page).await?;
        crate::progress::scenario_log!(
            "{} Lazy-load scroll completed in {} ms.",
            log_prefix,
            scroll_start.elapsed().as_millis()
        );
    }

    if resolved.freeze_animations {
        stabilize_capture_animations(&page, log_prefix).await?;
    }

    if resolved.settle_timeout_ms > 0 {
        match wait_for_page_settle(
            &page,
            std::time::Duration::from_millis(resolved.settle_timeout_ms),
        )
        .await
        {
            Ok(result) if result.settled => {
                crate::progress::scenario_log!(
                    "{} Page settled in {} ms before capture.",
                    log_prefix,
                    result.elapsed_ms
                );
            }
            Ok(result) => {
                crate::progress::scenario_log!(
                    "{} Page did not settle within {} ms; capturing anyway.",
                    log_prefix,
                    result.elapsed_ms
                );
            }
            Err(error) => {
                crate::progress::scenario_log!(
                    "{} Settle wait failed ({error}); capturing without settle.",
                    log_prefix
                );
            }
        }
    }

    if resolved.freeze_animations {
        // Resources that completed during settlement can start new animations.
        stabilize_capture_animations(&page, log_prefix).await?;
    }

    for screenshot_type in screenshot_types {
        crate::progress::scenario_log!(
            "{} Capturing {} screenshot...",
            log_prefix,
            screenshot_type_description(screenshot_type)
        );
        let capture_start = std::time::Instant::now();
        save_screenshot(
            test_suite,
            scenario,
            &page,
            screenshot_type.clone(),
            viewport_label,
        )
        .await?;
        crate::progress::scenario_log!(
            "{} {} screenshot captured in {} ms.",
            log_prefix,
            screenshot_type_description(screenshot_type),
            capture_start.elapsed().as_millis()
        );
    }

    Ok(())
}

/// Apply urlReplacements from the test suite to a URL.
/// Supports both literal string replacement and regex replacement.
pub(crate) fn collect_url_replacements(
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
fn apply_url_replacements(url: &str, replacements: &[UrlReplacement]) -> Result<String> {
    config::replacements::apply_replacements(url, replacements)
}

fn resolve_work_dir_path(path: &str) -> Result<Option<PathBuf>> {
    let root_dir = common::ROOT_DIR.get_required()?;
    config::resolve_optional_workspace_file(root_dir, path, "workspace file")
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
        let (_, state) = load_persisted_state(name)?;

        crate::progress::scenario_log!("{} Loading state: {}", log_prefix, name);

        // Restore cookies
        if let Some(cookies) = state.get("cookies").and_then(|c| c.as_array()) {
            for (index, cookie) in cookies.iter().enumerate() {
                let (name, cookie_param) = build_cookie_param(
                    cookie,
                    &format!("Persisted state '{}' cookie {}", name, index),
                )?;
                page.set_cookie(cookie_param).await.map_err(|error| {
                    anyhow!("Failed to restore state cookie '{}': {:?}", name, error)
                })?;
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
        let (_, state) = load_persisted_state(name)?;

        // Restore localStorage
        if let Some(local_storage) = state.get("localStorage").and_then(|v| v.as_object()) {
            if !local_storage.is_empty() {
                crate::progress::scenario_log!(
                    "{} Restoring localStorage from state: {}",
                    log_prefix,
                    name
                );
                restored = true;
            }
            for (key, value) in local_storage {
                let val_str = value.as_str().ok_or_else(|| {
                    anyhow!(
                        "Persisted state '{}' localStorage key '{}' must have a string value",
                        name,
                        key
                    )
                })?;
                let script = format!(
                    "localStorage.setItem({}, {})",
                    serde_json::to_string(key)?,
                    serde_json::to_string(val_str)?
                );
                page.evaluate(script).await.map_err(|e| {
                    anyhow!("Failed to restore localStorage key '{}': {:?}", key, e)
                })?;
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

    crate::progress::scenario_log!(
        "{} Loading cookies from: {}",
        log_prefix,
        cookie_path.display()
    );
    let content = std::fs::read_to_string(&cookie_path)?;
    let cookies: Vec<serde_json::Value> = serde_yaml::from_str(&content)?;

    for (index, cookie) in cookies.iter().enumerate() {
        let (name, cookie_param) = build_cookie_param(
            cookie,
            &format!("cookiePath '{}' cookie {}", cookie_path.display(), index),
        )?;
        page.set_cookie(cookie_param)
            .await
            .map_err(|error| anyhow!("Failed to load cookie '{}': {:?}", name, error))?;
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

fn build_cookie_param(
    cookie: &serde_json::Value,
    context: &str,
) -> Result<(
    String,
    chromiumoxide::cdp::browser_protocol::network::CookieParam,
)> {
    use chromiumoxide::cdp::browser_protocol::network::{CookieParam, TimeSinceEpoch};

    validate_cookie_value(cookie, context)?;
    let name = cookie
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("{} has no name after validation", context))?;
    let value = cookie
        .get("value")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("{} has no value after validation", context))?;
    let mut param = CookieParam::builder().name(name).value(value);

    if let Some(url) = cookie_url_from(cookie) {
        param = param.url(url);
    }
    if let Some(domain) = cookie.get("domain").and_then(|value| value.as_str()) {
        param = param.domain(domain);
    }
    if let Some(path) = cookie.get("path").and_then(|value| value.as_str()) {
        param = param.path(path);
    }
    if let Some(secure) = cookie.get("secure").and_then(|value| value.as_bool()) {
        param = param.secure(secure);
    }
    if let Some(http_only) = cookie.get("httpOnly").and_then(|value| value.as_bool()) {
        param = param.http_only(http_only);
    }
    if let Some(expires) = cookie.get("expires").and_then(|value| value.as_f64()) {
        param = param.expires(TimeSinceEpoch::new(expires));
    }
    if let Some(same_site) = cookie.get("sameSite").and_then(|value| value.as_str()) {
        param = param.same_site(parse_same_site(same_site)?);
    }

    let param = param
        .build()
        .map_err(|error| anyhow!("Failed to build {}: {:?}", context, error))?;
    Ok((name.to_string(), param))
}

/// Parse a SameSite string from cookie JSON into the CDP enum.
fn parse_same_site(
    s: &str,
) -> Result<chromiumoxide::cdp::browser_protocol::network::CookieSameSite> {
    use chromiumoxide::cdp::browser_protocol::network::CookieSameSite;
    match s {
        "Strict" | "strict" => Ok(CookieSameSite::Strict),
        "Lax" | "lax" => Ok(CookieSameSite::Lax),
        "None" | "none" => Ok(CookieSameSite::None),
        _ => anyhow::bail!(
            "Unsupported SameSite value '{}'; expected Strict, Lax, or None",
            s
        ),
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

    crate::progress::scenario_log!(
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

    crate::progress::scenario_log!(
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

    #[test]
    fn explicit_viewports_files_must_exist_inside_the_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let viewport_path = workspace.path().join("common/viewports.yaml");
        std::fs::create_dir_all(viewport_path.parent().unwrap()).unwrap();
        std::fs::write(
            &viewport_path,
            "- label: desktop\n  width: 1280\n  height: 720\n",
        )
        .unwrap();

        assert_eq!(
            resolve_viewports_file(workspace.path(), "common/viewports.yaml", true).unwrap(),
            Some(viewport_path.canonicalize().unwrap())
        );
        assert!(
            resolve_viewports_file(workspace.path(), "common/missing.yaml", false)
                .unwrap()
                .is_none()
        );
        assert!(
            resolve_viewports_file(workspace.path(), outside.path().to_str().unwrap(), true,)
                .unwrap_err()
                .to_string()
                .contains("escapes workspace root")
        );
        assert!(
            resolve_viewports_file(workspace.path(), "common/missing.yaml", true)
                .unwrap_err()
                .to_string()
                .contains("does not exist")
        );
    }

    fn scenario(id: &str, yaml: &str) -> TestScenario {
        serde_yaml::from_str(&format!("id: {id}\nurl: https://example.com/{id}\n{yaml}")).unwrap()
    }

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

    #[test]
    fn capture_order_includes_prerequisite_and_current_save_steps() {
        let setup = scenario("setup", "actions:\n  - save: prepared\n");
        let page = scenario(
            "page",
            "needs: setup\nactions:\n  - save: ready\nselectors:\n  - viewport\n",
        );
        let all = vec![setup, page];

        let types = scenario_screenshot_types(&all[1], &all).unwrap();

        assert!(matches!(&types[0], ScreenshotType::Step(name) if name == "prepared"));
        assert!(matches!(&types[1], ScreenshotType::Step(name) if name == "ready"));
        assert!(matches!(types[2], ScreenshotType::Viewport));
    }

    #[test]
    fn capture_order_rejects_duplicate_inherited_save_steps() {
        let setup = scenario("setup", "actions:\n  - save: current\n");
        let page = scenario("page", "needs: setup\nactions:\n  - save: current\n");
        let all = vec![setup, page];

        let error = scenario_screenshot_types(&all[1], &all).unwrap_err();

        assert!(error.to_string().contains("duplicate filename"));
    }
}
