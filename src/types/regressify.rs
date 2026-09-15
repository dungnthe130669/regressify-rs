#![allow(dead_code)]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::actions::TestAction;

pub struct TestSuiteInput {
    pub name: String,

    pub path: PathBuf,

    pub format: TestSuiteFormat,
}

pub enum TestSuiteFormat {
    Yaml,

    Json,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct TestSuite {
    pub id: Option<String>,

    pub test_date: Option<String>,

    pub async_capture_limit: Option<usize>,

    pub async_compare_limit: Option<usize>,

    pub browser: Option<Browser>,

    pub bypass_csp: Option<bool>,

    pub cookie_path: Option<String>,

    pub css_override_path: Option<String>,

    pub debug: Option<bool>,

    pub hide_selectors: Option<Vec<String>>,

    pub ignore_ssl_errors: Option<bool>,

    pub js_on_ready_path: Option<String>,

    pub mis_match_threshold: Option<f64>,

    pub post_interaction_wait: Option<f64>,

    pub remove_selectors: Option<Vec<String>>,

    pub scenarios: Vec<TestScenario>,

    pub state: Option<Needs>,

    pub url_replacements: Option<Vec<UrlReplacement>>,

    pub use_css_override: Option<bool>,

    pub viewport_names: Option<Selectors>,

    pub viewports_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub enum Browser {
    Chromium,

    Firefox,

    Webkit,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct TestScenario {
    /// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/
    pub actions: Option<Vec<TestAction>>,

    pub bypass_csp: Option<bool>,

    pub click_selector: Option<String>,

    pub cookie_path: Option<String>,

    pub css_override_path: Option<String>,

    pub delay: Option<f64>,

    pub description: Option<String>,

    pub hide_selectors: Option<Vec<String>>,

    pub id: Option<String>,

    pub js_on_ready_path: Option<String>,

    pub label: Option<String>,

    pub mis_match_threshold: Option<f64>,

    pub needs: Option<Needs>,

    pub no_scroll_top: Option<bool>,

    pub post_interaction_wait: Option<f64>,

    pub remove_selectors: Option<Vec<String>>,

    pub required_login: Option<bool>,

    pub restore: Option<Needs>,

    /// CSS selectors to capture instead of the full page. Use "viewport" for
    /// the visible viewport; each selector produces an independent screenshot.
    pub selectors: Option<Selectors>,

    pub url: String,

    pub use_css_override: Option<bool>,

    pub viewport_names: Option<Selectors>,
}

/// One or more CSS selectors to capture. Use "viewport" for the visible
/// viewport; omit the property for the full page.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub enum Selectors {
    String(String),

    StringArray(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub enum Needs {
    String(String),

    StringArray(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct UrlReplacement {
    #[serde(rename = "ref")]
    pub ref_pattern: String,

    pub test: String,

    pub regex: Option<bool>,

    pub flags: Option<String>,
}
