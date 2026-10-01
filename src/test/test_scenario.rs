use std::{future::Future, path::PathBuf, sync::Arc};

use anyhow::{Result, anyhow};
use chromiumoxide::Browser;
use colored::Colorize;

use crate::{
    common,
    config::{self, ResolvedConfig, WorkspaceConfig},
    helpers::OnceLockExtensions,
    test::{
        capture::{
            final_screenshot_order_key, final_screenshot_types, load_persisted_state,
            save_screenshot, saved_screenshot_types, validate_cookie_value,
            validate_screenshot_types, write_error_screenshots,
        },
        page_stability::{prepare_screenshot, stabilize_capture_animations, trigger_lazy_loading},
        test_action::{ActionExecutionContext, exec_test_action},
    },
    types::{Needs, ScreenshotType, TestAction, TestScenario, TestSuite, UrlReplacement},
};

fn legacy_actions(scenario: &TestScenario) -> Vec<TestAction> {
    use crate::types::{ClickAction, HoverAction, InputAction, ScrollAction};
    if scenario.actions.is_some() {
        return Vec::new();
    }
    let mut actions = Vec::new();
    let typing = scenario
        .key_press_selectors
        .as_deref()
        .or_else(|| {
            scenario
                .key_press_selector
                .as_ref()
                .map(std::slice::from_ref)
        })
        .unwrap_or_default();
    for entry in typing {
        actions.push(TestAction::Input(InputAction {
            input: entry.selector.clone(),
            value: Some(entry.key_press.clone()),
            append: Some(true),
            file: None,
            use_file_chooser: None,
            frame: None,
        }));
    }
    let selectors = |multiple: &Option<crate::types::Selectors>, single: &Option<String>| {
        multiple
            .as_ref()
            .map(|value| value.values().to_vec())
            .unwrap_or_else(|| single.iter().cloned().collect())
    };
    for hover in selectors(&scenario.hover_selectors, &scenario.hover_selector) {
        actions.push(TestAction::Hover(HoverAction { hover, frame: None }));
    }
    for click in selectors(&scenario.click_selectors, &scenario.click_selector) {
        actions.push(TestAction::Click(ClickAction { click, frame: None }));
    }
    if let Some(scroll) = &scenario.scroll_to_selector {
        actions.push(TestAction::Scroll(ScrollAction {
            scroll: scroll.clone(),
            frame: None,
        }));
    }
    actions
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
    is_reference: bool,
    test_suite: &'a TestSuite,
    scenario: &'a TestScenario,
    all_scenarios: &'a [TestScenario],
    resolved: &'a ResolvedConfig,
    url: &'a str,
    url_replacements: &'a [UrlReplacement],
    viewport: Option<&'a config::viewports::ViewportDef>,
    log_prefix: &'a str,
}

struct ScenarioPage {
    page: chromiumoxide::Page,
    runtime: super::runtime::Runtime,
    storage: Arc<super::storage_state::StorageState>,
    tabs: Option<super::tabs::Tabs>,
}

impl ScenarioPage {
    async fn exec(
        &mut self,
        browser: &Browser,
        ctx: CaptureContext<'_>,
        hooks: &super::hooks::DocumentHooks,
        action: &TestAction,
    ) -> Result<()> {
        // A popup can close itself. Handle switching before touching the old
        // document so returning to a surviving tab does not depend on that page.
        if let TestAction::SwitchTab(selection) = action {
            use crate::diagnostics::{record::Stage, runtime as diagnostics};
            let fields = super::test_action::action_diagnostic_fields(
                action,
                diagnostics::next_action_index(),
            );
            let tabs = self
                .tabs
                .as_mut()
                .ok_or_else(|| anyhow!("Tab tracking was not initialized"))?;
            let (page, runtime, first_visit) = diagnostics::action_scope(
                fields.clone(),
                diagnostics::stage(
                    Stage::ActionsFrames,
                    fields,
                    tabs.switch(browser, selection, ctx.url_replacements),
                ),
            )
            .await?;
            self.page = page;
            self.runtime = runtime;
            hooks.apply(&self.page).await?;
            if first_visit {
                prepare_page_for_actions(&self.page, ctx.resolved, ctx.log_prefix).await?;
            }
            return Ok(());
        }
        if let Some(tabs) = &self.tabs {
            tabs.check().await?;
        }
        hooks.apply(&self.page).await?;
        exec_test_action(
            ActionExecutionContext {
                page: &self.page,
                runtime: &self.runtime,
                storage: &self.storage,
                test_suite: ctx.test_suite,
                scenario: ctx.scenario,
                config: ctx.resolved,
                url_replacements: ctx.url_replacements,
                viewport_label: ctx.viewport.map(|viewport| viewport.label.as_str()),
                log_prefix: ctx.log_prefix,
            },
            action,
        )
        .await
    }
}

