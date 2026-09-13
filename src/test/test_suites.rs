use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use colored::Colorize;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};
use walkdir::WalkDir;

use crate::{
    cli::args::{RefArgs, SnapshotArgs, TestArgs},
    common,
    compare::report_gen::generate_comparison_report,
    config::{self, replacements},
    helpers::{OnceLockExtensions, copy_file_atomically},
    snapshot,
    test::{
        browsers::fetcher,
        capture,
        test_scenario::{
            ScenarioRequest, make_log_prefix, scenario_capture_order_keys,
            scenario_capture_viewports,
        },
        test_suite::{ScenarioOutcome, exec_single_scenario, parse_test_suite},
    },
    types::{
        Browser as TestBrowser, Needs, TestScenario, TestSuite, TestSuiteFormat, TestSuiteInput,
        UrlReplacement,
    },
};

/// Outcome of a spawned scenario task, used to decide the process exit code.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TaskStatus {
    /// Scenario captured successfully.
    Passed,
    /// Page-level failure (timeout, not found, ...) — does NOT fail the command.
    PageFailed,
    /// Infrastructure failure (browser launch) — fails the command.
    InfraFailed,
}

/// One independent capture unit: a single scenario at a single viewport, with a
/// dedicated global index. Built upfront (TS/BackstopJS-style) so every
/// scenario/viewport pair runs as its own task with its own browser and index.
struct CaptureJob {
    suite: Arc<TestSuite>,
    suite_semaphore: Arc<Semaphore>,
    scenario_idx: usize,
    viewport: Option<config::viewports::ViewportDef>,
    index: usize,
}

/// Flatten every scenario/viewport pair across all parsed suites into a flat list
/// of independent capture jobs, assigning each a 1-based dedicated index. The
/// total number of jobs is simply the returned vec's length.
fn build_capture_jobs(
    parsed_suites: &[(TestSuiteInput, Arc<TestSuite>)],
    workspace_config: &config::WorkspaceConfig,
) -> Result<Vec<CaptureJob>> {
    let mut jobs = Vec::new();
    for (_input, suite) in parsed_suites {
        let suite_semaphore = Arc::new(Semaphore::new(config::resolve_async_capture_limit(
            workspace_config,
            suite,
        )));
        for scenario_idx in 0..suite.scenarios.len() {
            let viewports = scenario_capture_viewports(
                workspace_config,
                suite,
                &suite.scenarios[scenario_idx],
            )?;
            for viewport in viewports {
                jobs.push(CaptureJob {
                    suite: suite.clone(),
                    suite_semaphore: suite_semaphore.clone(),
                    scenario_idx,
                    viewport,
                    index: 0,
                });
            }
        }
    }
    for (i, job) in jobs.iter_mut().enumerate() {
        job.index = i + 1;
    }
    Ok(jobs)
}

