use anyhow::{Result, anyhow};
use colored::Colorize;
use walkdir::WalkDir;

use crate::{
    cli::args::{RefArgs, TestArgs},
    common,
    compare::report_gen::approve_screenshots,
    helpers::OnceLockExtensions,
    test::{browsers::fetcher, test_suite::exec_test_suite},
    types::{TestSuiteFormat, TestSuiteInput},
};

/// Discover available test suite files in the visual_tests/ directory.
fn discover_test_suites(visual_tests_dir: &std::path::Path) -> Vec<TestSuiteInput> {
    let mut suites = Vec::new();
    for entry in WalkDir::new(visual_tests_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_name = entry.file_name().to_string_lossy().to_lowercase();
        if file_name.ends_with(".tests.yaml")
            || file_name.ends_with(".tests.yml")
            || file_name.ends_with(".tests.json")
        {
            let (suite_name, ext) = file_name
                .rsplit_once(".tests.")
                .expect("File name should contain '.tests.'");

            suites.push(TestSuiteInput {
                name: suite_name.to_string(),
                path: entry.path().to_path_buf(),
                format: match ext {
                    "yaml" | "yml" => TestSuiteFormat::Yaml,
                    "json" => TestSuiteFormat::Json,
                    _ => panic!("Unsupported test suite format: {}", ext),
                },
            });
        }
    }
    suites
}

/// Filter test suites by glob patterns (or return all if patterns is empty / contains "all" / "*").
fn filter_test_suites(available: Vec<TestSuiteInput>, patterns: &[String]) -> Vec<TestSuiteInput> {
    if patterns.is_empty()
        || patterns.contains(&"all".to_string())
        || patterns.contains(&"*".to_string())
    {
        available
    } else {
        available
            .into_iter()
            .filter(|ts| {
                patterns.iter().any(|pattern| {
                    glob::Pattern::new(pattern)
                        .map(|p| p.matches(&ts.name))
                        .unwrap_or(false)
                })
            })
            .collect()
    }
}

pub async fn exec_test_suites(args: TestArgs) -> Result<()> {
    let TestArgs {
        fetch_browser,
        test_suite: test_suites,
        skip_compare,
        mis_match_threshold,
    } = args;
    let root_dir = common::ROOT_DIR.get_required()?;
    let visual_tests_dir = root_dir.join("visual_tests");

    let exe_path = if fetch_browser.unwrap_or(false) {
        Some(
            fetcher()
                .await
                .map_err(|e| anyhow!("Failed to fetch the browser: {:?}", e))?,
        )
    } else {
        None
    };

    let available = discover_test_suites(&visual_tests_dir);
    let filtered = filter_test_suites(available, &test_suites);

    for t in filtered {
        match exec_test_suite(
            &visual_tests_dir,
            &exe_path,
            &t,
            skip_compare,
            mis_match_threshold,
        )
        .await
        {
            Ok(_) => eprintln!("Test suite '{}' completed successfully.", t.name),
            Err(e) => eprintln!("Error executing test suite '{}': {:?}", t.name, e),
        }
    }

    Ok(())
}

/// `ref` command: capture reference screenshots.
/// Runs the test suites (without comparison), then auto-approves results as reference.
pub async fn exec_ref(args: RefArgs) -> Result<()> {
    let RefArgs {
        fetch_browser,
        test_suite: test_suites,
    } = args;
    let root_dir = common::ROOT_DIR.get_required()?;
    let visual_tests_dir = root_dir.join("visual_tests");

    let exe_path = if fetch_browser.unwrap_or(false) {
        Some(
            fetcher()
                .await
                .map_err(|e| anyhow!("Failed to fetch the browser: {:?}", e))?,
        )
    } else {
        None
    };

    let available = discover_test_suites(&visual_tests_dir);
    let filtered = filter_test_suites(available, &test_suites);

    let backstop_dir = common::BACKSTOP_DIR.get_required()?;

    for t in &filtered {
        // Run tests without comparison (skip_compare=true)
        match exec_test_suite(&visual_tests_dir, &exe_path, t, true, 0.0).await {
            Ok(_) => {
                // Auto-approve: promote test screenshots to reference
                let suite_dir = backstop_dir.join(&t.name);
                if suite_dir.exists() {
                    eprintln!("Approving reference screenshots for '{}'...", t.name.cyan());
                    if let Err(e) = approve_screenshots(&suite_dir, &t.name) {
                        eprintln!("Error approving '{}': {:?}", t.name, e);
                    }
                }
            }
            Err(e) => eprintln!("Error executing test suite '{}': {:?}", t.name, e),
        }
    }

    Ok(())
}
