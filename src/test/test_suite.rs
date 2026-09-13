use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::{
    config,
    helpers::{IOExtensions, slugify},
    snapshot::get_asset_version,
    test::{
        browsers::{LaunchOptions, launch_browser},
        test_scenario::{ScenarioRequest, exec_test_scenario, make_log_prefix},
    },
    types::{TestSuite, TestSuiteInput},
};

/// Parse a test suite file (YAML or JSON) and assign IDs to scenarios.
pub fn parse_test_suite(visual_tests_dir: &Path, input: &TestSuiteInput) -> Result<TestSuite> {
    let TestSuiteInput { name, path, format } = input;

    let test_suite_path = if path.is_absolute() {
        path.clone()
    } else {
        visual_tests_dir.join(path)
    };
    if !test_suite_path.exists() {
        return Err(anyhow!(
            "Test suite file '{}' does not exist.",
            test_suite_path.display()
        ));
    }

    eprintln!(
        "Loading test suite from '{}'",
        test_suite_path.normalize_string(false)?
    );
    let test_suite_content = std::fs::read_to_string(&test_suite_path)?;

    eprintln!("Parsing test suite...");
    let mut test_suite: TestSuite = match format {
        crate::types::TestSuiteFormat::Yaml => serde_yaml::from_str(&test_suite_content)?,
        crate::types::TestSuiteFormat::Json => serde_json::from_str(&test_suite_content)?,
    };

    if test_suite.id.is_none() {
        test_suite.id = Some(name.clone());
    }

    // Set the test suite date time, format: YYYYMMDD_HHMMSS
    let now = chrono::Utc::now();
    test_suite.test_date = Some(now.format("%Y%m%d_%H%M%S").to_string());

    if test_suite.scenarios.is_empty() {
        return Err(anyhow!("Test suite contains no scenarios."));
    }

    // Loop through scenarios and assign IDs if missing
    for scenario in test_suite.scenarios.iter_mut() {
        match (scenario.id.as_ref(), scenario.label.as_ref()) {
            (None, Some(label)) => {
                scenario.id = Some(slugify(label));
            }
            (None, None) => {
                scenario.id = Some(get_asset_version(scenario.url.as_bytes(), 20));
            }
            (Some(_), _) => {}
        }
    }

    // Ensure all scenario IDs are unique
    let mut scenario_ids = std::collections::HashSet::new();
    for scenario in test_suite.scenarios.iter() {
        let scenario_id = scenario
            .id
            .as_ref()
            .ok_or_else(|| anyhow!("Scenario ID is missing."))?;
        if !scenario_ids.insert(scenario_id) {
            return Err(anyhow!(
                "Duplicate scenario ID found: '{}'. Scenario IDs must be unique.",
                scenario_id
            ));
        }
    }

    Ok(test_suite)
}

/// Execute a single scenario: launch browser, run scenario, close browser.
/// Categorized result of running a single scenario.
///
/// A visual regression tool must keep running (and exit zero) when the *page
/// under test* misbehaves — timeouts, missing elements, navigation errors are
/// expected outcomes that should surface in the comparison report, not crash
/// the run. Only failures of our own infrastructure (launching the browser)
/// should fail the command.
pub enum ScenarioOutcome {
    /// Scenario captured successfully.
    Passed,
    /// The page under test caused the failure (timeout, element not found,
    /// navigation error, etc.). Expected — must NOT fail the command.
    PageFailed(anyhow::Error),
    /// Our infrastructure failed (browser could not be launched). Must fail
    /// the command with a non-zero exit code.
    InfraFailed(anyhow::Error),
}

async fn wait_for_browser_handler(
    handle: &mut tokio::task::JoinHandle<()>,
    timeout: std::time::Duration,
    log_prefix: &str,
) -> bool {
    match tokio::time::timeout(timeout, &mut *handle).await {
        Ok(Ok(())) => {
            eprintln!("{} Browser handler finished cleanly.", log_prefix);
            true
        }
        Ok(Err(e)) => {
            eprintln!("{} Error waiting for browser handler task: {e}", log_prefix);
            false
        }
        Err(_) => {
            eprintln!(
                "{} Browser handler did not finish within {} ms; aborting.",
                log_prefix,
                timeout.as_millis()
            );
            handle.abort();
            let _ = handle.await;
            false
        }
    }
}