/// Spawn one task per capture job. Each task acquires a concurrency permit,
/// launches its own browser, and runs exactly one scenario/viewport capture with
/// its dedicated index. A failure in one capture only fails that task; siblings
/// (including other viewports of the same scenario) run independently.
fn spawn_capture_jobs(
    jobs: Vec<CaptureJob>,
    semaphore: &Arc<Semaphore>,
    exe_path: &Arc<Option<PathBuf>>,
    workspace_config: &Arc<config::WorkspaceConfig>,
    active_replacements: &Arc<Vec<UrlReplacement>>,
    apply_url_replacements: bool,
) -> Vec<JoinHandle<TaskStatus>> {
    let total = jobs.len();
    let mut handles = Vec::with_capacity(total);

    for job in jobs {
        let CaptureJob {
            suite,
            suite_semaphore,
            scenario_idx,
            viewport,
            index,
        } = job;
        let sem = semaphore.clone();
        let suite_sem = suite_semaphore.clone();
        let exe_path = exe_path.clone();
        let ws_config = workspace_config.clone();
        let replacements = active_replacements.clone();

        let handle = tokio::spawn(async move {
            let Ok((_suite_permit, _global_permit)) = acquire_capture_permits(suite_sem, sem).await
            else {
                eprintln!("  {} Capture semaphore closed", "ERROR:".red());
                return TaskStatus::InfraFailed;
            };
            let log_prefix = make_log_prefix(index, total);
            let label = suite.scenarios[scenario_idx]
                .label
                .as_deref()
                .or(suite.scenarios[scenario_idx].id.as_deref())
                .unwrap_or("unknown");
            match exec_single_scenario(
                &exe_path,
                ScenarioRequest {
                    test_suite: &suite,
                    scenario: &suite.scenarios[scenario_idx],
                    all_scenarios: &suite.scenarios,
                    workspace_config: &ws_config,
                    replacement_profile: &replacements,
                    apply_url_replacements,
                    viewport: viewport.as_ref(),
                    index,
                    total,
                },
            )
            .await
            {
                ScenarioOutcome::Passed => TaskStatus::Passed,
                ScenarioOutcome::PageFailed(e) => {
                    // Page errors (timeout, not found, ...) are expected for a
                    // visual regression tool — log and continue, the result is
                    // reflected in the comparison report.
                    eprintln!(
                        "{} {} Capture '{}' failed: {:?}",
                        log_prefix,
                        "ERROR:".red(),
                        label,
                        e
                    );
                    TaskStatus::PageFailed
                }
                ScenarioOutcome::InfraFailed(e) => {
                    eprintln!(
                        "{} {} Capture '{}' failed (infrastructure): {:?}",
                        log_prefix,
                        "ERROR:".red(),
                        label,
                        e
                    );
                    TaskStatus::InfraFailed
                }
            }
        });

        handles.push(handle);
    }

    handles
}

/// Acquire the suite limit before the global limit. This prevents a job that
/// already exceeds its suite cap from occupying a global capture slot.
async fn acquire_capture_permits(
    suite_semaphore: Arc<Semaphore>,
    global_semaphore: Arc<Semaphore>,
) -> Result<(OwnedSemaphorePermit, OwnedSemaphorePermit)> {
    let suite_permit = suite_semaphore
        .acquire_owned()
        .await
        .map_err(|_| anyhow!("Suite capture semaphore closed"))?;
    let global_permit = global_semaphore
        .acquire_owned()
        .await
        .map_err(|_| anyhow!("Global capture semaphore closed"))?;
    Ok((suite_permit, global_permit))
}

/// Discover available test suite files in the visual_tests/ directory.
fn discover_test_suites(visual_tests_dir: &std::path::Path) -> Vec<TestSuiteInput> {
    let mut suites: HashMap<String, (u8, TestSuiteInput)> = HashMap::new();
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

            let (format, priority) = match ext {
                "yaml" => (TestSuiteFormat::Yaml, 0),
                "yml" => (TestSuiteFormat::Yaml, 1),
                "json" => (TestSuiteFormat::Json, 2),
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

            let input = TestSuiteInput {
                name: suite_name.to_string(),
                path: entry.path().to_path_buf(),
                format,
            };

            match suites.get_mut(suite_name) {
                Some((current_priority, current_input)) if priority < *current_priority => {
                    *current_priority = priority;
                    *current_input = input;
                }
                Some(_) => {}
                None => {
                    suites.insert(suite_name.to_string(), (priority, input));
                }
            }
        }
    }
    let mut suites: Vec<_> = suites.into_values().map(|(_, input)| input).collect();
    suites.sort_by(|a, b| a.name.cmp(&b.name));
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
    root_dir: &std::path::Path,
    cli_profile: Option<&str>,
) -> Result<Vec<UrlReplacement>> {
    let profile_name = replacements::get_profile_name(cli_profile);
    let explicit_profile = cli_profile.is_some()
        || std::env::var("REPLACEMENT_PROFILE")
            .map(|value| !value.is_empty())
            .unwrap_or(false);

    let Some(profiles) = replacements::load_replacement_profiles(root_dir)? else {
        if explicit_profile {
            anyhow::bail!(
                "Replacement profile file not found: {}",
                root_dir.join("common/_replacement-profiles.yaml").display()
            );
        }
        return Ok(Vec::new());
    };

    let active = match profiles.profiles.get(&profile_name) {
        Some(profile) => profile.clone(),
        None if explicit_profile => {
            anyhow::bail!("Replacement profile '{}' is not defined", profile_name)
        }
        None => Vec::new(),
    };

    replacements::validate_replacements(
        &active,
        &format!("replacement profile '{}'", profile_name),
    )?;
    Ok(active)
}

