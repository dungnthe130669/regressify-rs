use std::fs::{self, create_dir_all};
use std::path::Path;

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;

use crate::compare::image_compare::{CompareResult, compare_images};
use crate::test::capture::read_error_marker;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportConfig {
    test_suite: String,
    id: String,
    tests: Vec<ReportTest>,
}

#[derive(Debug, Serialize)]
struct ReportTest {
    pair: ReportPair,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportPair {
    label: String,
    reference: String,
    test: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_image: Option<String>,
    selector: String,
    file_name: String,
    mis_match_threshold: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff: Option<ReportDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine_error_msg: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportDiff {
    is_same_dimensions: bool,
    dimension_difference: DimensionDiff,
    raw_mis_match_percentage: f64,
    mis_match_percentage: String,
    analysis_time: u128,
}

#[derive(Debug, Serialize)]
struct DimensionDiff {
    width: i64,
    height: i64,
}

/// Generate a comparison report for a test suite.
///
/// Directory structure:
/// ```text
/// .regressify/<suite_id>/
///   bitmaps_reference/   ← reference screenshots
///   bitmaps_test/<date>/            ← test screenshots + diff images + report.json
///   html_report/config.js           ← report config for React UI
///   tests/<date>/                   ← raw captured screenshots (existing)
/// ```
pub fn generate_comparison_report(
    suite_dir: &Path,
    suite_id: &str,
    test_date: &str,
    mis_match_threshold: f64,
) -> Result<()> {
    let tests_dir = suite_dir.join("bitmaps_test").join(test_date);
    if !tests_dir.exists() {
        anyhow::bail!("Test screenshots directory does not exist: {:?}", tests_dir);
    }

    let reference_dir = suite_dir.join("bitmaps_reference");
    let bitmaps_test_dir = suite_dir.join("bitmaps_test").join(test_date);
    let html_report_dir = suite_dir.join("html_report");

    create_dir_all(&reference_dir)?;
    create_dir_all(&bitmaps_test_dir)?;
    create_dir_all(&html_report_dir)?;

    // Collect all PNG files from bitmaps_test/<date>/
    let mut test_screenshots: Vec<String> = Vec::new();
    for entry in fs::read_dir(&tests_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.ends_with(".png") {
            test_screenshots.push(file_name);
        }
    }
    test_screenshots.sort();

    let mut report_tests: Vec<ReportTest> = Vec::new();

    for screenshot_name in &test_screenshots {
        let test_src = tests_dir.join(screenshot_name);
        let test_dst = bitmaps_test_dir.join(screenshot_name);

        // Copy test screenshot to bitmaps_test/<date>/ (skip if already there)
        if test_src != test_dst {
            fs::copy(&test_src, &test_dst)?;
        }

        let reference_path = reference_dir.join(screenshot_name);

        // Relative paths for the report (from html_report/ perspective)
        let relative_test = format!("../bitmaps_test/{}/{}", test_date, screenshot_name);

        // Derive label from filename: "scenario-id__FINAL.png" → "scenario-id FINAL"
        let label = screenshot_name
            .trim_end_matches(".png")
            .replace("__", " ")
            .replace('$', "");

        // If either side is flagged as a capture failure (via its JSON error
        // marker), the corresponding scenario never produced a real screenshot.
        // Skip the visual comparison and report the failure directly — comparing
        // against a placeholder is meaningless and would only produce noise.
        let test_err = read_error_marker(&test_dst);
        let ref_err = if reference_path.exists() {
            read_error_marker(&reference_path)
        } else {
            None
        };
        if test_err.is_some() || ref_err.is_some() {
            let msg = match (ref_err.as_deref(), test_err.as_deref()) {
                (Some(r), Some(t)) => format!(
                    "Both reference and test scenarios failed to capture (reference: {r}; test: {t})"
                ),
                (Some(r), None) => format!("Reference scenario failed to capture: {r}"),
                (None, Some(t)) => format!("Test scenario failed to capture: {t}"),
                (None, None) => unreachable!(),
            };
            eprintln!("    {} {} → {}", "FAIL:".red(), screenshot_name, msg);

            report_tests.push(ReportTest {
                pair: ReportPair {
                    label,
                    reference: format!("../bitmaps_reference/{}", screenshot_name),
                    test: relative_test,
                    diff_image: None,
                    selector: "document".to_string(),
                    file_name: screenshot_name.clone(),
                    mis_match_threshold,
                    diff: None,
                    engine_error_msg: Some(msg),
                },
                status: "fail".to_string(),
            });
            continue;
        }

        if !reference_path.exists() {
            // No reference image — first run or new scenario
            eprintln!(
                "    {} No reference for {}. Marking as new.",
                "NEW:".yellow(),
                screenshot_name
            );

            report_tests.push(ReportTest {
                pair: ReportPair {
                    label,
                    reference: format!("../bitmaps_reference/{}", screenshot_name),
                    test: relative_test,
                    diff_image: None,
                    selector: "document".to_string(),
                    file_name: screenshot_name.clone(),
                    mis_match_threshold,
                    diff: None,
                    engine_error_msg: Some("Reference image not found".to_string()),
                },
                status: "fail".to_string(),
            });
            continue;
        }

        // Compare images
        match compare_images(&reference_path, &test_dst) {
            Ok((result, diff_png)) => {
                let diff_image_name = screenshot_name.replace(".png", "_diff.png");
                let diff_image_path = bitmaps_test_dir.join(&diff_image_name);

                // Write diff image if available
                if let Some(diff_data) = diff_png {
                    fs::write(&diff_image_path, diff_data)?;
                }

                let passed = result.raw_mis_match_percentage <= mis_match_threshold
                    && result.is_same_dimensions;

                let status = if passed { "pass" } else { "fail" };

                let status_text = if passed {
                    format!("{:.2}%", result.raw_mis_match_percentage).green()
                } else {
                    format!("{:.2}%", result.raw_mis_match_percentage).red()
                };

                eprintln!(
                    "    {} {} → mismatch: {}",
                    if passed {
                        "PASS:".green()
                    } else {
                        "FAIL:".red()
                    },
                    screenshot_name,
                    status_text,
                );

                let relative_diff = format!("../bitmaps_test/{}/{}", test_date, diff_image_name);

                report_tests.push(ReportTest {
                    pair: ReportPair {
                        label,
                        reference: format!("../bitmaps_reference/{}", screenshot_name),
                        test: relative_test,
                        diff_image: if diff_image_path.exists() {
                            Some(relative_diff)
                        } else {
                            None
                        },
                        selector: "document".to_string(),
                        file_name: screenshot_name.clone(),
                        mis_match_threshold,
                        diff: Some(build_diff(&result)),
                        engine_error_msg: if !result.is_same_dimensions {
                            Some(format!(
                                "Dimensions differ: reference {}x{} vs test {}x{}",
                                result.ref_width,
                                result.ref_height,
                                result.test_width,
                                result.test_height
                            ))
                        } else {
                            None
                        },
                    },
                    status: status.to_string(),
                });
            }
            Err(e) => {
                eprintln!(
                    "    {} Failed to compare {}: {}",
                    "ERROR:".red(),
                    screenshot_name,
                    e
                );
                report_tests.push(ReportTest {
                    pair: ReportPair {
                        label,
                        reference: format!("../bitmaps_reference/{}", screenshot_name),
                        test: relative_test,
                        diff_image: None,
                        selector: "document".to_string(),
                        file_name: screenshot_name.clone(),
                        mis_match_threshold,
                        diff: None,
                        engine_error_msg: Some(format!("Comparison error: {}", e)),
                    },
                    status: "fail".to_string(),
                });
            }
        }
    }

    // Generate report.json
    let report = ReportConfig {
        test_suite: suite_id.to_string(),
        id: suite_id.to_string(),
        tests: report_tests,
    };

    let report_json = serde_json::to_string_pretty(&report)?;
    let report_path = bitmaps_test_dir.join("report.json");
    fs::write(&report_path, &report_json)?;

    // Generate config.js for Regressify React UI
    let config_js = format!("report({});", report_json);
    let config_path = html_report_dir.join("config.js");
    fs::write(&config_path, &config_js)?;

    // Print summary
    let pass_count = report.tests.iter().filter(|t| t.status == "pass").count();
    let fail_count = report.tests.iter().filter(|t| t.status == "fail").count();
    eprintln!(
        "  Comparison complete: {} passed, {} failed out of {} tests",
        pass_count.to_string().green(),
        if fail_count > 0 {
            fail_count.to_string().red()
        } else {
            fail_count.to_string().normal()
        },
        report.tests.len()
    );

    if fail_count > 0 {
        anyhow::bail!(
            "Comparison failed: {} failed out of {} tests",
            fail_count,
            report.tests.len()
        );
    }

    Ok(())
}

/// Promote test screenshots to reference (approve).
pub fn approve_screenshots(suite_dir: &Path) -> Result<()> {
    let bitmaps_test_dir = suite_dir.join("bitmaps_test");
    let reference_dir = suite_dir.join("bitmaps_reference");

    if !bitmaps_test_dir.exists() {
        eprintln!("  {}", "No test screenshots found, skipping.".yellow());
        return Ok(());
    }

    // Find the latest test run directory (sorted by name = date)
    let mut test_runs: Vec<String> = Vec::new();
    for entry in fs::read_dir(&bitmaps_test_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            test_runs.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    test_runs.sort();

    let latest_run = match test_runs.last() {
        Some(run) => run,
        None => {
            eprintln!("  {}", "No test run directories found, skipping.".yellow());
            return Ok(());
        }
    };

    let latest_test_dir = bitmaps_test_dir.join(latest_run);

    eprintln!("  Approving screenshots from run: {}", latest_run.cyan());

    create_dir_all(&reference_dir)?;

    let mut approved_count = 0;
    for entry in fs::read_dir(&latest_test_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Only copy actual test screenshots, not diff images
        if file_name.ends_with(".png") && !file_name.ends_with("_diff.png") {
            let src = entry.path();
            let dst = reference_dir.join(&file_name);
            fs::copy(&src, &dst)?;

            // Promote the error-marker sidecar so the reference retains its
            // failure flag; clear any stale marker when the new capture is good.
            let marker_src = crate::test::capture::error_marker_path(&src);
            let marker_dst = crate::test::capture::error_marker_path(&dst);
            if marker_src.exists() {
                fs::copy(&marker_src, &marker_dst)?;
            } else if marker_dst.exists() {
                fs::remove_file(&marker_dst)?;
            }

            approved_count += 1;
            eprintln!("    Approved: {}", file_name.green());
        }
    }

    eprintln!(
        "  {} screenshot(s) approved as new reference.",
        approved_count.to_string().green()
    );

    Ok(())
}

fn build_diff(result: &CompareResult) -> ReportDiff {
    ReportDiff {
        is_same_dimensions: result.is_same_dimensions,
        dimension_difference: DimensionDiff {
            width: result.dimension_difference.0,
            height: result.dimension_difference.1,
        },
        raw_mis_match_percentage: result.raw_mis_match_percentage,
        mis_match_percentage: result.mis_match_percentage.clone(),
        analysis_time: result.analysis_time_ms,
    }
}
