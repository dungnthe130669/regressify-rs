use std::fs;

use anyhow::Result;
use colored::Colorize;

use crate::{
    cli::args::{ApproveArgs, CompareArgs},
    common,
    compare::report_gen::{approve_screenshots, generate_comparison_report, select_capture_run},
    helpers::OnceLockExtensions,
};

fn compile_suite_filters(patterns: &[String]) -> Result<Option<Vec<glob::Pattern>>> {
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

fn suite_matches_filters(suite_id: &str, filters: &Option<Vec<glob::Pattern>>) -> bool {
    filters
        .as_ref()
        .is_none_or(|filters| filters.iter().any(|filter| filter.matches(suite_id)))
}

/// Entry point for the `compare` CLI command.
/// Compares the latest test screenshots against reference images for each test suite.
pub fn compare(args: CompareArgs) -> Result<()> {
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    if !regressify_dir.exists() {
        anyhow::bail!("Regressify directory does not exist: {:?}", regressify_dir);
    }

    let mis_match_threshold = args.mis_match_threshold;
    let async_compare_limit = args.async_compare_limit;
    let requested_run_id = common::RUN_ID.get().map(String::as_str);
    let suite_filters = compile_suite_filters(&args.test_suite)?;

    let mut entries: Vec<_> = fs::read_dir(regressify_dir)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());

    let mut any_compared = false;

    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let suite_id = match path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Filter by suite patterns if provided
        if !suite_matches_filters(&suite_id, &suite_filters) {
            continue;
        }

        let tests_dir = path.join("bitmaps_test");
        if !tests_dir.exists() {
            continue;
        }

        if let Some(selected_run) = select_capture_run(&tests_dir, requested_run_id)? {
            eprintln!(
                "Comparing test suite: {} (run: {})",
                suite_id.cyan(),
                selected_run
            );

            generate_comparison_report(
                &path,
                &suite_id,
                &selected_run,
                mis_match_threshold,
                async_compare_limit,
            )?;
            any_compared = true;
        }
    }

    if !any_compared {
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
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    if !regressify_dir.exists() {
        anyhow::bail!("Regressify directory does not exist: {:?}", regressify_dir);
    }

    let mut entries: Vec<_> = fs::read_dir(regressify_dir)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());

    let mut any_approved = false;
    let requested_run_id = common::RUN_ID.get().map(String::as_str);
    let suite_filters = compile_suite_filters(&args.test_suite)?;

    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let suite_id = match path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Filter by suite patterns if provided
        if !suite_matches_filters(&suite_id, &suite_filters) {
            continue;
        }

        let bitmaps_test_dir = path.join("bitmaps_test");
        if !bitmaps_test_dir.exists() {
            continue;
        }

        eprintln!("Approving test suite: {}", suite_id.cyan());
        approve_screenshots(&path, requested_run_id)?;
        any_approved = true;
    }

    if !any_approved {
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
}
