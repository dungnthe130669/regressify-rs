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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSuite {
    pub id: Option<String>,

    pub test_date: Option<String>,

    pub async_capture_limit: Option<f64>,

    pub async_compare_limit: Option<f64>,

    pub browser: Option<Browser>,

    pub css_override_path: Option<String>,

    pub debug: Option<bool>,

    pub hide_selectors: Option<Vec<String>>,

    pub ignore_ssl_errors: Option<bool>,

    pub mis_match_threshold: Option<f64>,

    pub post_interaction_wait: Option<f64>,

    pub remove_selectors: Option<Vec<String>>,

    pub scenarios: Vec<TestScenario>,

    pub state: Option<Needs>,

    pub url_replacements: Option<Vec<UrlReplacements>>,

    pub use_css_override: Option<bool>,

    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Browser {
    Chromium,

    Firefox,

    Webkit,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestScenario {
    /// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/
    pub actions: Option<Vec<TestAction>>,

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

    /// Array of selectors to capture. Defaults to document if omitted. Use "viewport" to capture
    /// the viewport size.
    pub selectors: Option<Selectors>,

    pub url: String,

    pub use_css_override: Option<bool>,

    pub viewport_names: Option<Selectors>,
}

/// Array of selectors to capture. Defaults to document if omitted. Use "viewport" to capture
/// the viewport size.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Selectors {
    String(String),

    StringArray(Vec<String>),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Needs {
    String(String),

    StringArray(Vec<String>),
}

#[derive(Serialize, Deserialize)]
pub struct UrlReplacements {
    #[serde(rename = "ref")]
    pub url_replacements_ref: String,

    pub test: String,
}
