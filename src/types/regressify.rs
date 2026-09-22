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

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct TestSuite {
    /// Stable suite identifier used for report and screenshot folders. Normally generated from the suite filename.
    pub id: Option<String>,

    /// Run timestamp recorded by Regressify. Normally leave this unset when authoring tests.
    pub test_date: Option<String>,

    /// Maximum simultaneous browser captures. Positive whole number; inherits the workspace setting (default: 5).
    #[schemars(range(min = 1), example = 5)]
    pub async_capture_limit: Option<usize>,

    /// Maximum simultaneous image comparisons. Positive whole number; inherits the workspace setting (default: 2).
    #[schemars(range(min = 1), example = 2)]
    pub async_compare_limit: Option<usize>,

    /// Browser engine. Only "chromium" is supported; normally leave unset to inherit it.
    pub browser: Option<Browser>,

    /// HTTP Basic Auth for this suite. Supply one object or a list; every entry needs origin, username, and password.
    pub basic_auth: Option<BasicAuthConfig>,

    /// Bypass the site's Content Security Policy. Inherits the workspace setting (default: false).
    pub bypass_csp: Option<bool>,

    /// Cookie YAML or JSON file to load before navigation. Inherits the workspace setting.
    #[schemars(example = "common/_cookies.yaml")]
    pub cookie_path: Option<String>,

    /// CSS file injected before capture. Inherits the workspace setting; useCssOverride controls whether it is applied.
    #[schemars(example = "common/_override.css")]
    pub css_override_path: Option<String>,

    /// Show the browser while capturing (default: false). Always disabled in CI environments without a display.
    pub debug: Option<bool>,

    /// CSS selectors to hide before capture while keeping their layout space. A list is required, even for one selector.
    #[schemars(example = vec![".timestamp", ".cookie-banner"])]
    pub hide_selectors: Option<Vec<String>>,

    /// Ignore HTTPS certificate errors for this suite. Inherits the workspace setting (default: false).
    pub ignore_ssl_errors: Option<bool>,

    /// JavaScript file executed when the page is ready, before capture. Inherits the workspace setting.
    #[schemars(example = "common/_on-ready.js")]
    pub js_on_ready_path: Option<String>,

    /// Allowed percentage of different pixels, from 0 to 100. Inherits the workspace setting (default: 0.1, meaning 0.1%).
    #[schemars(range(min = 0.0, max = 100.0), example = 0.1)]
    pub mis_match_threshold: Option<f64>,

    /// Milliseconds to wait after interactions before capturing. Non-negative; inherits the workspace setting (default: 1000).
    #[schemars(range(min = 0.0), example = 1000)]
    pub post_interaction_wait: Option<f64>,

    /// CSS selectors to remove from the page before capture, including their layout space. A list is required.
    #[schemars(example = vec![".advertisement"])]
    pub remove_selectors: Option<Vec<String>>,

    /// Independent test scenarios. Each list item requires a url and may contain actions and capture settings.
    #[schemars(example = serde_json::json!([{"url": "https://example.com", "label": "Home page"}]))]
    pub scenarios: Vec<TestScenario>,

    /// Saved browser state name or list of names, without .json. Loaded from the active output directory's states folder.
    #[schemars(example = "signed-in")]
    pub state: Option<Needs>,

    /// URL replacement rules applied in order. Each object requires ref (text to match) and test (replacement).
    pub url_replacements: Option<Vec<UrlReplacement>>,

    /// Apply the CSS override file before capture. Inherits the workspace setting (default: true).
    pub use_css_override: Option<bool>,

    /// Viewport label or list of labels from the viewport file. Matching ignores case; an empty list selects all viewports.
    #[schemars(example = vec!["phone", "desktop"])]
    pub viewport_names: Option<Selectors>,

    /// Workspace-relative viewport definition file. Default: common/_viewports.yaml. Explicit paths must exist inside the workspace.
    #[schemars(example = "common/_viewports.yaml")]
    pub viewports_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(schemars::JsonSchema)]
pub enum Browser {
    Chromium,

    #[schemars(skip)]
    Firefox,

