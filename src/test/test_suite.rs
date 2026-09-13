use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::{
    common, config,
    helpers::{IOExtensions, slugify},
    snapshot::get_asset_version,
    test::{
        browsers::{LaunchOptions, launch_browser},
        capture::is_artifact_write_error,
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

    // An explicit CLI run ID groups every selected suite under one known run;
    // otherwise use a sortable timestamp that avoids same-second collisions.
    let now = chrono::Utc::now();
    test_suite.test_date = Some(resolve_test_run_id(
        common::RUN_ID.get().map(String::as_str),
        now,
    ));

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

    // Screenshot names must remain unique on case-insensitive filesystems.
    let mut scenario_ids = std::collections::HashSet::new();
    for scenario in test_suite.scenarios.iter() {
        let scenario_id = scenario
            .id
            .as_ref()
            .ok_or_else(|| anyhow!("Scenario ID is missing."))?;
        if !scenario_ids.insert(scenario_id.to_lowercase()) {
            return Err(anyhow!(
                "Duplicate scenario ID found: '{}'. Scenario IDs must be unique.",
                scenario_id
            ));
        }
    }

    Ok(test_suite)
}

fn test_run_id(time: chrono::DateTime<chrono::Utc>) -> String {
    format!(
        "{}_{:09}",
        time.format("%Y%m%d_%H%M%S"),
        time.timestamp_subsec_nanos()
    )
}

fn resolve_test_run_id(
    explicit_run_id: Option<&str>,
    time: chrono::DateTime<chrono::Utc>,
) -> String {
    explicit_run_id
        .map(str::to_owned)
        .unwrap_or_else(|| test_run_id(time))
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
    /// The browser or an output artifact failed. Must fail the command with a
    /// non-zero exit code.
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

    classify_scenario_result(scenario_result)
}

fn classify_scenario_result(result: Result<()>) -> ScenarioOutcome {
    match result {
        Ok(()) => ScenarioOutcome::Passed,
        Err(error) if is_artifact_write_error(&error) => ScenarioOutcome::InfraFailed(error),
        Err(error) => ScenarioOutcome::PageFailed(error),
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

    #[test]
    fn test_run_ids_are_unique_and_sortable_within_one_second() {
        let first = chrono::DateTime::from_timestamp(1_700_000_000, 123).unwrap();
        let second = chrono::DateTime::from_timestamp(1_700_000_000, 456).unwrap();
        let first_id = test_run_id(first);
        let second_id = test_run_id(second);

        assert_eq!(first_id, "20231114_221320_000000123");
        assert_eq!(second_id, "20231114_221320_000000456");
        assert!(first_id < second_id);
    }

    #[test]
    fn explicit_run_id_overrides_generated_timestamp() {
        let time = chrono::DateTime::from_timestamp(1_700_000_000, 123).unwrap();

        assert_eq!(resolve_test_run_id(Some("ci-123"), time), "ci-123");
        assert_eq!(resolve_test_run_id(None, time), "20231114_221320_000000123");
    }

    #[test]
    fn parse_test_suite_rejects_case_insensitive_scenario_id_collisions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("suite.tests.yaml");
        std::fs::write(
            &path,
            "scenarios:\n  - id: Home\n    url: https://example.com/home\n  - id: home\n    url: https://example.com/other\n",
        )
        .unwrap();

        let result = parse_test_suite(
            temp_dir.path(),
            &TestSuiteInput {
                name: "suite".to_string(),
                path,
                format: TestSuiteFormat::Yaml,
            },
        );

        assert!(
            matches!(result, Err(error) if error.to_string().contains("Duplicate scenario ID"))
        );
    }

    #[test]
    fn parse_test_suite_rejects_unknown_suite_or_scenario_properties() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("suite.tests.yaml");
        std::fs::write(
            &path,
            "postinteractionwait: 5000\nscenarios:\n  - url: https://example.com\n    delayy: 10\n",
        )
        .unwrap();

        let result = parse_test_suite(
            temp_dir.path(),
            &TestSuiteInput {
                name: "suite".to_string(),
                path,
                format: TestSuiteFormat::Yaml,
            },
        );

        assert!(matches!(result, Err(error) if error.to_string().contains("unknown field")));
    }

    #[test]
    fn bundled_visual_suites_deserialize_with_the_strict_schema() {
        let visual_tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("visual_tests");
        for entry in std::fs::read_dir(&visual_tests_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let is_yaml = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension == "yaml" || extension == "yml");
            if !is_yaml {
                continue;
            }

            let result = parse_test_suite(
                &visual_tests_dir,
                &TestSuiteInput {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path,
                    format: TestSuiteFormat::Yaml,
                },
            );
            if let Err(error) = result {
                panic!("Bundled visual suite failed strict parsing: {}", error);
            }
        }
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

    #[test]
    fn artifact_write_failure_is_infrastructure_failure() {
        let error = crate::test::capture::artifact_write_error("write screenshot", "disk full");

        assert!(matches!(
            classify_scenario_result(Err(error)),
            ScenarioOutcome::InfraFailed(_)
        ));
        assert!(matches!(
            classify_scenario_result(Err(anyhow!("page failed"))),
            ScenarioOutcome::PageFailed(_)
        ));
    }
}
