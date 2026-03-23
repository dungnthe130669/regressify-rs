use std::fs;

use anyhow::{Result, anyhow};
use colored::Colorize;
use walkdir::WalkDir;

use crate::{
    cli::args::{RefArgs, SnapshotArgs, TestArgs},
    common,
    compare::report_gen::generate_comparison_report,
    helpers::OnceLockExtensions,
    snapshot,
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

    generate_snapshot_reports()?;

    Ok(())
}

/// `ref` command: capture baseline reference screenshots.
/// Deletes existing references, runs the test suites (without comparison),
/// then promotes captured screenshots directly as the new reference baseline.
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

    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    for t in &filtered {
        let suite_dir = regressify_dir.join(&t.name);

        // Delete existing reference screenshots
        let reference_dir = suite_dir.join("bitmaps_reference");
        if reference_dir.exists() {
            eprintln!(
                "Removing existing reference screenshots for '{}'...",
                t.name
            );
            fs::remove_dir_all(&reference_dir)?;
        }

        // Run tests without comparison (skip_compare=true, no reference to compare against)
        match exec_test_suite(&visual_tests_dir, &exe_path, t, true, 0.0).await {
            Ok(_) => {
                // Approve directly from tests/ directory
                let tests_dir = suite_dir.join("tests");
                if !tests_dir.exists() {
                    eprintln!("No test screenshots found for '{}', skipping.", t.name);
                    continue;
                }

                // Find the latest test run
                let mut test_runs: Vec<String> = Vec::new();
                for entry in fs::read_dir(&tests_dir)?.flatten() {
                    if entry.path().is_dir() {
                        test_runs.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
                test_runs.sort();

                let latest_run = match test_runs.last() {
                    Some(run) => run.clone(),
                    None => {
                        eprintln!("No test run directories found for '{}', skipping.", t.name);
                        continue;
                    }
                };

                let latest_test_dir = tests_dir.join(&latest_run);
                let ref_dest = suite_dir.join("bitmaps_reference").join(&t.name);
                fs::create_dir_all(&ref_dest)?;

                let mut count = 0;
                for entry in fs::read_dir(&latest_test_dir)?.flatten() {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if file_name.ends_with(".png") {
                        fs::copy(entry.path(), ref_dest.join(&file_name))?;
                        count += 1;
                    }
                }

                eprintln!(
                    "Captured {} reference screenshot(s) for '{}' (run: {}).",
                    count.to_string().green(),
                    t.name.cyan(),
                    latest_run
                );

                // Run comparison so results show all scenarios as passed
                eprintln!("Running visual comparison...");
                if let Err(e) = generate_comparison_report(&suite_dir, &t.name, &latest_run, 0.0) {
                    eprintln!("Visual comparison failed: {:?}", e);
                }
            }
            Err(e) => eprintln!("Error executing test suite '{}': {:?}", t.name, e),
        }
    }

    generate_snapshot_reports()?;

    Ok(())
}

fn generate_snapshot_reports() -> Result<()> {
    snapshot::snapshot(SnapshotArgs {
        title: "Visual Tests Summary | Regressify".to_string(),
        heading: "Visual Tests Summary".to_string(),
        subheading: "Snapshot regression test results".to_string(),
    })
}
