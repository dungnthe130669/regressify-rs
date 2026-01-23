use std::path::Path;

use anyhow::{Result, anyhow};
use chromiumoxide::browser::{Browser, BrowserConfig, HeadlessMode};
use futures::StreamExt;

use crate::{
    common,
    helpers::{IOExtensions, OnceLockExtensions},
    test::test_scenario::exec_test_scenario,
    types::TestSuiteInput,
};

pub async fn exec_test_suite(visual_tests_dir: &Path, input: TestSuiteInput) -> Result<()> {
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
    let test_suite: crate::types::TestSuite = match format {
        crate::types::TestSuiteFormat::Yaml => serde_yaml::from_str(&test_suite_content)?,
        crate::types::TestSuiteFormat::Json => serde_json::from_str(&test_suite_content)?,
    };

    // create a `Browser` that spawns a `chromium` process running with UI (`with_head()`, headless is default)
    // and the handler that drives the websocket etc.
    eprintln!("Launching browser...");
    let mut config_builder = BrowserConfig::builder();

    if debug {
        config_builder = config_builder.with_head();
    }

    let config = config_builder
        .window_size(1920, 1080)
        .viewport(None)
        .headless_mode(HeadlessMode::New)
        .build()
        .map_err(|e| anyhow!("Cannot create browser config: {:?}", e))?;
    let (mut browser, mut handler) = Browser::launch(config).await?;

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
        exec_test_scenario(&browser, scenario).await?;
    }

    eprintln!("Closing browser...");
    browser.close().await?;

    eprintln!("Waiting for handler to finish...");
    handle.await?;

    Ok(())
}
