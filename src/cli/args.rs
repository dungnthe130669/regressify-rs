use anyhow::Result;

use crate::helpers::is_windows_reserved_path_component;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct ScenarioFilter {
    pub suite: String,
    pub scenario: String,
}

impl std::str::FromStr for ScenarioFilter {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (suite, scenario) = value
            .rsplit_once(':')
            .filter(|(suite, scenario)| !suite.trim().is_empty() && !scenario.trim().is_empty())
            .ok_or_else(|| "Scenario selection must be '<suite>:<scenario-id>'".to_string())?;
        Ok(Self {
            suite: suite.to_string(),
            scenario: scenario.to_string(),
        })
    }
}

impl std::fmt::Display for ScenarioFilter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.suite, self.scenario)
    }
}

#[derive(clap::Args)]
pub struct OptimizeArgs {
    /// Compression speed (1 = smallest files, 10 = fastest)
    #[clap(long, visible_alias = "sp", default_value_t = 8, value_parser = speed_parser)]
    pub speed: u8,

    /// Minimum file size (in KB) to optimize
    #[clap(long, visible_alias = "mk", default_value_t = 50)]
    pub min_size_kb: u64,
}

#[derive(clap::Args)]
pub struct SnapshotArgs {
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

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
    /// Download managed Chromium if missing (default: true); false requires cached Chromium
    #[clap(long)]
    pub fetch_browser: Option<bool>,

    /// Names of test suites to execute (or 'all' / '*' for all test suites)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

    /// Capture only this exact <suite>:<scenario-id> (repeat for multiple scenarios)
    #[clap(long = "scenario")]
    pub scenarios: Vec<ScenarioFilter>,

    /// Skip visual comparison after test run
    #[clap(long, default_value_t = false)]
    pub skip_compare: bool,

    /// Mismatch threshold percentage (0.0-100.0). Tests with mismatch above this fail.
    #[clap(
        long,
        visible_alias = "threshold",
        default_value_t = 0.1,
        value_parser = percentage_parser
    )]
    pub mis_match_threshold: f64,

    /// Replacement profile name (overrides REPLACEMENT_PROFILE env var)
    #[clap(long, visible_alias = "rp")]
    pub replacement_profile: Option<String>,
}

#[derive(clap::Args)]
pub struct RefArgs {
    /// Download managed Chromium if missing (default: true); false requires cached Chromium
    #[clap(long)]
    pub fetch_browser: Option<bool>,

    /// Names of test suites to capture references for (or 'all' / '*' for all)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

    /// Capture only this exact <suite>:<scenario-id> (repeat for multiple scenarios)
    #[clap(long = "scenario")]
    pub scenarios: Vec<ScenarioFilter>,

    /// Replacement profile name (overrides REPLACEMENT_PROFILE env var)
    #[clap(long, visible_alias = "rp")]
    pub replacement_profile: Option<String>,
}

#[derive(clap::Args)]
pub struct CompareArgs {
    /// Names of test suites to compare (or 'all' / '*' for all)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

    /// Compare only this exact <suite-output-id>:<scenario-id> from capture metadata
    #[clap(long = "scenario")]
    pub scenarios: Vec<ScenarioFilter>,

    /// Mismatch threshold percentage (0.0-100.0). Tests with mismatch above this fail.
    #[clap(
        long,
        visible_alias = "threshold",
        default_value_t = 0.1,
        value_parser = percentage_parser
    )]
    pub mis_match_threshold: f64,

    /// Maximum image comparisons to run in parallel
    #[clap(long, default_value_t = 2, value_parser = positive_usize_parser)]
    pub async_compare_limit: usize,
}

#[derive(clap::Args)]
pub struct ApproveArgs {
    /// Names of test suites to approve (or 'all' / '*' for all)
    #[clap(long, short = 's')]
    pub test_suite: Vec<String>,

    /// Approve only this exact <suite-output-id>:<scenario-id> from capture metadata
    #[clap(long = "scenario")]
    pub scenarios: Vec<ScenarioFilter>,
}

