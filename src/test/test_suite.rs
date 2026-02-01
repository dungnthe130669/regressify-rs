use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::{
    helpers::{IOExtensions, slugify},
    snapshot::get_asset_version,
    test::{browsers::launch_browser, test_scenario::exec_test_scenario},
    types::TestSuiteInput,
};

pub async fn exec_test_suite(
    visual_tests_dir: &Path,
    exe_path: &Option<PathBuf>,
    input: &TestSuiteInput,
) -> Result<()> {
    let TestSuiteInput {
        name,
        path: _,
        format,
    } = input;

    let test_suite_path = visual_tests_dir.join(format!("{}.tests.yaml", name));
    if !test_suite_path.exists() {
        return Err(anyhow!(
            "Test suite file '{}' does not exist.",
            test_suite_path.display()
        ));
    }

    // Parse the test suite file (YAML or JSON)
    eprintln!(
        "Loading test suite from '{}'",
        test_suite_path.normalize_string(false)?
    );
    let test_suite_content = std::fs::read_to_string(&test_suite_path)?;

    eprintln!("Parsing test suite...");
    let mut test_suite: crate::types::TestSuite = match format {
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
    } else {
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
    }

    // For each test scenario
    eprintln!("Executing test scenarios...");
    let scenarios_result = async {
        for scenario in &test_suite.scenarios {
            let (mut browser, handle) = launch_browser(exe_path)
                .await
                .map_err(|e| anyhow!("Failed to launch browser: {:?}", e))?;

            exec_test_scenario(&browser, &test_suite, scenario)
                .await
                .ok();

            eprintln!("Closing browser...");
            if let Err(e) = browser.close().await {
                eprintln!("Failed to close browser gracefully: {e}");
            }

            eprintln!("Waiting for handler to finish...");
            if let Err(e) = handle.await {
                eprintln!("Error waiting for browser handler task: {e}");
            }
        }
        Ok(())
    }
    .await;

    scenarios_result
}
