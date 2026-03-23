use std::fs;

use anyhow::Result;
use colored::Colorize;

use crate::{
    cli::args::{ApproveArgs, CompareArgs},
    common,
    compare::report_gen::{approve_screenshots, generate_comparison_report},
    helpers::OnceLockExtensions,
};

/// Entry point for the `compare` CLI command.
/// Compares the latest test screenshots against reference images for each test suite.
pub fn compare(args: CompareArgs) -> Result<()> {
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    if !regressify_dir.exists() {
        anyhow::bail!("Regressify directory does not exist: {:?}", regressify_dir);
    }

    let mis_match_threshold = args.mis_match_threshold;

    let mut entries: Vec<_> = fs::read_dir(regressify_dir)?.flatten().collect();
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
        if !args.test_suite.is_empty() && !args.test_suite.contains(&"all".to_string()) {
            let matched = args.test_suite.iter().any(|pattern| {
                glob::Pattern::new(pattern)
                    .map(|p| p.matches(&suite_id))
                    .unwrap_or(false)
            });
            if !matched {
                continue;
            }
        }

        let tests_dir = path.join("tests");
        if !tests_dir.exists() {
            continue;
        }

        // Find the latest test run
        let mut test_runs: Vec<String> = Vec::new();
        for run_entry in fs::read_dir(&tests_dir)?.flatten() {
            if run_entry.path().is_dir() {
                test_runs.push(run_entry.file_name().to_string_lossy().to_string());
            }
        }
        test_runs.sort();

        if let Some(latest_run) = test_runs.last() {
            eprintln!(
                "Comparing test suite: {} (run: {})",
                suite_id.cyan(),
                latest_run
            );

            generate_comparison_report(&path, &suite_id, latest_run, mis_match_threshold)?;
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

    let mut entries: Vec<_> = fs::read_dir(regressify_dir)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    let mut any_approved = false;

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
        if !args.test_suite.is_empty() && !args.test_suite.contains(&"all".to_string()) {
            let matched = args.test_suite.iter().any(|pattern| {
                glob::Pattern::new(pattern)
                    .map(|p| p.matches(&suite_id))
                    .unwrap_or(false)
            });
            if !matched {
                continue;
            }
        }

        let bitmaps_test_dir = path.join("bitmaps_test");
        if !bitmaps_test_dir.exists() {
            continue;
        }

        eprintln!("Approving test suite: {}", suite_id.cyan());
        approve_screenshots(&path, &suite_id)?;
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
