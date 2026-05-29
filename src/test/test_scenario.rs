use anyhow::Result;
use chromiumoxide::Browser;
use colored::Colorize;

use crate::{
    config::{self, WorkspaceConfig},
    test::{capture::save_screenshot, test_action::exec_test_action},
    types::{Needs, ScreenshotType, TestAction, TestScenario, TestSuite},
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

pub async fn exec_test_scenario(
    browser: &Browser,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    all_scenarios: &[TestScenario],
    workspace_config: &WorkspaceConfig,
) -> Result<()> {
    let label = scenario
        .label
        .as_deref()
        .or(scenario.id.as_deref())
        .unwrap_or(&scenario.url);

    eprintln!("  Executing scenario: {} ({})", label.cyan(), scenario.url);

    // Resolve configuration cascade: scenario -> suite -> workspace -> defaults
    let resolved = config::resolve_config(workspace_config, test_suite, scenario);

    // Apply urlReplacements to the scenario URL
    let url = apply_url_replacements(&scenario.url, test_suite);

    // Create page with about:blank first, apply CSP bypass, then navigate
    let page = if resolved.bypass_csp {
        let page = browser.new_page("about:blank").await?;
        let cmd = chromiumoxide::cdp::browser_protocol::page::SetBypassCspParams::builder()
            .enabled(true)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build SetBypassCspParams: {}", e))?;
        page.execute(cmd).await?;
        page.goto(url.as_str()).await?;
        page
    } else {
        browser.new_page(url.as_str()).await?
    };

    // Load state (cookies + localStorage) if configured at suite level
    if let Some(ref state) = test_suite.state {
        load_state(&page, state).await?;
    }

    // Load cookies from cookiePath if configured
    if let Some(ref cookie_path) = scenario.cookie_path {
        load_cookie_path(&page, cookie_path).await?;
    }

    // Apply CSS override if configured
    apply_css_override(&page, test_suite, scenario).await?;

    // Apply suite-level hideSelectors
    if let Some(ref selectors) = test_suite.hide_selectors {
        for sel in selectors {
            inject_css(&page, sel, "visibility: hidden !important").await?;
        }
    }

    // Apply suite-level removeSelectors
    if let Some(ref selectors) = test_suite.remove_selectors {
        for sel in selectors {
            inject_css(&page, sel, "display: none !important").await?;
        }
    }

    // Apply scenario-level hideSelectors (overrides suite if set)
    if let Some(ref selectors) = scenario.hide_selectors {
        for sel in selectors {
            inject_css(&page, sel, "visibility: hidden !important").await?;
        }
    }

    // Apply scenario-level removeSelectors (overrides suite if set)
    if let Some(ref selectors) = scenario.remove_selectors {
        for sel in selectors {
            inject_css(&page, sel, "display: none !important").await?;
        }
    }

    // Execute jsOnReadyPath script if configured
    if let Some(ref js_path) = resolved.js_on_ready_path {
        let root_dir = crate::common::ROOT_DIR
            .get()
            .ok_or_else(|| anyhow::anyhow!("ROOT_DIR not initialized"))?;
        let visual_tests_dir = root_dir.join("visual_tests");
        let full_path = visual_tests_dir.join(js_path);
        let canonical = full_path
            .canonicalize()
            .map_err(|_| anyhow::anyhow!("jsOnReadyPath '{}' does not exist", js_path))?;
        let canonical_base = visual_tests_dir.canonicalize().unwrap_or(visual_tests_dir);
        if !canonical.starts_with(&canonical_base) {
            anyhow::bail!("jsOnReadyPath '{}' escapes visual_tests directory", js_path);
        }
        let js_content = std::fs::read_to_string(&canonical)?;
        page.evaluate(js_content).await?;
        eprintln!("    Executed jsOnReady script: {}", js_path);
    }

    // Execute prerequisite actions from `needs` dependencies
    let mut visited = Vec::new();
    let needs_actions = collect_needs_actions(scenario, all_scenarios, &mut visited);
    if !needs_actions.is_empty() {
        eprintln!(
            "    Executing {} prerequisite actions from dependencies...",
            needs_actions.len()
        );
        for action in &needs_actions {
            exec_test_action(&page, test_suite, scenario, action, &resolved).await?;
        }
    }

    // Execute this scenario's own actions
    if let Some(actions) = &scenario.actions {
        eprintln!("    Executing {} actions...", actions.len());
        for action in actions {
            exec_test_action(&page, test_suite, scenario, action, &resolved).await?;
        }
    }

    // Execute clickSelector if configured (clicks the element after all actions)
    if let Some(ref click_sel) = scenario.click_selector {
        eprintln!("    Clicking selector: {}", click_sel);
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
    if let Some(wait_ms) = resolved.post_interaction_wait {
        if wait_ms > 0.0 {
            eprintln!("    Waiting for {} ms after interactions...", wait_ms);
            tokio::time::sleep(std::time::Duration::from_millis(wait_ms as u64)).await;
        }
    }

    // Apply delay before screenshot
    if let Some(delay) = resolved.delay {
        if delay > 0.0 {
            eprintln!("    Delay: waiting {} ms before screenshot...", delay);
            tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
        }
    }

    // Scroll page top-to-bottom then back to top to trigger lazy-loaded images
    if !resolved.no_scroll_top {
        eprintln!("    Scrolling page to load lazy images...");
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
        .await
        .ok();
    }

    eprintln!("    Capturing final screenshot...");
    save_screenshot(test_suite, scenario, &page, ScreenshotType::Final).await?;

    Ok(())
}

/// Apply urlReplacements from the test suite to a URL.
/// Supports both literal string replacement and regex replacement.
fn apply_url_replacements(url: &str, test_suite: &TestSuite) -> String {
    let mut result = url.to_string();
    if let Some(ref replacements) = test_suite.url_replacements {
        for r in replacements {
            if r.regex.unwrap_or(false) {
                // Regex replacement
                let flags = r.flags.as_deref().unwrap_or("");
                let case_insensitive = flags.contains('i');
                let pattern_str = if case_insensitive {
                    format!("(?i){}", r.ref_pattern)
                } else {
                    r.ref_pattern.clone()
                };
                if let Ok(re) = regex::Regex::new(&pattern_str) {
                    result = re.replace_all(&result, r.test.as_str()).to_string();
                } else {
                    eprintln!(
                        "    WARNING: Invalid regex pattern '{}' in urlReplacements",
                        r.ref_pattern
                    );
                }
            } else {
                // Literal string replacement
                result = result.replace(&r.ref_pattern, &r.test);
            }
        }
    }
    result
}

/// Load persisted state (cookies + localStorage) into the page.
async fn load_state(page: &chromiumoxide::Page, state: &Needs) -> Result<()> {
    let state_names = match state {
        Needs::String(s) => vec![s.clone()],
        Needs::StringArray(v) => v.clone(),
    };

    for name in &state_names {
        let state_path = std::path::Path::new(".regressify")
            .join("states")
            .join(format!("{}.json", name));

        if !state_path.exists() {
            eprintln!(
                "    {} State file not found: {}",
                "WARNING:".yellow(),
                state_path.display()
            );
            continue;
        }

        eprintln!("    Loading state: {}", name);
        let content = std::fs::read_to_string(&state_path)?;
        let state: serde_json::Value = serde_json::from_str(&content)?;

        // Restore cookies
        if let Some(cookies) = state.get("cookies").and_then(|c| c.as_array()) {
            use chromiumoxide::cdp::browser_protocol::network::CookieParam;
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

                    let cookie_param = param
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to build cookie param: {:?}", e))?;
                    page.set_cookie(cookie_param).await.ok();
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
                    page.evaluate(script).await.ok();
                }
            }
        }
    }

    Ok(())
}