fn validate_workspace_config(workspace_config: &config::WorkspaceConfig) -> Result<()> {
    if workspace_config.async_capture_limit == Some(0) {
        anyhow::bail!("workspace asyncCaptureLimit must be greater than zero");
    }
    if workspace_config.async_compare_limit == Some(0) {
        anyhow::bail!("workspace asyncCompareLimit must be greater than zero");
    }
    if let Some(browser) = &workspace_config.browser
        && !matches!(browser, TestBrowser::Chromium)
    {
        anyhow::bail!(
            "Unsupported browser in workspace config: only 'chromium' is currently supported"
        );
    }
    if let Some(replacements) = &workspace_config.url_replacements {
        replacements::validate_replacements(replacements, "workspace urlReplacements")?;
    }
    Ok(())
}

fn validate_test_suite_config(input: &TestSuiteInput, suite: &TestSuite) -> Result<()> {
    if suite.async_capture_limit == Some(0) {
        anyhow::bail!(
            "Test suite '{}' asyncCaptureLimit must be greater than zero",
            input.name
        );
    }
    if suite.async_compare_limit == Some(0) {
        anyhow::bail!(
            "Test suite '{}' asyncCompareLimit must be greater than zero",
            input.name
        );
    }
    if let Some(browser) = &suite.browser
        && !matches!(browser, TestBrowser::Chromium)
    {
        anyhow::bail!(
            "Unsupported browser in test suite '{}': only 'chromium' is currently supported",
            input.name
        );
    }
    if let Some(replacements) = &suite.url_replacements {
        replacements::validate_replacements(
            replacements,
            &format!("test suite '{}' urlReplacements", input.name),
        )?;
    }
    for (index, scenario) in suite.scenarios.iter().enumerate() {
        capture::final_screenshot_types(scenario).map_err(|error| {
            anyhow!(
                "Invalid selectors for scenario '{}': {}",
                scenario_identifier(scenario, &format!("<scenario:{}>", index + 1)),
                error
            )
        })?;
    }
    validate_scenario_dependencies(input, suite)?;
    Ok(())
}

fn normalize_scenario_id(id: &str) -> String {
    id.trim().to_ascii_lowercase()
}

fn scenario_identifier(scenario: &TestScenario, fallback: &str) -> String {
    scenario
        .id
        .as_deref()
        .or(scenario.label.as_deref())
        .unwrap_or(fallback)
        .to_string()
}

fn needs_ids(needs: &Needs) -> Vec<&str> {
    match needs {
        Needs::String(id) => vec![id.as_str()],
        Needs::StringArray(ids) => ids.iter().map(String::as_str).collect(),
    }
}