async fn prepare_page_for_actions(
    page: &chromiumoxide::Page,
    resolved: &ResolvedConfig,
    log_prefix: &str,
) -> Result<()> {
    if resolved.freeze_animations {
        stabilize_capture_animations(page, log_prefix).await?;
    }
    if !resolved.no_scroll_top {
        trigger_lazy_loading(page).await?;
    }
    Ok(())
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
    let capture_context = CaptureContext {
        is_reference: !should_apply_url_replacements,
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
            use crate::diagnostics::{record::*, runtime as diagnostics};
            if let Some(context) = diagnostics::current() {
                context.record(
                    Stage::CaptureArtifacts,
                    DiagnosticEvent::StageFinished,
                    Severity::Error,
                    [],
                    diagnostics::Completion {
                        outcome: Some(Outcome::Failure),
                        error: Some(SanitizedError::capture(&e)),
                        duration_ms: None,
                    },
                );
            }
            write_error_screenshots(
                ctx.test_suite,
                ctx.scenario,
                &error_screenshot_types,
                viewport_label,
                &SanitizedError::capture(&e).summary(),
            )?;
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
async fn run_scenario_capture_inner(
    browser: &Browser,
    ctx: CaptureContext<'_>,
    screenshot_types: &[ScreenshotType],
) -> Result<()> {
    let CaptureContext {
        is_reference,
        test_suite,
        scenario,
        all_scenarios,
        resolved,
        url,
        viewport,
        log_prefix,
        ..
    } = ctx;
    let viewport_label = viewport.map(|v| v.label.as_str());

    let hooks =
        super::hooks::DocumentHooks::load(common::ROOT_DIR.get_required()?, scenario, resolved)?;

    // Create page with about:blank first, apply the viewport + CSP bypass, then
    // navigate so layout is computed at the intended size from the first paint.
    let page = browser.new_page("about:blank").await?;
    let runtime = super::runtime::Runtime::new(browser, &page).await?;
    let authentication =
        super::authentication::Authentication::install(browser, &page, &resolved.basic_auth)
            .await?;
    if resolved.bypass_csp {
        let cmd = chromiumoxide::cdp::browser_protocol::page::SetBypassCspParams::builder()
            .enabled(true)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build SetBypassCspParams: {}", e))?;
        page.execute(cmd).await?;
    }
    apply_viewport(&page, viewport).await?;
    let storage =
        super::storage_state::StorageState::install(browser, &page, resolved.state.as_ref(), url)
            .await?;

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

    let mut visited = Vec::new();
    let needs_actions = collect_needs_actions(scenario, all_scenarios, &mut visited);
    let storage = Arc::new(storage);
    let tabs = if needs_actions
        .iter()
        .copied()
        .chain(scenario.actions.iter().flatten())
        .any(|action| matches!(action, TestAction::SwitchTab(_)))
    {
        Some(super::tabs::Tabs::install(&page, storage.clone(), resolved, viewport).await?)
    } else {
        None
    };
    let mut active = ScenarioPage {
        page,
        runtime,
        storage,
        tabs,
    };

    let run_hook = |script: Option<&str>, page: &chromiumoxide::Page| {
        let script = script.map(str::to_string);
        let endpoint = browser.websocket_address().to_string();
        let target = page.target_id().as_ref().to_string();
        async move {
            if let Some(script) = script {
                let root = common::ROOT_DIR.get_required()?;
                let script = config::resolve_workspace_file(root, &script, "Node hook")?;
                let mut hook_scenario = serde_json::to_value(scenario)?;
                hook_scenario["requiredLogin"] = serde_json::json!(resolved.required_login);
                hook_scenario["url"] = serde_json::json!(url);
                hook_scenario["testSuite"] = serde_json::json!(test_suite.id);
                let payload = serde_json::json!({"script": script, "endpoint": endpoint, "target": target,
                    "scenario": hook_scenario, "viewport": viewport.map(|value| serde_json::json!({"label": value.label, "width": value.width, "height": value.height})),
                    "isReference": is_reference, "config": resolved});
                super::node_hook::run(
                    root,
                    payload,
                    std::time::Duration::from_millis(resolved.element_timeout_ms),
                )
                .await?;
            }
            Ok::<_, anyhow::Error>(())
        }
    };
    run_hook(scenario.on_before_script.as_deref(), &active.page).await?;

    let target = active
        .runtime
        .target(
            &None,
            std::time::Duration::from_millis(resolved.element_timeout_ms),
        )
        .await?;
    let options = scenario
        .engine_options
        .as_ref()
        .and_then(|value| value.goto_parameters.as_ref())
        .cloned()
        .unwrap_or_default();
    target.navigate(url, &options).await?;
    if let Some(selector) = &scenario.ready_selector {
        let timeout = scenario
            .ready_timeout
            .filter(|value| *value > 0)
            .unwrap_or(resolved.element_timeout_ms);
        let readiness = crate::test::actionability::ActionabilityConfig {
            element_timeout_ms: timeout,
            actionability_timeout_ms: timeout,
            ..Default::default()
        };
        crate::test::actionability::wait_for_selector(&target, selector, &None, &readiness).await?;
    }

    hooks.apply(&active.page).await?;
    prepare_page_for_actions(&active.page, resolved, log_prefix).await?;

    // Execute prerequisite actions from `needs` dependencies
    if !needs_actions.is_empty() {
        crate::progress::scenario_log!(
            "{} Executing {} prerequisite actions from dependencies...",
            log_prefix,
            needs_actions.len()
        );
        for action in &needs_actions {
            active.exec(browser, ctx, &hooks, action).await?;
        }
    }

    // Execute this scenario's own actions
    if let Some(actions) = &scenario.actions {
        crate::progress::scenario_log!("{} Executing {} actions...", log_prefix, actions.len());
        for action in actions {
            active.exec(browser, ctx, &hooks, action).await?;
        }
    }

    hooks.apply(&active.page).await?;

    for action in &legacy_actions(scenario) {
        active.exec(browser, ctx, &hooks, action).await?;
    }

    run_hook(scenario.on_ready_script.as_deref(), &active.page).await?;
    hooks.apply(&active.page).await?;

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

    prepare_screenshot(&active.page, resolved, log_prefix).await?;

    for screenshot_type in screenshot_types {
        hooks.apply(&active.page).await?;
        if let Some(tabs) = &active.tabs {
            tabs.check().await?;
        }
        if let Some(authentication) = &authentication {
            authentication.check()?;
        }
        save_screenshot(
            test_suite,
            scenario,
            &active.page,
            screenshot_type.clone(),
            viewport_label,
        )
        .await?;
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
    _log_prefix: &str,
) -> Result<()> {
    use crate::diagnostics::{record::*, runtime as diagnostics};
    diagnostics::stage(
        Stage::StateHooks,
        [(Field::ActionType, FieldValue::Kind(SafeValue::Restore))],
        load_state_cookies_inner(page, state),
    )
    .await
}

async fn load_state_cookies_inner(page: &chromiumoxide::Page, state: &Needs) -> Result<()> {
    let state_names = match state {
        Needs::String(s) => vec![s.clone()],
        Needs::StringArray(v) => v.clone(),
    };

    for name in &state_names {
        let (_, state) = load_persisted_state(name)?;

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

/// Load cookies from a YAML file.
async fn load_cookie_path(
    page: &chromiumoxide::Page,
    cookie_path: &str,
    _log_prefix: &str,
) -> Result<()> {
    use crate::diagnostics::{record::*, runtime as diagnostics};
    diagnostics::stage(
        Stage::StateHooks,
        [
            (Field::ActionType, FieldValue::Kind(SafeValue::Restore)),
            (
                Field::Artifact,
                FieldValue::Identity(SafeIdentity::fingerprint(cookie_path)),
            ),
        ],
        load_cookie_path_inner(page, cookie_path),
    )
    .await
}

async fn load_cookie_path_inner(page: &chromiumoxide::Page, cookie_path: &str) -> Result<()> {
    let Some(cookie_path) = resolve_work_dir_path(cookie_path)? else {
        return Ok(()); // No cookie file — that's fine
    };

    let content = std::fs::read_to_string(&cookie_path)?;
    let cookies: Vec<serde_json::Value> = serde_yaml::from_str(&content)?;
    let cookies = super::capture::expand_cookie_entries(
        &cookies,
        &format!("cookiePath '{}'", cookie_path.display()),
    )?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_shortcuts_preserve_order_and_yield_to_actions() {
        let mut scenario: TestScenario = serde_yaml::from_str("url: https://example.com\nkeyPressSelector: {selector: '#input', keyPress: hello}\nhoverSelectors: ['#one', '#two']\nclickSelectors: ['#first', '#second']\nscrollToSelector: '#last'\n").unwrap();
        let actions = legacy_actions(&scenario);
        assert_eq!(actions.len(), 6);
        assert!(matches!(actions[0], TestAction::Input(_)));
        assert!(matches!(actions[1], TestAction::Hover(_)));
        assert!(matches!(actions[3], TestAction::Click(_)));
        assert!(matches!(actions[5], TestAction::Scroll(_)));
        scenario.actions = Some(Vec::new());
        assert!(legacy_actions(&scenario).is_empty());
    }

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
    #[ignore = "requires managed Chromium; run regressify install"]
    async fn popup_actions_and_captures_follow_the_selected_tab() -> Result<()> {
        let root = match std::env::var_os("REGRESSIFY_POPUP_CAPTURE_FIXTURE") {
            Some(path) => PathBuf::from(path),
            None => {
                let workspace = tempfile::tempdir()?;
                let output = std::process::Command::new(std::env::current_exe()?)
                    .args([
                        "--exact",
                        "test::test_scenario::tests::popup_actions_and_captures_follow_the_selected_tab",
                        "--ignored",
                        "--nocapture",
                    ])
                    .env("REGRESSIFY_POPUP_CAPTURE_FIXTURE", workspace.path())
                    .output()?;
                anyhow::ensure!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return Ok(());
            }
        };
        common::ROOT_DIR.set(root.clone()).unwrap();
        let output_dir = root.join(".regressify");
        common::REGRESSIFY_DIR.set(output_dir.clone()).unwrap();
        std::fs::create_dir_all(root.join("common"))?;
        std::fs::write(
            root.join("common/_on-ready.js"),
            "window.hookCount = (window.hookCount || 0) + 1;",
        )?;
        let authenticated = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let authenticated_request = authenticated.clone();
        let destination = crate::test::browser_tests::Site::new(move |request| {
            let authorized = request.headers().iter().any(|header| {
                header.field.equiv("Authorization") && header.value.as_str() == "Basic dXNlcjpwYXNz"
            });
            let response = if authorized {
                authenticated_request.store(true, std::sync::atomic::Ordering::Relaxed);
                tiny_http::Response::from_string(
                    r#"<!doctype html>
                    <script>window.restored = localStorage.getItem('fixture'); window.firstWidth = innerWidth;</script>
                    <style>body{margin:0;min-height:100vh;background:rgb(0,255,0)}</style>
                    <button id="change" style="margin:40px" onclick="document.body.style.background='rgb(0,0,255)'; localStorage.setItem('popup', 'changed')">Change</button>
                    <button id="close" onclick="window.close()">Close</button>"#,
                )
            } else {
                tiny_http::Response::from_string("unauthorized")
                    .with_status_code(tiny_http::StatusCode(401))
                    .with_header(
                        tiny_http::Header::from_bytes(
                            "WWW-Authenticate",
                            "Basic realm=\"popup-fixture\"",
                        )
                        .unwrap(),
                    )
            };
            let _ =
                request.respond(response.with_header(
                    tiny_http::Header::from_bytes("Content-Type", "text/html").unwrap(),
                ));
        })?;
        let destination_origin = destination.origin.clone();
        let leaked_auth = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let unexpected_auth = leaked_auth.clone();
        let site = crate::test::browser_tests::Site::new(move |request| {
            if request
                .headers()
                .iter()
                .any(|header| header.field.equiv("Authorization"))
            {
                unexpected_auth.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let content = format!(
                r#"<!doctype html>
                <script>localStorage.setItem('original', 'retained');</script>
                <style>body{{margin:0;min-height:100vh;background:rgb(255,0,0)}}</style>
                <a id="open" style="margin:40px;display:block" href="{destination_origin}/popup" target="_blank" rel="noopener">Open popup</a>
                <button id="second" onclick="window.open('{destination_origin}/popup?second')">Second popup</button>"#
            );
            let _ =
                request.respond(tiny_http::Response::from_string(content).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "text/html").unwrap(),
                ));
        })?;
        let state_path = super::super::capture::state_path("popup-initial")?;
        std::fs::create_dir_all(state_path.parent().unwrap())?;
        std::fs::write(
            state_path,
            serde_json::to_vec(&serde_json::json!({
                "cookies": [], "origins": [{"origin": destination.origin,
                    "localStorage": [{"name": "fixture", "value": "restored-before-scripts"}]}]
            }))?,
        )?;
        let viewport = config::viewports::ViewportDef {
            label: "phone".into(),
            width: 360,
            height: 240,
        };
        let (mut browser, handler, directory) = crate::test::browser_tests::launch_browser(
            "popup-capture",
            super::super::browsers::LaunchOptions {
                viewport: Some((viewport.width, viewport.height)),
                ..Default::default()
            },
        )
        .await?;
        let result: Result<()> = async {
            let suite = TestSuite { id: Some("tabs".into()), test_date: Some("run".into()), ..Default::default() };
            let resolved = ResolvedConfig {
                no_scroll_top: true, delay: Some(0.0), post_interaction_wait: Some(0.0),
                settle_timeout_ms: 0, freeze_animations: false, element_timeout_ms: 5000,
                state: Some(Needs::String("popup-initial".into())),
                basic_auth: vec![crate::types::BasicAuth { origin: destination.origin.clone(), username: "user".into(), password: "pass".into() }],
                ..Default::default()
            };
            let setup = scenario("open-popup", "actions:\n  - click: '#open'\n  - switchTab: new\n  - wait: '#change'\n    url: '**/popup'\n  - save: popup\n");
            let mut current = scenario("popup-capture", "needs: open-popup\nselectors: [viewport]\nactions:\n  - click: '#change'\n  - save: changed\n  - persist: after-popup\n  - click: '#close'\n  - switchTab: previous\n  - wait: '#open'\n  - save: original\n  - click: '#second'\n  - switchTab: new\n    url: '**/popup?second'\n  - wait: '#change'\n");
            current.url = site.origin.clone();
            let all_scenarios = [setup, current.clone()];
            run_scenario_capture_inner(&browser, CaptureContext {
                is_reference: false, test_suite: &suite, scenario: &current,
                all_scenarios: &all_scenarios, resolved: &resolved, url: &current.url,
                url_replacements: &[], viewport: Some(&viewport), log_prefix: "popup-capture",
            }, &final_screenshot_types(&current)?).await?;
            for (kind, color) in [
                (ScreenshotType::Step("popup".into()), (0, 255, 0)),
                (ScreenshotType::Step("changed".into()), (0, 0, 255)),
                (ScreenshotType::Step("original".into()), (255, 0, 0)),
                (ScreenshotType::Viewport, (0, 255, 0)),
            ] {
                let key = final_screenshot_order_key("popup-capture", Some(&viewport.label), &kind);
                let path = output_dir.join("tabs/bitmaps_test/run").join(format!("{key}.png"));
                let image = lodepng::decode32(std::fs::read(path)?)?;
                let pixel = image.buffer.first().ok_or_else(|| anyhow!("empty tab screenshot"))?;
                anyhow::ensure!((pixel.r, pixel.g, pixel.b) == color, "{key} captured the wrong tab: {pixel:?}");
                anyhow::ensure!(image.width == viewport.width as usize, "{key} used the wrong viewport");
            }
            let mut pages_checked = 0;
            for page in browser.pages().await? {
                if page.url().await?.is_some_and(|url| url.starts_with(&site.origin) || url.starts_with(&destination.origin)) {
                    let hooks: u32 = page.evaluate("window.hookCount").await?.into_value()?;
                    anyhow::ensure!(hooks == 1, "document hooks did not run exactly once on each tab");
                    if page.url().await?.is_some_and(|url| url.starts_with(&destination.origin)) {
                        let restored: String = page.evaluate("window.restored").await?.into_value()?;
                        anyhow::ensure!(restored == "restored-before-scripts", "popup scripts ran before storage restoration");
                        let width: u32 = page.evaluate("window.firstWidth").await?.into_value()?;
                        anyhow::ensure!(width == viewport.width, "popup first-load viewport was incorrect");
                    }
                    pages_checked += 1;
                }
            }
            anyhow::ensure!(pages_checked == 2, "scenario did not retain the original and the surviving popup");
            anyhow::ensure!(authenticated.load(std::sync::atomic::Ordering::Relaxed), "popup authentication was not handled");
            anyhow::ensure!(!leaked_auth.load(std::sync::atomic::Ordering::Relaxed), "credentials reached an unconfigured origin");
            let (_, persisted) = super::super::capture::load_persisted_state("after-popup")?;
            let origins = super::super::storage_state::validate_origins(&persisted)?;
            anyhow::ensure!(origins[&site.origin]["original"] == "retained", "persist lost the original tab's origin");
            anyhow::ensure!(origins[&destination.origin]["popup"] == "changed", "persist lost popup storage");
            Ok(())
        }.await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }

    #[tokio::test]
    #[ignore = "requires managed Chromium; run regressify install"]
    async fn lazy_loading_precedes_actions_and_preserves_hover() -> Result<()> {
        let root = match std::env::var_os("REGRESSIFY_LAZY_LOADING_FIXTURE") {
            Some(path) => PathBuf::from(path),
            None => {
                let workspace = tempfile::tempdir()?;
                let output = std::process::Command::new(std::env::current_exe()?)
                    .args([
                        "--exact",
                        "test::test_scenario::tests::lazy_loading_precedes_actions_and_preserves_hover",
                        "--ignored",
                        "--nocapture",
                    ])
                    .env("REGRESSIFY_LAZY_LOADING_FIXTURE", workspace.path())
                    .output()?;
                anyhow::ensure!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return Ok(());
            }
        };
        common::ROOT_DIR.set(root).unwrap();
        let site = crate::test::browser_tests::Site::new(|request| {
            let _ = request.respond(
                tiny_http::Response::from_string(
                    r#"<!doctype html><style>
                    html { scroll-behavior: smooth; }
                    body { margin: 0; height: 12000px; }
                    #target { margin: 40px; width: 120px; height: 50px; }
                    #target:hover { background: green; }
                    </style><button id="target">Hover</button><script>
                    window.fixture = { scrolls: 0, scrollsAtHover: null, scrolledAfterHover: false };
                    window.initialAnimation = document.body.animate(
                        [{ opacity: 0.8 }, { opacity: 1 }], { duration: 60000, fill: 'forwards' }
                    );
                    window.addEventListener('scroll', () => {
                        window.fixture.scrolls++;
                        if (window.fixture.scrollsAtHover !== null)
                            window.fixture.scrolledAfterHover = true;
                    });
                    document.querySelector('#target').addEventListener('pointermove', () => {
                        if (window.fixture.scrollsAtHover === null) {
                            window.fixture.scrollsAtHover = window.fixture.scrolls;
                            window.fixture.animationStateAtHover = window.initialAnimation.playState;
                        }
                    });
                    </script>"#,
                )
                .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html").unwrap()),
            );
        })?;
        let (mut browser, handler, directory) =
            crate::test::browser_tests::launch_browser("lazy-loading", Default::default()).await?;
        let result: Result<()> = async {
            let suite = TestSuite::default();
            let mut prerequisite = scenario("prerequisite", "actions:\n  - hover: '#target'\n");
            prerequisite.url = site.origin.clone();
            for (no_scroll_top, freeze_animations) in
                [(false, false), (false, true), (true, false), (true, true)]
            {
                let resolved = ResolvedConfig {
                    no_scroll_top,
                    post_interaction_wait: Some(0.0),
                    delay: Some(0.0),
                    settle_timeout_ms: 0,
                    freeze_animations,
                    ..Default::default()
                };
                for (kind, yaml) in [
                    ("no-actions", ""),
                    ("empty-actions", "actions: []\n"),
                    ("actions", "actions:\n  - hover: '#target'\n"),
                    ("needs", "needs: prerequisite\n"),
                    ("legacy", "hoverSelector: '#target'\n"),
                ] {
                    let mut current = scenario(kind, yaml);
                    current.url = format!("{}/{kind}-{no_scroll_top}-{freeze_animations}", site.origin);
                    let all_scenarios = [prerequisite.clone(), current.clone()];
                    run_with_scenario_timeout(
                        std::time::Duration::from_secs(20),
                        kind,
                        run_scenario_capture_inner(
                            &browser,
                            CaptureContext {
                                is_reference: true,
                                test_suite: &suite,
                                scenario: &current,
                                all_scenarios: &all_scenarios,
                                resolved: &resolved,
                                url: &current.url,
                                url_replacements: &[],
                                viewport: None,
                                log_prefix: kind,
                            },
                            &[],
                        ),
                    )
                    .await?;
                    let mut checked = false;
                    for page in browser.pages().await? {
                        if page.url().await?.as_deref() != Some(current.url.as_str()) {
                            continue;
                        }
                        let state: serde_json::Value = page
                            .evaluate(
                                "({ ...window.fixture, hovered: document.querySelector('#target').matches(':hover'), atTop: window.scrollY === 0 })",
                            )
                            .await?
                            .into_value()?;
                        anyhow::ensure!(
                            state["scrolledAfterHover"] == false,
                            "{kind} hover was disturbed (noScrollTop={no_scroll_top}): {state}"
                        );
                        let scrolls = if matches!(kind, "no-actions" | "empty-actions") {
                            state["scrolls"].as_u64()
                        } else {
                            anyhow::ensure!(
                                state["hovered"] == true,
                                "{kind} did not preserve hover: {state}"
                            );
                            let expected_animation = if freeze_animations { "finished" } else { "running" };
                            anyhow::ensure!(
                                state["animationStateAtHover"] == expected_animation,
                                "{kind} initial animations were not prepared before actions: {state}"
                            );
                            state["scrollsAtHover"].as_u64()
                        }
                        .ok_or_else(|| anyhow!("{kind} scroll state was not recorded: {state}"))?;
                        anyhow::ensure!(
                            (scrolls == 0) == no_scroll_top && state["atTop"] == true,
                            "{kind} lazy-load ordering or noScrollTop was incorrect: {state}"
                        );
                        page.close().await?;
                        checked = true;
                    }
                    anyhow::ensure!(checked, "{kind} capture page was not found");
                }
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