/// Load cookies from a YAML file.
async fn load_cookie_path(page: &chromiumoxide::Page, cookie_path: &str) -> Result<()> {
    if !std::path::Path::new(cookie_path).exists() {
        eprintln!(
            "    {} Cookie file not found: {}",
            "WARNING:".yellow(),
            cookie_path
        );
        return Ok(());
    }

    eprintln!("    Loading cookies from: {}", cookie_path);
    let content = std::fs::read_to_string(cookie_path)?;
    let cookies: Vec<serde_json::Value> = serde_yaml::from_str(&content)?;

    use chromiumoxide::cdp::browser_protocol::network::CookieParam;
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

            let cookie_param = param
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build cookie param: {:?}", e))?;
            page.set_cookie(cookie_param).await.ok();
        }
    }

    Ok(())
}

/// Apply CSS override from file if configured.
async fn apply_css_override(
    page: &chromiumoxide::Page,
    test_suite: &TestSuite,
    scenario: &TestScenario,
) -> Result<()> {
    let use_override = scenario
        .use_css_override
        .or(test_suite.use_css_override)
        .unwrap_or(true);

    if !use_override {
        return Ok(());
    }

    let css_path = scenario
        .css_override_path
        .as_deref()
        .or(test_suite.css_override_path.as_deref())
        .unwrap_or("common/_override.css");

    if !std::path::Path::new(css_path).exists() {
        return Ok(()); // No override file — that's fine
    }

    eprintln!("    Applying CSS override: {}", css_path);
    let css_content = std::fs::read_to_string(css_path)?;
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
    page.evaluate(script).await.ok();

    Ok(())
}

/// Inject a CSS rule for a selector.
async fn inject_css(page: &chromiumoxide::Page, selector: &str, rule: &str) -> Result<()> {
    use crate::test::actionability::escape_css_rule;
    let safe_selector = escape_css_rule(selector);
    let script = format!(
        "(() => {{
            var style = document.createElement('style');
            style.type = 'text/css';
            style.innerHTML = '{} {{ {} }}';
            document.head.appendChild(style);
        }})()",
        safe_selector, rule
    );
    page.evaluate(script).await?;
    Ok(())
}
