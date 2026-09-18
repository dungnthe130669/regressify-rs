#![allow(dead_code)]

pub mod basic_auth;
pub mod replacements;
pub mod viewports;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    common,
    types::{
        BasicAuth, BasicAuthConfig, Browser, Needs, Selectors, TestScenario, TestSuite,
        UrlReplacement,
    },
};

pub const DEFAULT_DELAY_MS: f64 = 1000.0;
pub const DEFAULT_POST_INTERACTION_WAIT_MS: f64 = 1000.0;
pub const DEFAULT_ASYNC_COMPARE_LIMIT: usize = 2;
pub const DEFAULT_ASYNC_CAPTURE_LIMIT: usize = 5;

/// Workspace-level configuration loaded from `regressify.yaml` or `regressify.yml`.
/// All fields are optional — missing fields inherit defaults.
#[derive(Deserialize, Serialize, Clone, Default)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceConfig {
    pub basic_auth: Option<BasicAuthConfig>,
    pub async_capture_limit: Option<usize>,
    pub async_compare_limit: Option<usize>,
    pub browser: Option<Browser>,
    pub bypass_csp: Option<bool>,
    pub cookie_path: Option<String>,
    pub css_override_path: Option<String>,
    pub debug: Option<bool>,
    pub delay: Option<f64>,
    /// Freeze active CSS and Web Animations before capture. Default: true.
    pub freeze_animations: Option<bool>,
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
    /// Max time in ms for one complete scenario/viewport capture. Default:
    /// 600000 (ten minutes).
    pub scenario_timeout: Option<u64>,
}

/// Fully resolved configuration — all cascade levels merged with defaults applied.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedConfig {
    #[serde(skip_serializing)]
    pub basic_auth: Vec<BasicAuth>,
    pub async_capture_limit: usize,
    pub async_compare_limit: usize,
    pub browser: Browser,
    pub bypass_csp: bool,
    pub cookie_path: Option<String>,
    pub css_override_path: Option<String>,
    pub debug: bool,
    pub delay: Option<f64>,
    pub freeze_animations: bool,
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
    pub viewports_path_is_explicit: bool,
    pub element_timeout_ms: u64,
    pub actionability_timeout_ms: u64,
    pub settle_timeout_ms: u64,
    pub scenario_timeout_ms: u64,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            basic_auth: Vec::new(),
            async_capture_limit: DEFAULT_ASYNC_CAPTURE_LIMIT,
            async_compare_limit: DEFAULT_ASYNC_COMPARE_LIMIT,
            browser: Browser::Chromium,
            bypass_csp: false,
            cookie_path: None,
            css_override_path: None,
            debug: false,
            delay: Some(DEFAULT_DELAY_MS),
            freeze_animations: true,
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
            viewports_path_is_explicit: false,
            element_timeout_ms: 30_000,
            actionability_timeout_ms: 5_000,
            settle_timeout_ms: 5_000,
            scenario_timeout_ms: 600_000,
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