fn validate_dependency_chain(
    input: &TestSuiteInput,
    scenarios: &[TestScenario],
    id_map: &HashMap<String, Vec<usize>>,
    scenario_idx: usize,
    level: usize,
    trail: &mut Vec<String>,
    active_ids: &mut Vec<String>,
) -> Result<()> {
    if level >= 100 {
        anyhow::bail!(
            "Scenario dependency depth exceeded 100 in test suite '{}': {}",
            input.name,
            trail.join(" -> ")
        );
    }

    let Some(needs) = scenarios[scenario_idx].needs.as_ref() else {
        return Ok(());
    };

    for needed_id in needs_ids(needs) {
        let normalized_need = normalize_scenario_id(needed_id);
        if active_ids.contains(&normalized_need) {
            let mut cycle = trail.clone();
            cycle.push(needed_id.to_string());
            anyhow::bail!(
                "Circular scenario dependency detected in test suite '{}': {}",
                input.name,
                cycle.join(" -> ")
            );
        }

        let matching_indices = id_map.get(&normalized_need).cloned().unwrap_or_default();
        if matching_indices.len() != 1 {
            anyhow::bail!(
                "The test suite '{}' must contain exactly one scenario with id: {}",
                input.name,
                needed_id
            );
        }

        let target_idx = matching_indices[0];
        active_ids.push(normalized_need);
        trail.push(scenario_identifier(&scenarios[target_idx], needed_id));
        validate_dependency_chain(
            input,
            scenarios,
            id_map,
            target_idx,
            level + 1,
            trail,
            active_ids,
        )?;
        trail.pop();
        active_ids.pop();
    }

    Ok(())
}

fn validate_scenario_dependencies(input: &TestSuiteInput, suite: &TestSuite) -> Result<()> {
    let mut id_map: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, scenario) in suite.scenarios.iter().enumerate() {
        if let Some(id) = scenario.id.as_deref() {
            id_map
                .entry(normalize_scenario_id(id))
                .or_default()
                .push(index);
        }
    }

    if let Some((id, _)) = id_map.iter().find(|(_, indices)| indices.len() > 1) {
        anyhow::bail!(
            "Duplicate scenario ID found in test suite '{}': '{}'. Scenario IDs must be unique.",
            input.name,
            id
        );
    }

    for (index, scenario) in suite.scenarios.iter().enumerate() {
        let start = scenario_identifier(scenario, &format!("<scenario:{}>", index + 1));
        let mut trail = vec![start];
        let mut active_ids = scenario
            .id
            .as_deref()
            .map(|id| vec![normalize_scenario_id(id)])
            .unwrap_or_default();
        validate_dependency_chain(
            input,
            &suite.scenarios,
            &id_map,
            index,
            0,
            &mut trail,
            &mut active_ids,
        )?;
    }

    Ok(())
}

