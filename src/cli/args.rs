use anyhow::Result;

#[derive(clap::Args)]
pub struct OptimizeArgs {
    /// Min quality for image optimization (0-100)
    #[clap(long, visible_alias = "mq", default_value_t = 0, value_parser = quality_parser)]
    pub min_quality: u8,

    /// Max quality for image optimization (0-100)
    #[clap(long, visible_alias = "Mq", default_value_t = 30, value_parser = quality_parser)]
    pub max_quality: u8,

    /// Speed for image optimization (1-10)
    #[clap(long, visible_alias = "sp", default_value_t = 8, value_parser = speed_parser)]
    pub speed: i32,

    /// Minimum file size (in KB) to optimize
    #[clap(long, visible_alias = "mk", default_value_t = 50)]
    pub min_size_kb: u64,
}

#[derive(clap::Args)]
pub struct SnapshotArgs {
    /// The title of the report
    #[clap(long, default_value = "Visual Tests Summary | Regressify")]
    pub title: String,

    /// The heading of the report
    #[clap(long, default_value = "Visual Tests Summary")]
    pub heading: String,

    /// The subheading of the report. Support Markdown.
    #[clap(long, default_value = "Snapshot regression test results")]
    pub subheading: String,
}

#[derive(clap::Args)]
pub struct TestArgs {
    /// Whether to fetch the browser before running tests
    #[clap(long)]
    pub fetch_browser: Option<bool>,

    /// Names of test suites to execute (or 'all' / '*' for all test suites)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

    /// Skip visual comparison after test run
    #[clap(long, default_value_t = false)]
    pub skip_compare: bool,

    /// Mismatch threshold percentage (0.0-100.0). Tests with mismatch above this fail.
    #[clap(long, visible_alias = "threshold", default_value_t = 0.1)]
    pub mis_match_threshold: f64,

    /// Replacement profile name (overrides REPLACEMENT_PROFILE env var)
    #[clap(long, visible_alias = "rp")]
    pub replacement_profile: Option<String>,
}

#[derive(clap::Args)]
pub struct RefArgs {
    /// Whether to fetch the browser before running tests
    #[clap(long)]
    pub fetch_browser: Option<bool>,

    /// Names of test suites to capture references for (or 'all' / '*' for all)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

    /// Replacement profile name (overrides REPLACEMENT_PROFILE env var)
    #[clap(long, visible_alias = "rp")]
    pub replacement_profile: Option<String>,
}

#[derive(clap::Args)]
pub struct CompareArgs {
    /// Names of test suites to compare (or 'all' / '*' for all)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

    /// Mismatch threshold percentage (0.0-100.0). Tests with mismatch above this fail.
    #[clap(long, visible_alias = "threshold", default_value_t = 0.1)]
    pub mis_match_threshold: f64,
}

#[derive(clap::Args)]
pub struct ApproveArgs {
    /// Names of test suites to approve (or 'all' / '*' for all)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,
}

fn quality_parser(s: &str) -> Result<u8, String> {
    s.parse::<u8>()
        .map_err(|_| {
            format!(
                "'{}' is not a valid number. Quality must be an integer between 0 and 100.",
                s
            )
        })
        .and_then(|value| {
            if value <= 100 {
                Ok(value)
            } else {
                Err(format!("Quality must be between 0 and 100, got {}", value))
            }
        })
}

fn speed_parser(s: &str) -> Result<i32, String> {
    s.parse::<i32>()
        .map_err(|_| {
            format!(
                "'{}' is not a valid number. Speed must be an integer between 1 and 10.",
                s
            )
        })
        .and_then(|value| {
            if (1..=10).contains(&value) {
                Ok(value)
            } else {
                Err(format!("Speed must be between 1 and 10, got {}", value))
            }
        })
}
