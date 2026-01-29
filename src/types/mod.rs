use serde::{Deserialize, Serialize};

mod actions;
mod regressify;

#[cfg(feature = "local")]
pub use actions::*;
#[cfg(feature = "local")]
pub use regressify::*;

#[derive(Debug, Deserialize)]
pub struct BackstopReport {
    pub id: String,
    pub tests: Vec<BackstopTest>,
}

#[derive(Debug, Deserialize)]
pub struct BackstopTest {
    pub pair: BackstopPair,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct BackstopPair {
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
}
