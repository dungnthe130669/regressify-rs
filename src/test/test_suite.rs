use std::path::Path;

use anyhow::{Result, anyhow};
use chromiumoxide::{
    browser::{Browser, BrowserConfig, HeadlessMode},
    handler::viewport::Viewport,
};
use futures::StreamExt;

use crate::{
    common,
    helpers::{IOExtensions, OnceLockExtensions, slugify},
    snapshot::get_asset_version,
    test::test_scenario::exec_test_scenario,
    types::TestSuiteInput,
};

pub async fn exec_test_suite(visual_tests_dir: &Path, input: &TestSuiteInput) -> Result<()> {
    let TestSuiteInput {
        name,
        path: _,
        format,
    } = input;
    let debug = *common::DEBUG.get_required()?;

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

    // create a `Browser` that spawns a `chromium` process running with UI (`with_head()`, headless is default)
    // and the handler that drives the websocket etc.
    let mut config_builder = BrowserConfig::builder();

    if debug {
        config_builder = config_builder.with_head();
    }

    let config = config_builder
        .window_size(1920, 1080)
        .viewport(Some(Viewport {
            width: 1920,
            height: 1080,
            device_scale_factor: Some(1.0),
            emulating_mobile: false,
            has_touch: false,
            is_landscape: true,
        }))
        .headless_mode(HeadlessMode::New)
        .args([
            "--disable-infobars",
            "--disable-setuid-sandbox",
            "--ignore-certificate-errors",
            "--ignore-certificate-errors-spki-list",
            "--no-sandbox",
            // "--disable-setuid-sandbox",
            "--disable-gpu",
            "--window-position=0,0",
        ])
        .build()
        .map_err(|e| anyhow!("Cannot create browser config: {:?}", e))?;

    eprintln!("Launching browser...");
    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| anyhow!("Cannot launch browser: {:?}", e))?;

    // spawn a new task that continuously polls the handler
    let handle = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    // For each test scenario
    eprintln!("Executing test scenarios...");
    for scenario in &test_suite.scenarios {
        exec_test_scenario(&browser, &test_suite, scenario).await?;
    }

    eprintln!("Closing browser...");
    browser.close().await?;

    eprintln!("Waiting for handler to finish...");
    handle.await?;

    Ok(())
}
