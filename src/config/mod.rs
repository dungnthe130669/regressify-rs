#![allow(dead_code)]

pub mod replacements;
pub mod viewports;

use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use crate::{
    common,
    types::{Browser, Needs, Selectors, TestScenario, TestSuite, UrlReplacement},
};

pub const DEFAULT_POST_INTERACTION_WAIT_MS: f64 = 1000.0;

/// Workspace-level configuration loaded from `regressify.yaml` or `regressify.yml`.
/// All fields are optional — missing fields inherit defaults.
#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub async_capture_limit: Option<usize>,
    pub async_compare_limit: Option<f64>,
    pub browser: Option<Browser>,
    pub bypass_csp: Option<bool>,
    pub cookie_path: Option<String>,
    pub css_override_path: Option<String>,
    pub debug: Option<bool>,
    pub delay: Option<f64>,
    pub hide_selectors: Option<Vec<String>>,
    pub ignore_ssl_errors: Option<bool>,
    pub js_on_ready_path: Option<String>,
    pub mis_match_threshold: Option<f64>,
    pub no_scroll_top: Option<bool>,
    pub post_interaction_wait: Option<f64>,
    pub remove_selectors: Option<Vec<String>>,
    pub required_login: Option<bool>,
    pub state: Option<Needs>,
    pub url_replacements: Option<Vec<UrlReplacement>>,
    pub use_css_override: Option<bool>,
    pub viewport_names: Option<Selectors>,
    pub viewports_path: Option<String>,
    /// Timeout in ms for waiting for an element to exist (default: 30000)
    pub element_timeout: Option<u64>,
    /// Timeout in ms for actionability checks (default: 5000)
    pub actionability_timeout: Option<u64>,
    /// Max time in ms to wait for the page to settle (fonts ready, layout/DOM
    /// quiet) right before capturing the screenshot. `0` disables the settle
    /// wait. Default: 5000.
    pub settle_timeout: Option<u64>,
}

/// Fully resolved configuration — all cascade levels merged with defaults applied.
#[derive(Clone)]
pub struct ResolvedConfig {
    pub async_capture_limit: usize,
    pub async_compare_limit: f64,
    pub browser: Browser,
    pub bypass_csp: bool,
    pub cookie_path: Option<String>,
    pub css_override_path: Option<String>,
    pub debug: bool,
    pub delay: Option<f64>,
    pub hide_selectors: Vec<String>,
    pub ignore_ssl_errors: bool,
    pub js_on_ready_path: Option<String>,
    pub mis_match_threshold: f64,
    pub no_scroll_top: bool,
    pub post_interaction_wait: Option<f64>,
    pub remove_selectors: Vec<String>,
    pub required_login: bool,
    pub state: Option<Needs>,
    pub use_css_override: bool,
    pub viewport_names: Option<Vec<String>>,
    pub viewports_path: String,
    pub element_timeout_ms: u64,
    pub actionability_timeout_ms: u64,
    pub settle_timeout_ms: u64,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            async_capture_limit: 5,
            async_compare_limit: 50.0,
            browser: Browser::Chromium,
            bypass_csp: false,
            cookie_path: None,
            css_override_path: None,
            debug: false,
            delay: None,
            hide_selectors: Vec::new(),
            ignore_ssl_errors: false,
            js_on_ready_path: None,
            mis_match_threshold: 0.1,
            no_scroll_top: false,
            post_interaction_wait: Some(DEFAULT_POST_INTERACTION_WAIT_MS),
            remove_selectors: Vec::new(),
            required_login: false,
            state: None,
            use_css_override: true,
            viewport_names: None,
            viewports_path: "common/_viewports.yaml".to_string(),
            element_timeout_ms: 30_000,
            actionability_timeout_ms: 5_000,
            settle_timeout_ms: 5_000,
        }
    }
}

/// Load workspace config from `regressify.yaml` or `regressify.yml` in the given directory.
/// Returns `WorkspaceConfig::default()` if neither file exists.
pub fn load_workspace_config(root_dir: &Path) -> Result<WorkspaceConfig> {
    let yaml_path = root_dir.join("regressify.yaml");
    let yml_path = root_dir.join("regressify.yml");

    let config_path = if yaml_path.exists() {
        Some(yaml_path)
    } else if yml_path.exists() {
        Some(yml_path)
    } else {
        None
    };

    match config_path {
        Some(path) => {
            let content = std::fs::read_to_string(&path)?;
            let config: WorkspaceConfig = serde_yaml::from_str(&content)?;
            Ok(config)
        }
        None => Ok(WorkspaceConfig::default()),
    }
}