pub async fn exec_single_scenario(
    exe_path: &Option<PathBuf>,
    req: ScenarioRequest<'_>,
) -> ScenarioOutcome {
    let log_prefix = make_log_prefix(req.index, req.total);
    // Browser launch is infrastructure — if this fails the environment is
    // broken (e.g. Chrome not installed/launchable), so it must fail the run.
    let resolved = config::resolve_config(req.workspace_config, req.test_suite, req.scenario);
    let launch_options = LaunchOptions {
        debug: resolved.debug,
        ignore_ssl_errors: resolved.ignore_ssl_errors,
    };
    let (mut browser, handle, user_data_dir) =
        match launch_browser(exe_path, &log_prefix, launch_options).await {
            Ok(triple) => triple,
            Err(e) => {
                return ScenarioOutcome::InfraFailed(anyhow!("Failed to launch browser: {:?}", e));
            }
        };

    // Everything from here on interacts with the page under test. Any error is
    // a page-level failure that must not fail the command.
    let scenario_result = exec_test_scenario(&browser, req).await;

    eprintln!("{} Closing browser...", log_prefix);
    let browser_closed =
        match tokio::time::timeout(std::time::Duration::from_secs(10), browser.close()).await {
            Ok(Ok(_)) => true,
            Ok(Err(e)) => {
                eprintln!("{} Failed to close browser gracefully: {e}", log_prefix);
                false
            }
            Err(_) => {
                eprintln!(
                    "{} Browser close did not finish within 10000 ms.",
                    log_prefix
                );
                false
            }
        };

    eprintln!("{} Waiting for handler to finish...", log_prefix);
    let mut handle = handle;
    let handler_finished =
        wait_for_browser_handler(&mut handle, std::time::Duration::from_secs(5), &log_prefix).await;

    // The handler normally removes this directory. Only retry after either the
    // browser or handler has confirmed shutdown; deleting it earlier can race a
    // still-running Chromium process.
    if browser_closed || handler_finished {
        let _ = std::fs::remove_dir_all(&user_data_dir);
    } else {
        eprintln!(
            "{} Keeping browser profile '{}' because shutdown was not confirmed.",
            log_prefix,
            user_data_dir.display()
        );
    }

    match scenario_result {
        Ok(()) => ScenarioOutcome::Passed,
        Err(e) => ScenarioOutcome::PageFailed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TestSuiteFormat;

    #[test]
    fn parse_test_suite_uses_discovered_json_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("alloy.tests.json");
        std::fs::write(
            &path,
            r#"{"scenarios":[{"url":"https://json.example.com"}]}"#,
        )
        .unwrap();

        let suite = parse_test_suite(
            temp_dir.path(),
            &TestSuiteInput {
                name: "alloy".to_string(),
                path,
                format: TestSuiteFormat::Json,
            },
        )
        .unwrap();

        assert_eq!(suite.id.as_deref(), Some("alloy"));
        assert_eq!(suite.scenarios[0].url, "https://json.example.com");
    }

    #[test]
    fn parse_test_suite_uses_discovered_yml_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("alloy.tests.yml");
        std::fs::write(&path, "scenarios:\n  - url: https://yml.example.com\n").unwrap();

        let suite = parse_test_suite(
            temp_dir.path(),
            &TestSuiteInput {
                name: "alloy".to_string(),
                path,
                format: TestSuiteFormat::Yaml,
            },
        )
        .unwrap();

        assert_eq!(suite.id.as_deref(), Some("alloy"));
        assert_eq!(suite.scenarios[0].url, "https://yml.example.com");
    }

    #[tokio::test]
    async fn timed_out_handler_is_aborted_before_returning() {
        let mut handle = tokio::spawn(std::future::pending::<()>());

        let finished =
            wait_for_browser_handler(&mut handle, std::time::Duration::from_millis(1), "[test]")
                .await;

        assert!(!finished);
        assert!(handle.is_finished());
    }
}
