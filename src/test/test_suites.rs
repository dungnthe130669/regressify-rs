use std::fs;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use colored::Colorize;
use tokio::sync::Semaphore;
use tokio::time::{Duration, sleep};
use walkdir::WalkDir;

use crate::{
    cli::args::{RefArgs, SnapshotArgs, TestArgs},
    common,
    compare::report_gen::generate_comparison_report,
    config::{self, replacements},
    helpers::OnceLockExtensions,
    snapshot,
    test::{
        browsers::fetcher,
        test_suite::{exec_single_scenario, parse_test_suite},
    },
    types::{TestSuite, TestSuiteFormat, TestSuiteInput, UrlReplacement},
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
            let Some((suite_name, ext)) = file_name.rsplit_once(".tests.") else {
                eprintln!(
                    "  {} Skipping unexpected test suite file name: {}",
                    "WARNING:".yellow(),
                    file_name
                );
                continue;
            };

            let format = match ext {
                "yaml" | "yml" => TestSuiteFormat::Yaml,
                "json" => TestSuiteFormat::Json,
                _ => {
                    eprintln!(
                        "  {} Skipping test suite with unsupported format '{}': {}",
                        "WARNING:".yellow(),
                        ext,
                        file_name
                    );
                    continue;
                }
            };

            suites.push(TestSuiteInput {
                name: suite_name.to_string(),
                path: entry.path().to_path_buf(),
                format,
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

/// Build a semaphore that ramps concurrency from 1 to `limit`,
/// adding one permit every `step_ms` milliseconds until the target is reached.
fn create_ramped_semaphore(limit: usize) -> Arc<Semaphore> {
    let target = limit.max(1);
    let step_ms = common::ASYNC_CAPTURE_RAMP_MS.get().copied().unwrap_or(1000);
    let semaphore = Arc::new(Semaphore::new(1));

    if target > 1 {
        let sem = Arc::clone(&semaphore);
        tokio::spawn(async move {
            for _ in 1..target {
                sleep(Duration::from_millis(step_ms)).await;
                sem.add_permits(1);
            }
        });
    }

    semaphore
}

fn load_active_replacement_profile(
    visual_tests_dir: &std::path::Path,
    cli_profile: Option<&str>,
) -> Result<Vec<UrlReplacement>> {
    let profile_name = replacements::get_profile_name(cli_profile);
    let explicit_profile = cli_profile.is_some()
        || std::env::var("REPLACEMENT_PROFILE")
            .map(|value| !value.is_empty())
            .unwrap_or(false);

    let Some(profiles) = replacements::load_replacement_profiles(visual_tests_dir)? else {
        if explicit_profile {
            anyhow::bail!(
                "Replacement profile file not found: {}",
                visual_tests_dir
                    .join("common/_replacement-profiles.yaml")
                    .display()
            );
        }
        return Ok(Vec::new());
    };

    Ok(profiles
        .profiles
        .get(&profile_name)
        .cloned()
        .unwrap_or_default())
}

pub async fn exec_test_suites(args: TestArgs) -> Result<()> {
    let TestArgs {
        fetch_browser,
        test_suite: test_suites,
        skip_compare,
        mis_match_threshold,
        replacement_profile,
    } = args;
    let root_dir = common::ROOT_DIR.get_required()?;
    let visual_tests_dir = root_dir.join("visual_tests");

    // Load workspace config
    let workspace_config = config::load_workspace_config(root_dir)?;
    let active_replacements =
        load_active_replacement_profile(&visual_tests_dir, replacement_profile.as_deref())?;

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

    // Parse all test suites upfront
    let parsed_suites: Vec<(TestSuiteInput, Arc<TestSuite>)> = filtered
        .into_iter()
        .filter_map(|t| match parse_test_suite(&visual_tests_dir, &t) {
            Ok(suite) => Some((t, Arc::new(suite))),
            Err(e) => {
                eprintln!("Error parsing test suite '{}': {:?}", t.name, e);
                None
            }
        })
        .collect();

    // Run all scenarios across all suites with a global concurrency limit
    let limit = common::ASYNC_CAPTURE_LIMIT.get().copied().unwrap_or(5);
    let semaphore = create_ramped_semaphore(limit);
    let exe_path = Arc::new(exe_path);
    let workspace_config = Arc::new(workspace_config);
    let active_replacements = Arc::new(active_replacements);
    let mut handles = Vec::new();

    for (_input, suite) in &parsed_suites {
        for scenario_idx in 0..suite.scenarios.len() {
            let sem = semaphore.clone();
            let suite = suite.clone();
            let exe_path = exe_path.clone();
            let ws_config = workspace_config.clone();
            let replacements = active_replacements.clone();

            let handle = tokio::spawn(async move {
                let Ok(_permit) = sem.acquire().await else {
                    eprintln!("  {} Scenario semaphore closed", "ERROR:".red());
                    return false;
                };
                if let Err(e) = exec_single_scenario(
                    &exe_path,
                    &suite,
                    &suite.scenarios[scenario_idx],
                    &suite.scenarios,
                    &ws_config,
                    &replacements,
                )
                .await
                {
                    let label = suite.scenarios[scenario_idx]
                        .label
                        .as_deref()
                        .or(suite.scenarios[scenario_idx].id.as_deref())
                        .unwrap_or("unknown");
                    eprintln!("  {} Scenario '{}' failed: {:?}", "ERROR:".red(), label, e);
                    return false;
                }
                true
            });

            handles.push(handle);
        }
    }

    // Wait for all scenarios to complete
    let mut scenario_failures = 0usize;
    for handle in handles {
        match handle.await {
            Ok(true) => {}
            Ok(false) => scenario_failures += 1,
            Err(e) => {
                eprintln!("Task failed: {:?}", e);
                scenario_failures += 1;
            }
        }
    }

    // Run visual comparison per suite
    let mut comparison_failures = 0usize;
    for (input, suite) in &parsed_suites {
        if !skip_compare
            && let (Some(suite_id), Some(test_date)) =
                (suite.id.as_deref(), suite.test_date.as_deref())
        {
            let regressify_dir = common::REGRESSIFY_DIR.get_required()?;
            let suite_dir = regressify_dir.join(suite_id);

            let threshold = suite.mis_match_threshold.unwrap_or(mis_match_threshold);

            eprintln!("Running visual comparison for '{}'...", input.name);
            if let Err(e) = generate_comparison_report(&suite_dir, suite_id, test_date, threshold) {
                eprintln!("Visual comparison failed for '{}': {:?}", input.name, e);
                comparison_failures += 1;
            }
        }
        eprintln!("Test suite '{}' completed.", input.name);
    }

    generate_snapshot_reports()?;

    if scenario_failures > 0 || comparison_failures > 0 {
        anyhow::bail!(
            "Test run failed: {} scenario(s) failed, {} suite comparison(s) failed",
            scenario_failures,
            comparison_failures
        );
    }

    Ok(())
}

/// `ref` command: capture baseline reference screenshots.
/// Deletes existing references, runs the test suites (without comparison),
/// then promotes captured screenshots directly as the new reference baseline.
pub async fn exec_ref(args: RefArgs) -> Result<()> {
    let RefArgs {
        fetch_browser,
        test_suite: test_suites,
        replacement_profile,
    } = args;
    if replacement_profile.is_some() {
        eprintln!(
            "{}",
            "WARNING: --replacement-profile is ignored in ref mode (references use raw URLs)."
                .yellow()
        );
    }
    let root_dir = common::ROOT_DIR.get_required()?;
    let visual_tests_dir = root_dir.join("visual_tests");

    // Load workspace config
    let workspace_config = config::load_workspace_config(root_dir)?;

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

    // Delete existing reference screenshots before running
    for t in &filtered {
        let suite_dir = regressify_dir.join(&t.name);
        let reference_dir = suite_dir.join("bitmaps_reference");
        if reference_dir.exists() {
            eprintln!(
                "Removing existing reference screenshots for '{}'...",
                t.name
            );
            fs::remove_dir_all(&reference_dir)?;
        }
    }

    // Parse all test suites upfront
    let parsed_suites: Vec<(TestSuiteInput, Arc<TestSuite>)> = filtered
        .into_iter()
        .filter_map(|t| match parse_test_suite(&visual_tests_dir, &t) {
            Ok(suite) => Some((t, Arc::new(suite))),
            Err(e) => {
                eprintln!("Error parsing test suite '{}': {:?}", t.name, e);
                None
            }
        })
        .collect();

    // Run all scenarios across all suites with a global concurrency limit
    let limit = common::ASYNC_CAPTURE_LIMIT.get().copied().unwrap_or(5);
    let semaphore = create_ramped_semaphore(limit);
    let exe_path = Arc::new(exe_path);
    let workspace_config = Arc::new(workspace_config);
    let active_replacements = Arc::new(Vec::<UrlReplacement>::new());
    let mut handles = Vec::new();

    for (_input, suite) in &parsed_suites {
        for scenario_idx in 0..suite.scenarios.len() {
            let sem = semaphore.clone();
            let suite = suite.clone();
            let exe_path = exe_path.clone();
            let ws_config = workspace_config.clone();
            let replacements = active_replacements.clone();

            let handle = tokio::spawn(async move {
                let Ok(_permit) = sem.acquire().await else {
                    eprintln!("  {} Scenario semaphore closed", "ERROR:".red());
                    return false;
                };
                if let Err(e) = exec_single_scenario(
                    &exe_path,
                    &suite,
                    &suite.scenarios[scenario_idx],
                    &suite.scenarios,
                    &ws_config,
                    &replacements,
                )
                .await
                {
                    let label = suite.scenarios[scenario_idx]
                        .label
                        .as_deref()
                        .or(suite.scenarios[scenario_idx].id.as_deref())
                        .unwrap_or("unknown");
                    eprintln!("  {} Scenario '{}' failed: {:?}", "ERROR:".red(), label, e);
                    return false;
                }
                true
            });

            handles.push(handle);
        }
    }

    // Wait for all scenarios to complete
    let mut scenario_failures = 0usize;
    for handle in handles {
        match handle.await {
            Ok(true) => {}
            Ok(false) => scenario_failures += 1,
            Err(e) => {
                eprintln!("Task failed: {:?}", e);
                scenario_failures += 1;
            }
        }
    }

    if scenario_failures > 0 {
        anyhow::bail!(
            "Reference capture failed: {} scenario(s) failed",
            scenario_failures
        );
    }

    // Promote screenshots and run comparison per suite
    for (input, _suite) in &parsed_suites {
        let suite_dir = regressify_dir.join(&input.name);
        let tests_dir = suite_dir.join("bitmaps_test");

        if !tests_dir.exists() {
            eprintln!("No test screenshots found for '{}', skipping.", input.name);
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
                eprintln!(
                    "No test run directories found for '{}', skipping.",
                    input.name
                );
                continue;
            }
        };

        let latest_test_dir = tests_dir.join(&latest_run);
        let ref_dest = suite_dir.join("bitmaps_reference");
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
            input.name.cyan(),
            latest_run
        );

        // Run comparison so results show all scenarios as passed
        eprintln!("Running visual comparison...");
        generate_comparison_report(&suite_dir, &input.name, &latest_run, 0.0)?;
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