fn selectors_to_vec(s: &Option<Selectors>) -> Option<Vec<String>> {
    match s {
        Some(Selectors::String(s)) => Some(vec![s.clone()]),
        Some(Selectors::StringArray(v)) => Some(v.clone()),
        None => None,
    }
}

fn needs_to_vec(needs: &Needs) -> Vec<String> {
    match needs {
        Needs::String(value) => vec![value.clone()],
        Needs::StringArray(values) => values.clone(),
    }
}

fn needs_from_vec(values: Vec<String>) -> Option<Needs> {
    match values.as_slice() {
        [] => None,
        [single] => Some(Needs::String(single.clone())),
        _ => Some(Needs::StringArray(values)),
    }
}

fn merge_state_and_restore(state: Option<Needs>, restore: Option<Needs>) -> Option<Needs> {
    let mut values = Vec::new();
    for needs in state.iter().chain(restore.iter()) {
        for value in needs_to_vec(needs) {
            if !values.contains(&value) {
                values.push(value);
            }
        }
    }
    needs_from_vec(values)
}

/// Resolve configuration by cascading: scenario -> suite -> workspace -> defaults.
/// For most fields: first non-None value wins (scenario overrides suite overrides workspace).
/// For selectors (hide/remove): scenario replaces suite (no merge).
pub fn resolve_config(
    workspace: &WorkspaceConfig,
    suite: &TestSuite,
    scenario: &TestScenario,
) -> ResolvedConfig {
    let defaults = ResolvedConfig::default();

    ResolvedConfig {
        async_capture_limit: suite
            .async_capture_limit
            .or(workspace.async_capture_limit)
            .unwrap_or(defaults.async_capture_limit),

        async_compare_limit: suite
            .async_compare_limit
            .or(workspace.async_compare_limit)
            .unwrap_or(defaults.async_compare_limit),

        browser: suite
            .browser
            .clone()
            .or(workspace.browser.clone())
            .unwrap_or(defaults.browser),

        bypass_csp: scenario
            .bypass_csp
            .or(suite.bypass_csp)
            .or(workspace.bypass_csp)
            .unwrap_or(defaults.bypass_csp),

        cookie_path: scenario
            .cookie_path
            .clone()
            .or(suite.cookie_path.clone())
            .or(workspace.cookie_path.clone()),

        css_override_path: scenario
            .css_override_path
            .clone()
            .or(suite.css_override_path.clone())
            .or(workspace.css_override_path.clone()),

        debug: suite.debug.or(workspace.debug).unwrap_or(defaults.debug),

        delay: scenario.delay.or(workspace.delay),

        hide_selectors: scenario
            .hide_selectors
            .clone()
            .or(suite.hide_selectors.clone())
            .or(workspace.hide_selectors.clone())
            .unwrap_or(defaults.hide_selectors),

        ignore_ssl_errors: suite
            .ignore_ssl_errors
            .or(workspace.ignore_ssl_errors)
            .unwrap_or(defaults.ignore_ssl_errors),

        js_on_ready_path: scenario
            .js_on_ready_path
            .clone()
            .or(suite.js_on_ready_path.clone())
            .or(workspace.js_on_ready_path.clone()),

        mis_match_threshold: scenario
            .mis_match_threshold
            .or(suite.mis_match_threshold)
            .or(workspace.mis_match_threshold)
            .unwrap_or(defaults.mis_match_threshold),

        no_scroll_top: scenario
            .no_scroll_top
            .or(workspace.no_scroll_top)
            .unwrap_or(defaults.no_scroll_top),

        post_interaction_wait: scenario
            .post_interaction_wait
            .or(suite.post_interaction_wait)
            .or(workspace.post_interaction_wait)
            .or(defaults.post_interaction_wait),

        remove_selectors: scenario
            .remove_selectors
            .clone()
            .or(suite.remove_selectors.clone())
            .or(workspace.remove_selectors.clone())
            .unwrap_or(defaults.remove_selectors),

        required_login: common::REQUIRED_LOGIN.get().copied().unwrap_or(false)
            || scenario
                .required_login
                .or(workspace.required_login)
                .unwrap_or(defaults.required_login),

        state: merge_state_and_restore(
            suite.state.clone().or(workspace.state.clone()),
            scenario.restore.clone(),
        ),

        use_css_override: scenario
            .use_css_override
            .or(suite.use_css_override)
            .or(workspace.use_css_override)
            .unwrap_or(defaults.use_css_override),

        viewport_names: selectors_to_vec(&scenario.viewport_names)
            .or(selectors_to_vec(&suite.viewport_names))
            .or(selectors_to_vec(&workspace.viewport_names)),

        viewports_path: suite
            .viewports_path
            .clone()
            .or(workspace.viewports_path.clone())
            .unwrap_or(defaults.viewports_path),

        element_timeout_ms: workspace
            .element_timeout
            .unwrap_or(defaults.element_timeout_ms),

        actionability_timeout_ms: workspace
            .actionability_timeout
            .unwrap_or(defaults.actionability_timeout_ms),

        settle_timeout_ms: workspace
            .settle_timeout
            .unwrap_or(defaults.settle_timeout_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suite() -> TestSuite {
        TestSuite {
            id: Some("alloy".to_string()),
            test_date: None,
            async_capture_limit: None,
            async_compare_limit: None,
            browser: None,
            bypass_csp: None,
            cookie_path: None,
            css_override_path: None,
            debug: None,
            hide_selectors: None,
            ignore_ssl_errors: None,
            js_on_ready_path: None,
            mis_match_threshold: None,
            post_interaction_wait: None,
            remove_selectors: None,
            scenarios: Vec::new(),
            state: None,
            url_replacements: None,
            use_css_override: None,
            viewport_names: None,
            viewports_path: None,
        }
    }

    fn scenario() -> TestScenario {
        TestScenario {
            actions: None,
            bypass_csp: None,
            click_selector: None,
            cookie_path: None,
            css_override_path: None,
            delay: None,
            description: None,
            hide_selectors: None,
            id: Some("home".to_string()),
            js_on_ready_path: None,
            label: None,
            mis_match_threshold: None,
            needs: None,
            no_scroll_top: None,
            post_interaction_wait: None,
            remove_selectors: None,
            required_login: None,
            restore: None,
            selectors: None,
            url: "https://example.com".to_string(),
            use_css_override: None,
            viewport_names: None,
        }
    }

    #[test]
    fn post_interaction_wait_defaults_to_1000_ms() {
        let resolved = resolve_config(&WorkspaceConfig::default(), &suite(), &scenario());

        assert_eq!(resolved.post_interaction_wait, Some(1000.0));
    }

    #[test]
    fn post_interaction_wait_uses_scenario_suite_workspace_order() {
        let workspace = WorkspaceConfig {
            post_interaction_wait: Some(3000.0),
            ..Default::default()
        };
        let mut suite = suite();
        suite.post_interaction_wait = Some(2000.0);
        let mut scenario = scenario();
        scenario.post_interaction_wait = Some(500.0);

        assert_eq!(
            resolve_config(&workspace, &suite, &scenario).post_interaction_wait,
            Some(500.0)
        );

        scenario.post_interaction_wait = None;
        assert_eq!(
            resolve_config(&workspace, &suite, &scenario).post_interaction_wait,
            Some(2000.0)
        );

        suite.post_interaction_wait = None;
        assert_eq!(
            resolve_config(&workspace, &suite, &scenario).post_interaction_wait,
            Some(3000.0)
        );
    }

    #[test]
    fn restore_extends_state_without_duplicates() {
        let mut suite = suite();
        suite.state = Some(Needs::StringArray(vec![
            "auth".to_string(),
            "prefs".to_string(),
        ]));
        let mut scenario = scenario();
        scenario.restore = Some(Needs::StringArray(vec![
            "auth".to_string(),
            "cart".to_string(),
        ]));

        let state = resolve_config(&WorkspaceConfig::default(), &suite, &scenario).state;
        match state {
            Some(Needs::StringArray(values)) => {
                assert_eq!(values, vec!["auth", "prefs", "cart"]);
            }
            _ => panic!("expected merged state array"),
        }
    }
}
