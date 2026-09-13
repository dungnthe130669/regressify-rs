use serde::{Deserialize, Serialize};

mod actions;
mod regressify;

pub use actions::*;
pub use regressify::*;

#[derive(Debug, Deserialize)]
pub struct RegressifyReport {
    pub id: String,
    pub tests: Vec<RegressifyTest>,
}

#[derive(Debug, Deserialize)]
pub struct RegressifyTest {
    pub pair: RegressifyPair,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct RegressifyPair {
    pub reference: Option<String>,
    pub test: Option<String>,
    #[serde(rename = "diffImage")]
    pub diff_image: Option<String>,
    #[serde(rename = "engineErrorMsg")]
    pub engine_error_msg: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HtmlReportSummary {
    pub id: String,
    pub total_tests: usize,
    pub total_passed: usize,
    pub total_failed: usize,
    pub has_engine_errors: bool,
    pub percent: String,
    pub status: &'static str,
    pub passed_class: Option<&'static str>,
    pub failed_class: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenshotType {
    Step(String),
    Final,
    Viewport,
    Selector(String),
}