fn parse_test_suites(
    visual_tests_dir: &std::path::Path,
    inputs: Vec<TestSuiteInput>,
) -> Result<Vec<(TestSuiteInput, Arc<TestSuite>)>> {
    let mut parsed = Vec::new();
    for input in inputs {
        let suite = parse_test_suite(visual_tests_dir, &input)
            .map_err(|e| anyhow!("Error parsing test suite '{}': {:?}", input.name, e))?;
        validate_test_suite_config(&input, &suite)?;
        parsed.push((input, Arc::new(suite)));
    }
    if parsed.is_empty() {
        anyhow::bail!("No matching test suites found");
    }
    Ok(parsed)
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
    validate_workspace_config(&workspace_config)?;
    let active_replacements =
        load_active_replacement_profile(root_dir, replacement_profile.as_deref())?;

    let available = discover_test_suites(&visual_tests_dir);
    let filtered = filter_test_suites(available, &test_suites);

    // Parse all test suites upfront. YAML/config errors are infrastructure
    // errors and must fail before scenarios start.
    let parsed_suites = parse_test_suites(&visual_tests_dir, filtered)?;

    let workspace_config = Arc::new(workspace_config);

    // Write the per-suite report ordering manifest so the comparison report lists
    // screenshots in scenario-then-viewport order (matching the TS/BackstopJS
    // behaviour) instead of alphabetically by filename.
    for (input, suite) in &parsed_suites {
        let mut order: Vec<String> = Vec::new();
        let mut thresholds: Vec<(String, f64)> = Vec::new();
        for scenario in &suite.scenarios {
            let order_keys =
                scenario_capture_order_keys(workspace_config.as_ref(), suite, scenario)?;
            let resolved = config::resolve_config(workspace_config.as_ref(), suite, scenario);
            for order_key in &order_keys {
                thresholds.push((order_key.clone(), resolved.mis_match_threshold));
            }
            order.extend(order_keys);
        }
        capture::write_report_order(suite, &order).map_err(|e| {
            anyhow!(
                "Failed to write report ordering manifest for '{}': {:?}",
                input.name,
                e
            )
        })?;
        capture::write_report_thresholds(suite, &thresholds).map_err(|e| {
            anyhow!(
                "Failed to write report threshold manifest for '{}': {:?}",
                input.name,
                e
            )
        })?;
    }

    // Flatten every scenario/viewport pair into an independent capture job (own
    // browser, own dedicated index) and spawn them — matching the TS/BackstopJS
    // model where pairs are scaffolded upfront and run independently.
    let jobs = build_capture_jobs(&parsed_suites, workspace_config.as_ref())?;

    let exe_path = if fetch_browser.unwrap_or(false) {
        Some(
            fetcher()
                .await
                .map_err(|e| anyhow!("Failed to fetch the browser: {:?}", e))?,
        )
    } else {
        None
    };

    // Run all scenarios across all suites with a global concurrency limit.
    let limit = common::ASYNC_CAPTURE_LIMIT.get().copied().unwrap_or(5);
    let semaphore = create_ramped_semaphore(limit);
    let exe_path = Arc::new(exe_path);
    let active_replacements = Arc::new(active_replacements);
    let handles = spawn_capture_jobs(
        jobs,
        &semaphore,
        &exe_path,
        &workspace_config,
        &active_replacements,
        true,
    );

    // Wait for all scenarios to complete. Only infrastructure failures (browser
    // launch) fail the command; page-level failures are reported but tolerated.
    let mut page_failures = 0usize;
    let mut infra_failures = 0usize;
    for handle in handles {
        match handle.await {
            Ok(TaskStatus::Passed) => {}
            Ok(TaskStatus::PageFailed) => page_failures += 1,
            Ok(TaskStatus::InfraFailed) => infra_failures += 1,
            Err(e) => {
                // A panicked task is a bug in our code, not the page under test.
                eprintln!("Task panicked: {:?}", e);
                infra_failures += 1;
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
            let async_compare_limit =
                config::resolve_async_compare_limit(workspace_config.as_ref(), suite);

            eprintln!("Running visual comparison for '{}'...", input.name);
            if let Err(e) = generate_comparison_report(
                &suite_dir,
                suite_id,
                test_date,
                threshold,
                async_compare_limit,
            ) {
                eprintln!("Visual comparison failed for '{}': {:?}", input.name, e);
                comparison_failures += 1;
            }
        }
        eprintln!("Test suite '{}' completed.", input.name);
    }

    generate_snapshot_reports()?;

    if page_failures > 0 {
        eprintln!(
            "{} scenario(s) failed due to page errors (e.g. timeout, not found). \
             These are reported in the comparison and do not fail the run.",
            page_failures.to_string().yellow()
        );
    }
    if comparison_failures > 0 {
        eprintln!(
            "{} suite comparison(s) could not be generated.",
            comparison_failures.to_string().yellow()
        );
    }

    // Only infrastructure failures (browser could not be launched) fail the
    // command. Page errors and visual differences are expected results of a
    // regression run and must not produce a non-zero exit.
    if infra_failures > 0 {
        anyhow::bail!(
            "Test run failed: {} scenario(s) had browser or artifact failures",
            infra_failures
        );
    }

    if comparison_failures > 0 {
        anyhow::bail!(
            "Test run failed: {} suite comparison report(s) could not be generated",
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
    validate_workspace_config(&workspace_config)?;

    let available = discover_test_suites(&visual_tests_dir);
    let filtered = filter_test_suites(available, &test_suites);

    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    // Parse all test suites upfront. YAML/config errors are infrastructure
    // errors and must fail before scenarios start.
    let parsed_suites = parse_test_suites(&visual_tests_dir, filtered)?;

    let workspace_config = Arc::new(workspace_config);

    for (input, suite) in &parsed_suites {
        let mut order: Vec<String> = Vec::new();
        let mut thresholds: Vec<(String, f64)> = Vec::new();
        for scenario in &suite.scenarios {
            let order_keys =
                scenario_capture_order_keys(workspace_config.as_ref(), suite, scenario)?;
            let resolved = config::resolve_config(workspace_config.as_ref(), suite, scenario);
            for order_key in &order_keys {
                thresholds.push((order_key.clone(), resolved.mis_match_threshold));
            }
            order.extend(order_keys);
        }
        capture::write_report_order(suite, &order).map_err(|e| {
            anyhow!(
                "Failed to write report ordering manifest for '{}': {:?}",
                input.name,
                e
            )
        })?;
        capture::write_report_thresholds(suite, &thresholds).map_err(|e| {
            anyhow!(
                "Failed to write report threshold manifest for '{}': {:?}",
                input.name,
                e
            )
        })?;
    }

    let jobs = build_capture_jobs(&parsed_suites, workspace_config.as_ref())?;

    let exe_path = if fetch_browser.unwrap_or(false) {
        Some(
            fetcher()
                .await
                .map_err(|e| anyhow!("Failed to fetch the browser: {:?}", e))?,
        )
    } else {
        None
    };

    // Run all scenarios across all suites with a global concurrency limit
    let limit = common::ASYNC_CAPTURE_LIMIT.get().copied().unwrap_or(5);
    let semaphore = create_ramped_semaphore(limit);
    let exe_path = Arc::new(exe_path);
    let active_replacements = Arc::new(Vec::<UrlReplacement>::new());

    // Flatten every scenario/viewport pair into an independent capture job (own
    // browser, own dedicated index) and spawn them — matching the TS/BackstopJS
    // model where pairs are scaffolded upfront and run independently.
    let handles = spawn_capture_jobs(
        jobs,
        &semaphore,
        &exe_path,
        &workspace_config,
        &active_replacements,
        false,
    );

    // Wait for all scenarios to complete. Only infrastructure failures (browser
    // launch) fail the command; page-level failures are reported but tolerated.
    let mut page_failures = 0usize;
    let mut infra_failures = 0usize;
    for handle in handles {
        match handle.await {
            Ok(TaskStatus::Passed) => {}
            Ok(TaskStatus::PageFailed) => page_failures += 1,
            Ok(TaskStatus::InfraFailed) => infra_failures += 1,
            Err(e) => {
                // A panicked task is a bug in our code, not the page under test.
                eprintln!("Task panicked: {:?}", e);
                infra_failures += 1;
            }
        }
    }

    if page_failures > 0 {
        eprintln!(
            "{} scenario(s) failed due to page errors (e.g. timeout, not found). \
             No reference screenshot was captured for them; the run continues.",
            page_failures.to_string().yellow()
        );
    }

    // Only infrastructure failures (browser could not be launched) fail the
    // command. Page errors must not produce a non-zero exit.
    if infra_failures > 0 {
        anyhow::bail!(
            "Reference capture failed: {} scenario(s) had browser or artifact failures",
            infra_failures
        );
    }

    // Promote screenshots and run comparison per suite
    for (input, suite) in &parsed_suites {
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
        let mut skipped_failed_count = 0;
        for entry in fs::read_dir(&latest_test_dir)?.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.ends_with(".png") && !file_name.ends_with("_diff.png") {
                let src = entry.path();
                let marker_src = crate::test::capture::error_marker_path(&src);
                if marker_src.exists() {
                    skipped_failed_count += 1;
                    eprintln!(
                        "    {} Skipping failed capture: {}",
                        "SKIP:".yellow(),
                        file_name
                    );
                    continue;
                }

                let dst = ref_dest.join(&file_name);
                copy_file_atomically(&src, &dst)?;

                let marker_dst = crate::test::capture::error_marker_path(&dst);
                if marker_dst.exists() {
                    fs::remove_file(marker_dst)?;
                }

                count += 1;
            }
        }

        eprintln!(
            "Captured {} reference screenshot(s) for '{}' (run: {}).",
            count.to_string().green(),
            input.name.cyan(),
            latest_run
        );
        if skipped_failed_count > 0 {
            eprintln!(
                "Skipped {} failed capture placeholder(s) for '{}'. They remain report-only and were not promoted to references.",
                skipped_failed_count.to_string().yellow(),
                input.name.cyan()
            );
        }

        // Run comparison so results show all scenarios as passed. A failing
        // comparison here only means a scenario hit a page error (and produced
        // an error placeholder); that must not fail the `ref` command, which is
        // about capturing baselines, not asserting them.
        eprintln!("Running visual comparison...");
        let async_compare_limit =
            config::resolve_async_compare_limit(workspace_config.as_ref(), suite);
        if let Err(e) = generate_comparison_report(
            &suite_dir,
            &input.name,
            &latest_run,
            0.0,
            async_compare_limit,
        ) {
            eprintln!(
                "{} Visual comparison for '{}' reported failures: {:?}",
                "WARNING:".yellow(),
                input.name,
                e
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> TestSuiteInput {
        TestSuiteInput {
            name: "alloy".to_string(),
            path: std::path::PathBuf::from("alloy.tests.yaml"),
            format: TestSuiteFormat::Yaml,
        }
    }

    fn suite(scenarios: Vec<TestScenario>) -> TestSuite {
        TestSuite {
            id: Some("alloy".to_string()),
            test_date: None,
            async_capture_limit: None,
            async_compare_limit: None,
            browser: None,
            bypass_csp: None,
            cookie_path: None,
            css_override_path: None,
            debug: None,
            hide_selectors: None,
            ignore_ssl_errors: None,
            js_on_ready_path: None,
            mis_match_threshold: None,
            post_interaction_wait: None,
            remove_selectors: None,
            scenarios,
            state: None,
            url_replacements: None,
            use_css_override: None,
            viewport_names: None,
            viewports_path: None,
        }
    }

    fn scenario(id: &str, needs: Option<Needs>) -> TestScenario {
        TestScenario {
            actions: None,
            bypass_csp: None,
            click_selector: None,
            cookie_path: None,
            css_override_path: None,
            delay: None,
            description: None,
            hide_selectors: None,
            id: Some(id.to_string()),
            js_on_ready_path: None,
            label: None,
            mis_match_threshold: None,
            needs,
            no_scroll_top: None,
            post_interaction_wait: None,
            remove_selectors: None,
            required_login: None,
            restore: None,
            selectors: None,
            url: format!("https://example.com/{id}"),
            use_css_override: None,
            viewport_names: None,
        }
    }

    #[test]
    fn validates_existing_dependencies() {
        let suite = suite(vec![
            scenario("base", None),
            scenario("page", Some(Needs::String("base".to_string()))),
        ]);

        validate_scenario_dependencies(&input(), &suite).unwrap();
    }

    #[test]
    fn rejects_missing_dependencies() {
        let suite = suite(vec![scenario(
            "page",
            Some(Needs::String("missing".to_string())),
        )]);

        let error = validate_scenario_dependencies(&input(), &suite).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("exactly one scenario with id: missing")
        );
    }

    #[test]
    fn rejects_circular_dependencies_with_path() {
        let suite = suite(vec![
            scenario("alpha", Some(Needs::String("beta".to_string()))),
            scenario("beta", Some(Needs::String("alpha".to_string()))),
        ]);

        let error = validate_scenario_dependencies(&input(), &suite).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Circular scenario dependency"));
        assert!(message.contains("alpha -> beta -> alpha"));
    }

    #[test]
    fn discover_test_suites_prefers_yaml_then_yml_then_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("alloy.tests.json"), "{}").unwrap();
        std::fs::write(temp_dir.path().join("alloy.tests.yml"), "scenarios: []\n").unwrap();
        std::fs::write(temp_dir.path().join("alloy.tests.yaml"), "scenarios: []\n").unwrap();

        let suites = discover_test_suites(temp_dir.path());

        assert_eq!(suites.len(), 1);
        assert_eq!(suites[0].name, "alloy");
        assert_eq!(
            suites[0].path.file_name().and_then(|name| name.to_str()),
            Some("alloy.tests.yaml")
        );
    }

    #[test]
    fn discover_test_suites_falls_back_to_yml_before_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("alloy.tests.json"), "{}").unwrap();
        std::fs::write(temp_dir.path().join("alloy.tests.yml"), "scenarios: []\n").unwrap();

        let suites = discover_test_suites(temp_dir.path());

        assert_eq!(suites.len(), 1);
        assert_eq!(
            suites[0].path.file_name().and_then(|name| name.to_str()),
            Some("alloy.tests.yml")
        );
    }

    #[test]
    fn rejects_zero_async_compare_limit_before_capture() {
        let workspace = config::WorkspaceConfig {
            async_compare_limit: Some(0),
            ..Default::default()
        };
        assert!(
            validate_workspace_config(&workspace)
                .unwrap_err()
                .to_string()
                .contains("asyncCompareLimit")
        );

        let mut suite = suite(vec![scenario("home", None)]);
        suite.async_compare_limit = Some(0);
        assert!(
            validate_test_suite_config(&input(), &suite)
                .unwrap_err()
                .to_string()
                .contains("asyncCompareLimit")
        );
    }

    #[test]
    fn rejects_zero_async_capture_limit_before_browser_launch() {
        let workspace = config::WorkspaceConfig {
            async_capture_limit: Some(0),
            ..Default::default()
        };
        assert!(
            validate_workspace_config(&workspace)
                .unwrap_err()
                .to_string()
                .contains("asyncCaptureLimit")
        );

        let mut suite = suite(vec![scenario("home", None)]);
        suite.async_capture_limit = Some(0);
        assert!(
            validate_test_suite_config(&input(), &suite)
                .unwrap_err()
                .to_string()
                .contains("asyncCaptureLimit")
        );
    }

    #[tokio::test]
    async fn suite_capture_limit_blocks_only_its_own_jobs() {
        let global = Arc::new(Semaphore::new(2));
        let suite_a = Arc::new(Semaphore::new(1));
        let suite_b = Arc::new(Semaphore::new(1));
        let first_a = acquire_capture_permits(suite_a.clone(), global.clone())
            .await
            .unwrap();

        assert!(
            tokio::time::timeout(
                Duration::from_millis(1),
                acquire_capture_permits(suite_a, global.clone()),
            )
            .await
            .is_err()
        );
        assert!(
            tokio::time::timeout(
                Duration::from_millis(1),
                acquire_capture_permits(suite_b, global),
            )
            .await
            .is_ok()
        );

        drop(first_a);
    }

    #[test]
    fn rejects_empty_selector_capture_lists_before_browser_launch() {
        let mut suite = suite(vec![scenario("home", None)]);
        suite.scenarios[0].selectors = Some(crate::types::Selectors::StringArray(Vec::new()));

        let error = validate_test_suite_config(&input(), &suite).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Invalid selectors for scenario 'home'")
        );
        assert!(error.to_string().contains("at least one capture target"));
    }
}
