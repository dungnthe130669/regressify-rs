#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyType {
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

    pub scenarios: Vec<Url>,

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
pub struct Url {
    /// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/
    pub actions: Option<Vec<Action>>,

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

/// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/#spoiler--check
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub check: Option<String>,

    pub frame: Option<Selectors>,

    pub click: Option<String>,

    pub goto: Option<String>,

    pub hide: Option<String>,

    pub hover: Option<String>,

    pub append: Option<bool>,

    pub input: Option<String>,

    pub value: Option<Selectors>,

    pub file: Option<Selectors>,

    pub use_file_chooser: Option<bool>,

    pub persist: Option<String>,

    pub key: Option<String>,

    pub press: Option<String>,

    pub remove: Option<String>,

    pub scroll: Option<String>,

    pub label: Option<Selectors>,

    pub select: Option<String>,

    pub uncheck: Option<String>,

    pub url: Option<String>,

    pub wait: Option<Wait>,
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
pub enum Wait {
    Double(f64),

    String(String),
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