fn speed_parser(s: &str) -> Result<u8, String> {
    s.parse::<u8>()
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

pub(crate) fn positive_usize_parser(s: &str) -> Result<usize, String> {
    s.parse::<usize>()
        .map_err(|_| format!("'{}' is not a positive integer.", s))
        .and_then(|value| {
            if value > 0 {
                Ok(value)
            } else {
                Err("Value must be greater than zero.".to_string())
            }
        })
}

pub(crate) fn positive_u32_parser(s: &str) -> Result<u32, String> {
    s.parse::<u32>()
        .map_err(|_| format!("'{}' is not a positive integer.", s))
        .and_then(|value| {
            if value > 0 {
                Ok(value)
            } else {
                Err("Value must be greater than zero.".to_string())
            }
        })
}

fn portable_identifier_parser(value: &str, label: &str) -> Result<String, String> {
    const MAX_RUN_ID_BYTES: usize = 80;
    let is_valid = !value.is_empty()
        && value.len() <= MAX_RUN_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if is_valid {
        Ok(value.to_string())
    } else {
        Err(format!(
            "{} must contain only ASCII letters, digits, underscores, or hyphens and be at most {} bytes.",
            label, MAX_RUN_ID_BYTES
        ))
    }
}

pub(crate) fn run_id_parser(value: &str) -> Result<String, String> {
    let run_id = portable_identifier_parser(value, "Run ID")?;
    if is_windows_reserved_path_component(&run_id) {
        return Err("Run ID cannot use a Windows-reserved device name.".to_string());
    }
    Ok(run_id)
}

pub(crate) fn site_env_parser(value: &str) -> Result<String, String> {
    portable_identifier_parser(value, "Site environment")
}

fn percentage_parser(s: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .map_err(|_| format!("'{}' is not a valid percentage.", s))
        .and_then(|value| {
            if value.is_finite() && (0.0..=100.0).contains(&value) {
                Ok(value)
            } else {
                Err(format!(
                    "Percentage must be a finite value between 0 and 100, got {}",
                    value
                ))
            }
        })
}

#[cfg(test)]
mod tests {
    use super::{
        percentage_parser, positive_u32_parser, positive_usize_parser, run_id_parser,
        site_env_parser, speed_parser,
    };

    #[test]
    fn capture_commands_accept_managed_browser_download_settings() {
        use crate::cli::{Command, root::Cli};
        use clap::Parser;

        for command in ["ref", "test"] {
            for (value, expected) in [
                (None, None),
                (Some("true"), Some(true)),
                (Some("false"), Some(false)),
            ] {
                let mut arguments = vec!["regressify", command];
                if let Some(value) = value {
                    arguments.extend(["--fetch-browser", value]);
                }
                let actual = match Cli::try_parse_from(arguments).unwrap().command {
                    Command::Ref(args) => args.fetch_browser,
                    Command::Test(args) => args.fetch_browser,
                    _ => panic!("Expected a capture command"),
                };
                assert_eq!(actual, expected);
            }
            let help = Cli::try_parse_from(["regressify", command, "--help"])
                .err()
                .unwrap()
                .to_string();
            let help = help.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(help.contains("default: true"));
            assert!(help.contains("false requires cached Chromium"));
        }
    }

    #[test]
    fn scenario_selection_parser_preserves_exact_names_and_rejects_empty_parts() {
        for value in ["shop:checkout", "folder/shop:home", "shop[1]:home"] {
            let filter: super::ScenarioFilter = value.parse().unwrap();
            assert_eq!(filter.to_string(), value);
        }
        for value in ["home", ":home", "shop:", "shop: "] {
            assert!(value.parse::<super::ScenarioFilter>().is_err());
        }
    }

    #[test]
    fn optimization_speed_is_limited_to_supported_range() {
        assert_eq!(speed_parser("1"), Ok(1));
        assert_eq!(speed_parser("10"), Ok(10));
        assert!(speed_parser("0").is_err());
        assert!(speed_parser("11").is_err());
    }

    #[test]
    fn positive_worker_count_rejects_zero() {
        assert_eq!(positive_usize_parser("1"), Ok(1));
        assert!(positive_usize_parser("0").is_err());
        assert!(positive_usize_parser("one").is_err());
        assert_eq!(positive_u32_parser("1"), Ok(1));
        assert!(positive_u32_parser("0").is_err());
    }

    #[test]
    fn mismatch_percentage_is_finite_and_in_range() {
        assert_eq!(percentage_parser("0"), Ok(0.0));
        assert_eq!(percentage_parser("100"), Ok(100.0));
        assert!(percentage_parser("-0.1").is_err());
        assert!(percentage_parser("100.1").is_err());
        assert!(percentage_parser("NaN").is_err());
    }

    #[test]
    fn run_ids_require_portable_output_names() {
        assert_eq!(run_id_parser("ci-123_abc"), Ok("ci-123_abc".to_string()));
        assert!(run_id_parser("../../outside").is_err());
        assert!(run_id_parser("release run").is_err());
        assert!(run_id_parser(&"a".repeat(81)).is_err());
        assert!(run_id_parser("CON").is_err());
    }

    #[test]
    fn site_environments_require_portable_output_names() {
        assert_eq!(site_env_parser("staging_2"), Ok("staging_2".to_string()));
        assert!(site_env_parser("../outside").is_err());
        assert!(site_env_parser("staging env").is_err());
    }
}