    #[schemars(skip)]
    Webkit,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct TestScenario {
    /// Browser actions, executed in order. Use one action per list item; a failed action stops this scenario but not other scenarios.
    pub actions: Option<Vec<TestAction>>,

    /// Legacy hover shortcut. Prefer an explicit hover action for ordered interactions.
    pub hover_selector: Option<String>,
    /// Legacy hover shortcut accepting one CSS selector or a list. Prefer explicit hover actions.
    pub hover_selectors: Option<Selectors>,
    /// Legacy click shortcut accepting one CSS selector or a list. Prefer explicit click actions.
    pub click_selectors: Option<Selectors>,
    /// Legacy keypress shortcut with selector and keyPress. Prefer a press action with key.
    pub key_press_selector: Option<KeyPressSelector>,
    /// List of legacy keypress shortcuts. Each entry needs selector and keyPress; prefer explicit press actions.
    pub key_press_selectors: Option<Vec<KeyPressSelector>>,
    /// Legacy CSS selector to scroll into view. Prefer an explicit scroll action.
    pub scroll_to_selector: Option<String>,
    /// CSS selector to wait for after navigation, before interactions. Use readyTimeout to override its timeout.
    #[schemars(example = &"main")]
    pub ready_selector: Option<String>,
    /// Whole milliseconds to wait for readySelector. Omit to use the configured element timeout (default: 30000).
    #[schemars(example = 30000)]
    pub ready_timeout: Option<u64>,
    /// Navigation settings, nested under gotoParameters (waitUntil, timeout, and referer).
    pub engine_options: Option<EngineOptions>,
    /// Node.js hook executed before navigation. Requires Playwright or playwright-core installed in the workspace.
    pub on_before_script: Option<String>,
    /// Node.js hook executed after the page is ready. Requires Playwright or playwright-core installed in the workspace.
    pub on_ready_script: Option<String>,

    /// HTTP Basic Auth for this scenario. Each object needs origin, username, and password; matching origins override inherited credentials.
    pub basic_auth: Option<BasicAuthConfig>,

    /// Bypass the site's Content Security Policy. Inherits the suite or workspace setting (default: false).
    pub bypass_csp: Option<bool>,

    /// Legacy CSS selector to click. Prefer an explicit click action.
    pub click_selector: Option<String>,

    /// Cookie YAML or JSON file to load before navigation. Inherits the suite or workspace setting.
    #[schemars(example = "common/_cookies.yaml")]
    pub cookie_path: Option<String>,

    /// CSS file injected before capture. Inherits the suite or workspace setting.
    #[schemars(example = "common/_override.css")]
    pub css_override_path: Option<String>,

    /// Milliseconds to wait before capture. Non-negative; inherits the workspace setting (default: 1000).
    #[schemars(range(min = 0.0), example = 1000)]
    pub delay: Option<f64>,

    /// Free-text notes about what this scenario covers.
    pub description: Option<String>,

    /// CSS selectors to hide without removing layout space. A list is required; replaces inherited hideSelectors.
    #[schemars(example = vec![".timestamp"])]
    pub hide_selectors: Option<Vec<String>>,

    /// Unique identifier used by needs and reports. Omit to generate it from the label or URL.
    #[schemars(example = "sign-in")]
    pub id: Option<String>,

    /// JavaScript file executed when the page is ready, before capture. Inherits the suite or workspace setting.
    #[schemars(example = "common/_on-ready.js")]
    pub js_on_ready_path: Option<String>,

    /// Human-readable scenario name shown in reports and navigation. Also used to generate an ID when id is omitted.
    #[schemars(example = "Home page")]
    pub label: Option<String>,

    /// Allowed percentage of different pixels, from 0 to 100. Inherits the suite or workspace setting (default: 0.1%).
    #[schemars(range(min = 0.0, max = 100.0), example = 0.1)]
    pub mis_match_threshold: Option<f64>,

    /// ID or list of IDs of other scenarios in this suite whose actions run first. Do not reference this scenario or create cycles.
    #[schemars(example = "sign-in")]
    pub needs: Option<Needs>,

    /// Skip the automatic page scroll used to load lazy content before capture. Inherits the workspace setting (default: false).
    pub no_scroll_top: Option<bool>,

    /// Milliseconds to wait after interactions before capture. Non-negative; inherits the suite or workspace setting (default: 1000).
    #[schemars(range(min = 0.0), example = 1000)]
    pub post_interaction_wait: Option<f64>,

    /// CSS selectors to remove, including layout space. A list is required; replaces inherited removeSelectors.
    #[schemars(example = vec![".advertisement"])]
    pub remove_selectors: Option<Vec<String>>,

    /// Login requirement flag exposed to ready scripts. This does not perform a login automatically.
    pub required_login: Option<bool>,

    /// Saved state name or list of names to restore in addition to inherited state. Created by the persist action.
    #[schemars(example = "signed-in")]
    pub restore: Option<Needs>,

    /// CSS selectors to capture instead of the full page. Use "viewport" for
    /// the visible viewport; each selector produces an independent screenshot.
    #[schemars(example = &"viewport", example = vec!["main", ".summary"])]
    pub selectors: Option<Selectors>,

    /// Required page URL to open before actions, for example https://example.com. URL replacement rules are applied before navigation.
    #[schemars(length(min = 1), example = "https://example.com")]
    pub url: String,

    /// Apply the CSS override file before capture. Inherits the suite or workspace setting (default: true).
    pub use_css_override: Option<bool>,

    /// Viewport label or list of labels from the viewport file. Replaces inherited selection; an empty list selects all viewports.
    #[schemars(example = vec!["phone", "desktop"])]
    pub viewport_names: Option<Selectors>,
}

/// One or more CSS selectors to capture. Use "viewport" for the visible
/// viewport; omit the property for the full page.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
pub enum Selectors {
    String(String),

