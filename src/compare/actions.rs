use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use colored::Colorize;

use crate::{
    cli::args::{ApproveArgs, CompareArgs, ScenarioFilter},
    common,
    compare::report_gen::{
        approve_screenshots_for_scenarios, generate_comparison_report_for_scenarios,
        select_capture_run, validate_scenario_captures,
    },
    helpers::OnceLockExtensions,
};

pub(crate) fn compile_suite_filters(patterns: &[String]) -> Result<Option<Vec<glob::Pattern>>> {
    if patterns.is_empty()
        || patterns
            .iter()
            .any(|pattern| pattern == "all" || pattern == "*")
    {
        return Ok(None);
    }

    patterns
        .iter()
        .map(|pattern| {
            glob::Pattern::new(pattern).map_err(|error| {
                anyhow::anyhow!("Invalid test suite pattern '{}': {}", pattern, error)
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

pub(crate) fn suite_matches_filters(suite_id: &str, filters: &Option<Vec<glob::Pattern>>) -> bool {
    filters
        .as_ref()
        .is_none_or(|filters| filters.iter().any(|filter| filter.matches(suite_id)))
}

pub(crate) fn require_explicit_filter_match(
    filters: &Option<Vec<glob::Pattern>>,
    matched_suite: bool,
) -> Result<()> {
    if filters.is_some() && !matched_suite {
        anyhow::bail!("No output test suites matched the requested --test-suite filter");
    }
    Ok(())
}

struct SelectedRun {
    path: PathBuf,
    suite_id: String,
    run_id: String,
    scenario_ids: Vec<String>,
}

fn selected_runs(
    regressify_dir: &Path,
    suite_patterns: &[String],
    scenario_filters: &[ScenarioFilter],
    requested_run_id: Option<&str>,
) -> Result<Vec<SelectedRun>> {
    if !regressify_dir.exists() {
        anyhow::bail!("Regressify directory does not exist: {:?}", regressify_dir);
    }
    let suite_filters = compile_suite_filters(suite_patterns)?;
    let mut entries: Vec<_> = fs::read_dir(regressify_dir)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());
    let mut runs = Vec::new();
    let mut matched_suite = false;
    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let suite_id = match path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };
        if !suite_matches_filters(&suite_id, &suite_filters) {
            continue;
        }
        matched_suite = true;
        let scenario_ids = scenario_filters
            .iter()
            .filter(|filter| filter.suite == suite_id)
            .map(|filter| filter.scenario.clone())
            .collect::<Vec<_>>();
        if !scenario_filters.is_empty() && scenario_ids.is_empty() {
            continue;
        }
        let tests_dir = path.join("bitmaps_test");
        if !tests_dir.exists() {
            continue;
        }
        if let Some(run_id) = select_capture_run(&tests_dir, requested_run_id)? {
            if !scenario_ids.is_empty() {
                validate_scenario_captures(&tests_dir.join(&run_id), &scenario_ids)?;
            }
            runs.push(SelectedRun {
                path,
                suite_id,
                run_id,
                scenario_ids,
            });
        }
    }
    require_explicit_filter_match(&suite_filters, matched_suite)?;
    for filter in scenario_filters {
        anyhow::ensure!(
            runs.iter().any(|run| run.suite_id == filter.suite),
            "Selected scenario '{filter}' has no capture run in the selected output suites"
        );
    }
    Ok(runs)
}

/// Entry point for the `compare` CLI command.
/// Compares the latest test screenshots against reference images for each test suite.
pub fn compare(args: CompareArgs) -> Result<()> {
    let runs = selected_runs(
        common::REGRESSIFY_DIR.get_required()?,
        &args.test_suite,
        &args.scenarios,
        common::RUN_ID.get().map(String::as_str),
    )?;
    for run in &runs {
        eprintln!(
            "Comparing test suite: {} (run: {})",
            run.suite_id.cyan(),
            run.run_id
        );
        generate_comparison_report_for_scenarios(
            &run.path,
            &run.suite_id,
            &run.run_id,
            args.mis_match_threshold,
            args.async_compare_limit,
            &run.scenario_ids,
        )?;
    }
    if runs.is_empty() {
        eprintln!(
            "{}",
            "No test suites with screenshots found to compare.".yellow()
        );
    }

    Ok(())
}

/// Entry point for the `approve` CLI command.
/// Promotes the latest test screenshots to reference images.
pub fn approve(args: ApproveArgs) -> Result<()> {
    let runs = selected_runs(
        common::REGRESSIFY_DIR.get_required()?,
        &args.test_suite,
        &args.scenarios,
        common::RUN_ID.get().map(String::as_str),
    )?;
    for run in &runs {
        eprintln!("Approving test suite: {}", run.suite_id.cyan());
        approve_screenshots_for_scenarios(&run.path, Some(&run.run_id), &run.scenario_ids)?;
    }
    if runs.is_empty() {
        eprintln!(
            "{}",
            "No test suites with test screenshots found to approve.".yellow()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_filters_match_all_or_compiled_patterns() {
        let all = compile_suite_filters(&[]).unwrap();
        assert!(suite_matches_filters("dashboard", &all));

        let filters = compile_suite_filters(&["dash*".to_string(), "store".to_string()]).unwrap();
        assert!(suite_matches_filters("dashboard", &filters));
        assert!(suite_matches_filters("store", &filters));
        assert!(!suite_matches_filters("account", &filters));
    }

    #[test]
    fn suite_filters_reject_invalid_globs() {
        let error = compile_suite_filters(&["[".to_string()]).unwrap_err();

        assert!(error.to_string().contains("Invalid test suite pattern"));
    }

    #[test]
    fn explicit_suite_filters_must_match_an_output_suite() {
        let explicit = compile_suite_filters(&["missing".to_string()]).unwrap();
        assert!(
            require_explicit_filter_match(&explicit, false)
                .unwrap_err()
                .to_string()
                .contains("No output test suites matched")
        );

        let all = compile_suite_filters(&[]).unwrap();
        assert!(require_explicit_filter_match(&all, false).is_ok());
        assert!(require_explicit_filter_match(&explicit, true).is_ok());
    }
}
