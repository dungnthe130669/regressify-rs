use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
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

const MAX_OUTPUT_IDENTIFIER_BYTES: usize = 80;

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

fn suite_output_dir(regressify_dir: &Path, suite: &TestSuite) -> Result<PathBuf> {
    let suite_id = suite
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("Test suite ID is required for output paths"))?;
    Ok(regressify_dir.join(suite_id))
}

fn current_capture_run_dir(tests_dir: &Path, suite: &TestSuite) -> Result<PathBuf> {
    let run_id = suite
        .test_date
        .as_deref()
        .ok_or_else(|| anyhow!("Test suite is missing a capture run ID"))?;
    let run_dir = tests_dir.join(run_id);
    if !run_dir.is_dir() {
        anyhow::bail!(
            "Current capture run directory does not exist: {}",
            run_dir.display()
        );
    }
    Ok(run_dir)
}

fn ensure_capture_run_directories_are_new(
    output_dir: &Path,
    suites: &[(TestSuiteInput, Arc<TestSuite>)],
) -> Result<()> {
    for (input, suite) in suites {
        let suite_dir = suite_output_dir(output_dir, suite)?;
        let run_id = suite
            .test_date
            .as_deref()
            .ok_or_else(|| anyhow!("Test suite '{}' is missing a capture run ID", input.name))?;
        let run_dir = suite_dir.join("bitmaps_test").join(run_id);
        if run_dir.exists() {
            anyhow::bail!(
                "Capture run directory already exists for test suite '{}': {}. Use a new --run-id or remove the stale run before capturing.",
                input.name,
                run_dir.display()
            );
        }
    }
    Ok(())
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
fn discover_test_suites(visual_tests_dir: &std::path::Path) -> Result<Vec<TestSuiteInput>> {
    let mut suites: HashMap<String, (u8, TestSuiteInput)> = HashMap::new();
    for entry in WalkDir::new(visual_tests_dir) {
        let entry = entry.map_err(|error| {
            anyhow!(
                "Failed to discover test suites under '{}': {}",
                visual_tests_dir.display(),
                error
            )
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
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
    Ok(suites)
}

/// Filter test suites by glob patterns (or return all if patterns is empty / contains "all" / "*").
fn filter_test_suites(
    available: Vec<TestSuiteInput>,
    patterns: &[String],
) -> Result<Vec<TestSuiteInput>> {
    if patterns.is_empty()
        || patterns.contains(&"all".to_string())
        || patterns.contains(&"*".to_string())
    {
        Ok(available)
    } else {
        let patterns: Vec<_> = patterns
            .iter()
            .map(|pattern| {
                glob::Pattern::new(pattern)
                    .map_err(|error| anyhow!("Invalid test suite pattern '{}': {}", pattern, error))
            })
            .collect::<Result<_>>()?;
        Ok(available
            .into_iter()
            .filter(|ts| patterns.iter().any(|pattern| pattern.matches(&ts.name)))
            .collect())
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
    validate_percentage(
        "workspace misMatchThreshold",
        workspace_config.mis_match_threshold,
    )?;
    validate_duration("workspace delay", workspace_config.delay)?;
    validate_duration(
        "workspace postInteractionWait",
        workspace_config.post_interaction_wait,
    )?;
    for (name, value) in [
        ("elementTimeout", workspace_config.element_timeout),
        (
            "actionabilityTimeout",
            workspace_config.actionability_timeout,
        ),
        ("scenarioTimeout", workspace_config.scenario_timeout),
    ] {
        if value == Some(0) {
            anyhow::bail!("workspace {} must be greater than zero", name);
        }
    }
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

fn validate_percentage(name: &str, value: Option<f64>) -> Result<()> {
    if let Some(value) = value
        && (!value.is_finite() || !(0.0..=100.0).contains(&value))
    {
        anyhow::bail!("{} must be a finite value between 0 and 100", name);
    }
    Ok(())
}

fn validate_duration(name: &str, value: Option<f64>) -> Result<()> {
    if let Some(value) = value
        && (!value.is_finite() || value < 0.0)
    {
        anyhow::bail!(
            "{} must be a finite value greater than or equal to zero",
            name
        );
    }
    Ok(())
}

fn validate_output_identifier(name: &str, value: Option<&str>) -> Result<()> {
    let value = value.ok_or_else(|| anyhow!("{} is required", name))?;
    if value.trim().is_empty()
        || value == "."
        || value == ".."
        || value.len() > MAX_OUTPUT_IDENTIFIER_BYTES
        || value.contains(['/', '\\', '\0', '<', '>', ':', '"', '|', '?', '*'])
    {
        anyhow::bail!(
            "{} '{}' is not a portable output path component no longer than {} bytes",
            name,
            value,
            MAX_OUTPUT_IDENTIFIER_BYTES
        );
    }
    Ok(())
}

fn validate_test_suite_config(input: &TestSuiteInput, suite: &TestSuite) -> Result<()> {
    validate_output_identifier("Test suite ID", suite.id.as_deref())?;
    validate_percentage(
        &format!("test suite '{}' misMatchThreshold", input.name),
        suite.mis_match_threshold,
    )?;
    validate_duration(
        &format!("test suite '{}' postInteractionWait", input.name),
        suite.post_interaction_wait,
    )?;
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
        let scenario_name = scenario_identifier(scenario, &format!("<scenario:{}>", index + 1));
        validate_output_identifier("Scenario ID", scenario.id.as_deref())?;
        validate_percentage(
            &format!("scenario '{}' misMatchThreshold", scenario_name),
            scenario.mis_match_threshold,
        )?;
        validate_duration(
            &format!("scenario '{}' delay", scenario_name),
            scenario.delay,
        )?;
        validate_duration(
            &format!("scenario '{}' postInteractionWait", scenario_name),
            scenario.post_interaction_wait,
        )?;
        capture::validate_scenario_capture_names(scenario).map_err(|error| {
            anyhow!(
                "Invalid selectors for scenario '{}': {}",
                scenario_name,
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
    validate_unique_suite_output_ids(&parsed)?;
    Ok(parsed)
}

fn validate_external_capture_files(
    root_dir: &Path,
    workspace_config: &config::WorkspaceConfig,
    parsed_suites: &[(TestSuiteInput, Arc<TestSuite>)],
) -> Result<()> {
    let output_dir = common::REGRESSIFY_DIR.get_required()?;
    validate_external_capture_files_in(root_dir, output_dir, workspace_config, parsed_suites)
}

fn validate_external_capture_files_in(
    root_dir: &Path,
    output_dir: &Path,
    workspace_config: &config::WorkspaceConfig,
    parsed_suites: &[(TestSuiteInput, Arc<TestSuite>)],
) -> Result<()> {
    let mut checked_cookie_files = HashSet::new();
    let mut checked_state_files = HashSet::new();

    for (input, suite) in parsed_suites {
        for (index, scenario) in suite.scenarios.iter().enumerate() {
            let resolved = config::resolve_config(workspace_config, suite, scenario);
            let scenario_name = scenario_identifier(scenario, &format!("<scenario:{}>", index + 1));
            let context = format!("test suite '{}' scenario '{}'", input.name, scenario_name);

            if let Some(cookie_path) = resolved.cookie_path.as_deref() {
                let path = config::resolve_workspace_file(root_dir, cookie_path, "cookiePath")
                    .with_context(|| format!("Invalid cookiePath for {}", context))?;
                if checked_cookie_files.insert(path.clone()) {
                    let content = std::fs::read_to_string(&path).with_context(|| {
                        format!("Failed to read cookiePath '{}'", path.display())
                    })?;
                    let cookies: Vec<serde_json::Value> = serde_yaml::from_str(&content)
                        .with_context(|| format!("Invalid cookie YAML '{}'", path.display()))?;
                    for (index, cookie) in cookies.iter().enumerate() {
                        crate::test::capture::validate_cookie_value(
                            cookie,
                            &format!("cookiePath '{}' cookie {}", path.display(), index),
                        )
                        .with_context(|| format!("Invalid cookie YAML '{}'", path.display()))?;
                    }
                }
            }

            if let Some(state) = resolved.state.as_ref() {
                let state_names = match state {
                    Needs::String(name) => vec![name.as_str()],
                    Needs::StringArray(names) => names.iter().map(String::as_str).collect(),
                };
                for state_name in state_names {
                    crate::test::capture::validate_state_name(state_name)?;
                    let state_path = output_dir
                        .join("states")
                        .join(format!("{}.json", state_name));
                    if checked_state_files.insert(state_path.clone()) {
                        crate::test::capture::read_persisted_state_file(&state_path, state_name)
                            .with_context(|| format!("Invalid persisted state for {}", context))?;
                    }
                }
            }

            if resolved.use_css_override
                && let Some(css_path) = resolved.css_override_path.as_deref()
            {
                config::resolve_workspace_file(root_dir, css_path, "cssOverridePath")
                    .with_context(|| format!("Invalid cssOverridePath for {}", context))?;
            }

            if let Some(script_path) = resolved.js_on_ready_path.as_deref() {
                config::resolve_workspace_file(root_dir, script_path, "jsOnReadyPath")
                    .with_context(|| format!("Invalid jsOnReadyPath for {}", context))?;
            }
        }
    }
    Ok(())
}

fn validate_unique_suite_output_ids(suites: &[(TestSuiteInput, Arc<TestSuite>)]) -> Result<()> {
    let mut inputs_by_output_id = HashMap::new();
    for (input, suite) in suites {
        let suite_id = suite
            .id
            .as_deref()
            .ok_or_else(|| anyhow!("Test suite '{}' is missing an ID", input.name))?;
        let key = suite_id.to_lowercase();
        if let Some(previous_input) = inputs_by_output_id.insert(key, input.name.as_str()) {
            anyhow::bail!(
                "Test suites '{}' and '{}' share output ID '{}' on case-insensitive filesystems",
                previous_input,
                input.name,
                suite_id
            );
        }
    }
    Ok(())
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

    let available = discover_test_suites(&visual_tests_dir)?;
    let filtered = filter_test_suites(available, &test_suites)?;

    // Parse all test suites upfront. YAML/config errors are infrastructure
    // errors and must fail before scenarios start.
    let parsed_suites = parse_test_suites(&visual_tests_dir, filtered)?;
    validate_external_capture_files(root_dir, &workspace_config, &parsed_suites)?;
    ensure_capture_run_directories_are_new(common::REGRESSIFY_DIR.get_required()?, &parsed_suites)?;

    let workspace_config = Arc::new(workspace_config);

    // Write the per-suite report ordering manifest so the comparison report lists
    // screenshots in scenario-then-viewport order (matching the TS/BackstopJS
    // behaviour) instead of alphabetically by filename.
    for (input, suite) in &parsed_suites {
        let mut order: Vec<String> = Vec::new();
        let mut thresholds: Vec<(String, f64)> = Vec::new();
        for scenario in &suite.scenarios {
            let order_keys = scenario_capture_order_keys(
                workspace_config.as_ref(),
                suite,
                scenario,
                &suite.scenarios,
            )?;
            let resolved = config::resolve_config(workspace_config.as_ref(), suite, scenario);
            for order_key in &order_keys {
                thresholds.push((order_key.clone(), resolved.mis_match_threshold));
                if let Some(prefix) = order_key.split("__").next() {
                    thresholds.push((prefix.to_string(), resolved.mis_match_threshold));
                }
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
            let suite_dir = suite_output_dir(regressify_dir, suite)?;

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

    let available = discover_test_suites(&visual_tests_dir)?;
    let filtered = filter_test_suites(available, &test_suites)?;

    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    // Parse all test suites upfront. YAML/config errors are infrastructure
    // errors and must fail before scenarios start.
    let parsed_suites = parse_test_suites(&visual_tests_dir, filtered)?;
    validate_external_capture_files(root_dir, &workspace_config, &parsed_suites)?;
    ensure_capture_run_directories_are_new(common::REGRESSIFY_DIR.get_required()?, &parsed_suites)?;

    let workspace_config = Arc::new(workspace_config);

    for (input, suite) in &parsed_suites {
        let mut order: Vec<String> = Vec::new();
        let mut thresholds: Vec<(String, f64)> = Vec::new();
        for scenario in &suite.scenarios {
            let order_keys = scenario_capture_order_keys(
                workspace_config.as_ref(),
                suite,
                scenario,
                &suite.scenarios,
            )?;
            let resolved = config::resolve_config(workspace_config.as_ref(), suite, scenario);
            for order_key in &order_keys {
                thresholds.push((order_key.clone(), resolved.mis_match_threshold));
                if let Some(prefix) = order_key.split("__").next() {
                    thresholds.push((prefix.to_string(), resolved.mis_match_threshold));
                }
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
        let suite_id = suite
            .id
            .as_deref()
            .ok_or_else(|| anyhow!("Test suite '{}' is missing an ID", input.name))?;
        let suite_dir = suite_output_dir(regressify_dir, suite)?;
        let tests_dir = suite_dir.join("bitmaps_test");

        if !tests_dir.exists() {
            eprintln!("No test screenshots found for '{}', skipping.", input.name);
            continue;
        }

        let current_run = suite
            .test_date
            .as_deref()
            .ok_or_else(|| anyhow!("Test suite '{}' is missing a capture run ID", input.name))?;
        let latest_test_dir = current_capture_run_dir(&tests_dir, suite)?;
        let ref_dest = suite_dir.join("bitmaps_reference");
        fs::create_dir_all(&ref_dest)?;

        let mut count = 0;
        let mut skipped_failed_count = 0;
        for entry in fs::read_dir(&latest_test_dir)? {
            let entry = entry?;
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
            current_run
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
        generate_comparison_report(&suite_dir, suite_id, current_run, 0.0, async_compare_limit)
            .map_err(|error| {
                anyhow!(
                    "Failed to generate reference comparison report for '{}': {}",
                    input.name,
                    error
                )
            })?;
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

        let suites = discover_test_suites(temp_dir.path()).unwrap();

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

        let suites = discover_test_suites(temp_dir.path()).unwrap();

        assert_eq!(suites.len(), 1);
        assert_eq!(
            suites[0].path.file_name().and_then(|name| name.to_str()),
            Some("alloy.tests.yml")
        );
    }

    #[test]
    fn discovery_and_filtering_report_invalid_inputs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing_directory = temp_dir.path().join("missing");
        let discovery = discover_test_suites(&missing_directory);
        assert!(
            matches!(discovery, Err(error) if error.to_string().contains("Failed to discover test suites"))
        );

        let filtering = filter_test_suites(Vec::new(), &["[".to_string()]);
        assert!(
            matches!(filtering, Err(error) if error.to_string().contains("Invalid test suite pattern"))
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

    #[test]
    fn rejects_zero_operational_timeouts_before_browser_launch() {
        for workspace in [
            config::WorkspaceConfig {
                element_timeout: Some(0),
                ..Default::default()
            },
            config::WorkspaceConfig {
                actionability_timeout: Some(0),
                ..Default::default()
            },
            config::WorkspaceConfig {
                scenario_timeout: Some(0),
                ..Default::default()
            },
        ] {
            assert!(
                validate_workspace_config(&workspace)
                    .unwrap_err()
                    .to_string()
                    .contains("must be greater than zero")
            );
        }
    }

    #[test]
    fn rejects_invalid_visual_thresholds_and_timing_values() {
        let invalid_workspace = config::WorkspaceConfig {
            mis_match_threshold: Some(100.1),
            ..Default::default()
        };
        assert!(
            validate_workspace_config(&invalid_workspace)
                .unwrap_err()
                .to_string()
                .contains("between 0 and 100")
        );

        let mut suite_with_negative_wait = suite(vec![scenario("home", None)]);
        suite_with_negative_wait.post_interaction_wait = Some(-1.0);
        assert!(
            validate_test_suite_config(&input(), &suite_with_negative_wait)
                .unwrap_err()
                .to_string()
                .contains("greater than or equal to zero")
        );

        let mut suite_with_non_finite_delay = suite(vec![scenario("home", None)]);
        suite_with_non_finite_delay.scenarios[0].delay = Some(f64::NAN);
        assert!(
            validate_test_suite_config(&input(), &suite_with_non_finite_delay)
                .unwrap_err()
                .to_string()
                .contains("finite value")
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

    #[test]
    fn suite_output_directory_uses_configured_suite_id() {
        let mut suite = suite(vec![scenario("home", None)]);
        suite.id = Some("production-dashboard".to_string());
        let temp = tempfile::tempdir().unwrap();

        let output = suite_output_dir(temp.path(), &suite).unwrap();

        assert_eq!(output, temp.path().join("production-dashboard"));
    }

    #[test]
    fn current_capture_run_directory_uses_suite_run_id_not_latest_directory() {
        let temp = tempfile::tempdir().unwrap();
        let tests_dir = temp.path().join("bitmaps_test");
        let mut suite = suite(vec![scenario("home", None)]);
        suite.test_date = Some("20260913_120000_000000001".to_string());
        std::fs::create_dir_all(tests_dir.join(suite.test_date.as_deref().unwrap())).unwrap();
        std::fs::create_dir_all(tests_dir.join("99991231_235959_999999999")).unwrap();

        assert_eq!(
            current_capture_run_dir(&tests_dir, &suite).unwrap(),
            tests_dir.join("20260913_120000_000000001")
        );
    }

    #[test]
    fn capture_commands_reject_reusing_an_existing_run_directory() {
        let temp = tempfile::tempdir().unwrap();
        let mut suite = suite(vec![scenario("home", None)]);
        suite.test_date = Some("ci-123".to_string());
        let parsed = vec![(input(), Arc::new(suite))];
        std::fs::create_dir_all(temp.path().join("alloy/bitmaps_test/ci-123")).unwrap();

        let error = ensure_capture_run_directories_are_new(temp.path(), &parsed).unwrap_err();
        assert!(error.to_string().contains("already exists"));

        std::fs::remove_dir_all(temp.path().join("alloy/bitmaps_test/ci-123")).unwrap();
        assert!(ensure_capture_run_directories_are_new(temp.path(), &parsed).is_ok());
    }

    #[test]
    fn rejects_unsafe_suite_and_scenario_output_identifiers() {
        let mut unsafe_suite = suite(vec![scenario("home", None)]);
        unsafe_suite.id = Some("../outside".to_string());
        assert!(
            validate_test_suite_config(&input(), &unsafe_suite)
                .unwrap_err()
                .to_string()
                .contains("Test suite ID")
        );

        let mut unsafe_scenario_suite = suite(vec![scenario("home", None)]);
        unsafe_scenario_suite.scenarios[0].id = Some("nested/home".to_string());
        assert!(
            validate_test_suite_config(&input(), &unsafe_scenario_suite)
                .unwrap_err()
                .to_string()
                .contains("Scenario ID")
        );

        let mut long_identifier_suite = suite(vec![scenario("home", None)]);
        long_identifier_suite.id = Some("a".repeat(MAX_OUTPUT_IDENTIFIER_BYTES + 1));
        assert!(
            validate_test_suite_config(&input(), &long_identifier_suite)
                .unwrap_err()
                .to_string()
                .contains("no longer than")
        );
    }

    #[test]
    fn rejects_suites_with_case_insensitive_output_id_collisions() {
        let mut first = suite(vec![scenario("home", None)]);
        first.id = Some("Production".to_string());
        let mut second = suite(vec![scenario("home", None)]);
        second.id = Some("production".to_string());
        let inputs = vec![
            (
                TestSuiteInput {
                    name: "first".to_string(),
                    path: PathBuf::from("first.tests.yaml"),
                    format: TestSuiteFormat::Yaml,
                },
                Arc::new(first),
            ),
            (
                TestSuiteInput {
                    name: "second".to_string(),
                    path: PathBuf::from("second.tests.yaml"),
                    format: TestSuiteFormat::Yaml,
                },
                Arc::new(second),
            ),
        ];

        let error = validate_unique_suite_output_ids(&inputs).unwrap_err();

        assert!(error.to_string().contains("share output ID"));
    }

    #[test]
    fn explicit_external_capture_files_are_validated_before_browser_launch() {
        let workspace_root = tempfile::tempdir().unwrap();
        let output_dir = workspace_root.path().join(".regressify");
        let cookies_path = workspace_root.path().join("common/cookies.yaml");
        std::fs::create_dir_all(cookies_path.parent().unwrap()).unwrap();
        std::fs::write(&cookies_path, "[]\n").unwrap();
        let parsed = vec![(input(), Arc::new(suite(vec![scenario("home", None)])))];

        let workspace = config::WorkspaceConfig {
            cookie_path: Some("common/cookies.yaml".to_string()),
            ..Default::default()
        };
        assert!(
            validate_external_capture_files_in(
                workspace_root.path(),
                &output_dir,
                &workspace,
                &parsed,
            )
            .is_ok()
        );

        std::fs::write(
            &cookies_path,
            "- name: session\n  value: abc\n  domain: example.com\n  sameSite: Invalid\n",
        )
        .unwrap();
        let error = validate_external_capture_files_in(
            workspace_root.path(),
            &output_dir,
            &workspace,
            &parsed,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("Strict, Lax, or None"));

        std::fs::write(&cookies_path, "not: a cookie list\n").unwrap();
        let error = validate_external_capture_files_in(
            workspace_root.path(),
            &output_dir,
            &workspace,
            &parsed,
        )
        .unwrap_err();
        assert!(error.to_string().contains("Invalid cookie YAML"));

        let missing_css = config::WorkspaceConfig {
            css_override_path: Some("common/missing.css".to_string()),
            ..Default::default()
        };
        assert!(
            validate_external_capture_files_in(
                workspace_root.path(),
                &output_dir,
                &missing_css,
                &parsed,
            )
            .unwrap_err()
            .to_string()
            .contains("Invalid cssOverridePath")
        );

        let missing_script = config::WorkspaceConfig {
            js_on_ready_path: Some("common/missing.js".to_string()),
            ..Default::default()
        };
        assert!(
            validate_external_capture_files_in(
                workspace_root.path(),
                &output_dir,
                &missing_script,
                &parsed,
            )
            .unwrap_err()
            .to_string()
            .contains("Invalid jsOnReadyPath")
        );
    }

    #[test]
    fn configured_persisted_states_are_validated_before_browser_launch() {
        let workspace_root = tempfile::tempdir().unwrap();
        let output_dir = workspace_root.path().join(".regressify");
        let mut configured_suite = suite(vec![scenario("home", None)]);
        configured_suite.state = Some(Needs::String("auth".to_string()));
        let parsed = vec![(input(), Arc::new(configured_suite))];
        let workspace = config::WorkspaceConfig::default();

        let error = validate_external_capture_files_in(
            workspace_root.path(),
            &output_dir,
            &workspace,
            &parsed,
        )
        .unwrap_err();
        assert!(error.to_string().contains("Invalid persisted state"));

        let state_path = output_dir.join("states/auth.json");
        std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        std::fs::write(&state_path, "not json").unwrap();
        let error = validate_external_capture_files_in(
            workspace_root.path(),
            &output_dir,
            &workspace,
            &parsed,
        )
        .unwrap_err();
        assert!(error.to_string().contains("Invalid persisted state"));

        std::fs::write(
            &state_path,
            r#"{"cookies":[],"localStorage":{"theme":"dark"}}"#,
        )
        .unwrap();
        assert!(
            validate_external_capture_files_in(
                workspace_root.path(),
                &output_dir,
                &workspace,
                &parsed,
            )
            .is_ok()
        );

        std::fs::write(&state_path, r#"{"cookies":{}}"#).unwrap();
        let error = validate_external_capture_files_in(
            workspace_root.path(),
            &output_dir,
            &workspace,
            &parsed,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("cookies' must be an array"));
    }
}