    StringArray(Vec<String>),
}

impl Selectors {
    pub fn values(&self) -> &[String] {
        match self {
            Self::String(value) => std::slice::from_ref(value),
            Self::StringArray(values) => values,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct KeyPressSelector {
    /// CSS selector of the element that receives the keypress. Quote selectors beginning with # in YAML.
    #[schemars(example = "#search")]
    pub selector: String,
    /// Key or combination, such as Enter, Tab, Ctrl+A, or Shift+Tab.
    #[schemars(example = &"Enter")]
    pub key_press: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct EngineOptions {
    /// Navigation checkpoint, timeout in milliseconds, and optional Referer header.
    pub goto_parameters: Option<GotoParameters>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct GotoParameters {
    /// Navigation checkpoint: commit, domcontentloaded, load, or networkidle. Default: load.
    pub wait_until: Option<NavigationWait>,
    /// Navigation timeout in whole milliseconds. Zero disables this navigation timeout, not the overall scenario timeout.
    #[schemars(example = 30000)]
    pub timeout: Option<u64>,
    /// Optional Referer HTTP header for navigation.
    #[schemars(example = "https://example.com")]
    pub referer: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(schemars::JsonSchema)]
pub enum NavigationWait {
    Commit,
    DomContentLoaded,
    #[default]
    Load,
    NetworkIdle,
}

/// A name or a list of names, processed in the given order.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
pub enum Needs {
    String(String),

    StringArray(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct BasicAuth {
    /// Website origin to authenticate, including the scheme and optional port, for example
    /// https://example.com. Required for every entry; credentials are only sent to this origin.
    #[schemars(length(min = 1), example = "https://example.com")]
    pub origin: String,
    /// HTTP Basic Auth username. Use "${BASIC_AUTH_USERNAME}" to read an environment variable.
    #[schemars(length(min = 1), example = "${BASIC_AUTH_USERNAME}")]
    pub username: String,
    /// HTTP Basic Auth password. Use "${BASIC_AUTH_PASSWORD}" instead of committing a secret.
    #[schemars(length(min = 1), example = "${BASIC_AUTH_PASSWORD}")]
    pub password: String,
}

/// HTTP Basic Auth credentials as one object or a list of objects. Each entry needs
/// origin, username, and password. Credentials for a matching origin override inherited ones.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
#[schemars(example = serde_json::json!({"origin": "https://example.com", "username": "${BASIC_AUTH_USERNAME}", "password": "${BASIC_AUTH_PASSWORD}"}))]
pub enum BasicAuthConfig {
    Single(BasicAuth),
    Multiple(Vec<BasicAuth>),
}

impl BasicAuthConfig {
    pub fn entries(&self) -> &[BasicAuth] {
        match self {
            Self::Single(entry) => std::slice::from_ref(entry),
            Self::Multiple(entries) => entries,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct UrlReplacement {
    /// Text to find in the URL, or a regular expression when regex is true.
    #[serde(rename = "ref")]
    #[schemars(example = "https://production.example.com")]
    pub ref_pattern: String,

    /// Replacement text. May contain capture references such as $1 when regex is true.
    #[schemars(example = "https://staging.example.com")]
    pub test: String,

    /// Treat ref as a regular expression (default: false). Otherwise perform a literal replacement.
    pub regex: Option<bool>,

    /// Regular-expression flags, such as i for case-insensitive matching. Only used when regex is true.
    #[schemars(example = &"i")]
    pub flags: Option<String>,
}