/// Resolve an explicitly configured file and ensure it remains inside the
/// workspace, including after symlink resolution.
pub fn resolve_workspace_file(
    root_dir: &Path,
    configured_path: &str,
    label: &str,
) -> Result<PathBuf> {
    let candidate = if Path::new(configured_path).is_absolute() {
        PathBuf::from(configured_path)
    } else {
        root_dir.join(configured_path)
    };
    if !candidate.exists() {
        anyhow::bail!("Configured {} '{}' does not exist", label, configured_path);
    }

    let canonical_root = root_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve workspace root '{}'", root_dir.display()))?;
    let canonical_path = candidate.canonicalize().with_context(|| {
        format!(
            "Failed to resolve configured {} '{}'",
            label, configured_path
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        anyhow::bail!(
            "Configured {} '{}' escapes workspace root '{}'",
            label,
            configured_path,
            canonical_root.display()
        );
    }
    if !canonical_path.is_file() {
        anyhow::bail!("Configured {} '{}' is not a file", label, configured_path);
    }
    Ok(canonical_path)
}

/// Resolve an optional default file. Missing defaults are allowed, but a file
/// that exists must satisfy the same containment and regular-file checks as an
/// explicitly configured path.
pub fn resolve_optional_workspace_file(
    root_dir: &Path,
    configured_path: &str,
    label: &str,
) -> Result<Option<PathBuf>> {
    let candidate = if Path::new(configured_path).is_absolute() {
        PathBuf::from(configured_path)
    } else {
        root_dir.join(configured_path)
    };
    if !candidate.exists() {
        return Ok(None);
    }
    resolve_workspace_file(root_dir, configured_path, label).map(Some)
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

/// Resolve the suite-wide CPU worker count used for image comparisons.
pub fn resolve_async_compare_limit(workspace: &WorkspaceConfig, suite: &TestSuite) -> usize {
    suite
        .async_compare_limit
        .or(workspace.async_compare_limit)
        .unwrap_or(DEFAULT_ASYNC_COMPARE_LIMIT)
        .max(1)
}

/// Resolve the per-suite capture cap. The CLI-level global cap is applied by
/// the scheduler separately, so each suite cannot consume all capture slots.
pub fn resolve_async_capture_limit(workspace: &WorkspaceConfig, suite: &TestSuite) -> usize {
    suite
        .async_capture_limit
        .or(workspace.async_capture_limit)
        .unwrap_or(DEFAULT_ASYNC_CAPTURE_LIMIT)
        .max(1)
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
    let configured_viewports_path = suite
        .viewports_path
        .clone()
        .or(workspace.viewports_path.clone());

    ResolvedConfig {
        basic_auth: [
            &workspace.basic_auth,
            &suite.basic_auth,
            &scenario.basic_auth,
        ]
        .into_iter()
        .flatten()
        .flat_map(|config| config.entries().iter().cloned())
        .collect(),

        async_capture_limit: resolve_async_capture_limit(workspace, suite),

        async_compare_limit: resolve_async_compare_limit(workspace, suite),

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

        delay: scenario.delay.or(workspace.delay).or(defaults.delay),

        freeze_animations: workspace
            .freeze_animations
            .unwrap_or(defaults.freeze_animations),

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

        viewports_path: configured_viewports_path
            .clone()
            .unwrap_or(defaults.viewports_path),

        viewports_path_is_explicit: configured_viewports_path.is_some(),

        element_timeout_ms: workspace
            .element_timeout
            .unwrap_or(defaults.element_timeout_ms),

        actionability_timeout_ms: workspace
            .actionability_timeout
            .unwrap_or(defaults.actionability_timeout_ms),

        settle_timeout_ms: workspace
            .settle_timeout
            .unwrap_or(defaults.settle_timeout_ms),

        scenario_timeout_ms: workspace
            .scenario_timeout
            .unwrap_or(defaults.scenario_timeout_ms),
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
            ..Default::default()
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
            ..Default::default()
        }
    }

    #[test]
    fn basic_auth_cascades_by_origin_and_is_not_serialized() {
        let workspace: WorkspaceConfig = serde_yaml::from_str(
            "basicAuth:\n  origin: https://example.com\n  username: workspace\n  password: private\n",
        ).unwrap();
        let suite: TestSuite = serde_yaml::from_str(
            "scenarios: []\nbasicAuth:\n  - origin: https://other.example\n    username: other\n    password: private\n",
        ).unwrap();
        let scenario: TestScenario = serde_yaml::from_str(
            "url: https://example.com\nbasicAuth:\n  origin: https://example.com\n  username: scenario\n  password: private\n",
        ).unwrap();
        let resolved = resolve_config(&workspace, &suite, &scenario);
        let entries = basic_auth::resolve(&resolved.basic_auth).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].username, "scenario");
        assert_eq!(entries[1].username, "other");
        assert!(
            !serde_json::to_string(&resolved)
                .unwrap()
                .contains("private")
        );
    }

    #[test]
    fn delay_defaults_to_1000_ms_and_preserves_explicit_zero() {
        let workspace = WorkspaceConfig::default();
        let suite = suite();
        let mut scenario = scenario();
        assert_eq!(
            resolve_config(&workspace, &suite, &scenario).delay,
            Some(1000.0)
        );
        let workspace = WorkspaceConfig {
            delay: Some(250.0),
            ..workspace
        };
        assert_eq!(
            resolve_config(&workspace, &suite, &scenario).delay,
            Some(250.0)
        );
        scenario.delay = Some(0.0);
        assert_eq!(
            resolve_config(&workspace, &suite, &scenario).delay,
            Some(0.0)
        );
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

    #[test]
    fn scenario_timeout_uses_workspace_value_or_default() {
        let default = resolve_config(&WorkspaceConfig::default(), &suite(), &scenario());
        assert_eq!(default.scenario_timeout_ms, 600_000);

        let workspace = WorkspaceConfig {
            scenario_timeout: Some(42_000),
            ..Default::default()
        };
        let resolved = resolve_config(&workspace, &suite(), &scenario());
        assert_eq!(resolved.scenario_timeout_ms, 42_000);
    }

    #[test]
    fn freeze_animations_defaults_on_and_can_be_disabled() {
        let default = resolve_config(&WorkspaceConfig::default(), &suite(), &scenario());
        assert!(default.freeze_animations);

        let workspace = WorkspaceConfig {
            freeze_animations: Some(false),
            ..Default::default()
        };
        let resolved = resolve_config(&workspace, &suite(), &scenario());
        assert!(!resolved.freeze_animations);
    }

    #[test]
    fn async_compare_limit_uses_suite_workspace_and_default_order() {
        let workspace = WorkspaceConfig {
            async_compare_limit: Some(3),
            ..Default::default()
        };
        let mut suite = suite();

        assert_eq!(resolve_async_compare_limit(&workspace, &suite), 3);
        suite.async_compare_limit = Some(4);
        assert_eq!(resolve_async_compare_limit(&workspace, &suite), 4);
        assert_eq!(
            resolve_async_compare_limit(
                &WorkspaceConfig::default(),
                &TestSuite {
                    async_compare_limit: None,
                    ..suite
                }
            ),
            DEFAULT_ASYNC_COMPARE_LIMIT
        );
    }

    #[test]
    fn async_capture_limit_uses_suite_workspace_and_default_order() {
        let workspace = WorkspaceConfig {
            async_capture_limit: Some(3),
            ..Default::default()
        };
        let mut suite = suite();

        assert_eq!(resolve_async_capture_limit(&workspace, &suite), 3);
        suite.async_capture_limit = Some(4);
        assert_eq!(resolve_async_capture_limit(&workspace, &suite), 4);
        assert_eq!(
            resolve_async_capture_limit(
                &WorkspaceConfig::default(),
                &TestSuite {
                    async_capture_limit: None,
                    ..suite
                }
            ),
            DEFAULT_ASYNC_CAPTURE_LIMIT
        );
    }

    #[test]
    fn workspace_config_rejects_unknown_properties() {
        let result = serde_yaml::from_str::<WorkspaceConfig>("postinteractionwait: 5000\n");

        assert!(matches!(result, Err(error) if error.to_string().contains("unknown field")));
    }

    #[test]
    fn workspace_file_resolution_requires_an_in_workspace_file() {
        let workspace = tempfile::tempdir().unwrap();
        let file = workspace.path().join("common/cookies.yaml");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "[]\n").unwrap();

        assert_eq!(
            resolve_workspace_file(workspace.path(), "common/cookies.yaml", "cookiePath").unwrap(),
            file.canonicalize().unwrap()
        );
        assert!(
            resolve_workspace_file(workspace.path(), "missing.yaml", "cookiePath")
                .unwrap_err()
                .to_string()
                .contains("does not exist")
        );
        assert!(
            resolve_workspace_file(workspace.path(), "common", "cookiePath")
                .unwrap_err()
                .to_string()
                .contains("is not a file")
        );
    }

    #[test]
    fn optional_workspace_file_resolution_allows_missing_defaults() {
        let workspace = tempfile::tempdir().unwrap();

        assert_eq!(
            resolve_optional_workspace_file(
                workspace.path(),
                "common/missing.css",
                "cssOverridePath",
            )
            .unwrap(),
            None
        );
    }
}
