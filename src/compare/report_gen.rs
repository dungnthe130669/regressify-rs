use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, create_dir_all};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use rayon::prelude::*;
use serde::Serialize;

use crate::compare::image_compare::{CompareResult, compare_images};
use crate::helpers::{copy_file_atomically, write_file_atomically};
use crate::test::capture::{
    REPORT_ORDER_FILE, REPORT_ORDER_VERSION, REPORT_THRESHOLDS_FILE, REPORT_URLS_FILE,
    REPORT_URLS_VERSION, ReportOrderManifest, ReportPageUrls, ReportUrlsManifest,
    read_error_marker,
};

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
    reference_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
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

struct ComparisonJob {
    pair: ReportPair,
    reference_path: PathBuf,
    test_path: PathBuf,
}

enum ReportWork {
    Immediate(ReportTest),
    Compare(ComparisonJob),
}

enum ComparisonOutcome {
    Compared {
        result: CompareResult,
        diff_png: Option<Vec<u8>>,
    },
    Failed(anyhow::Error),
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

struct ReportOrder {
    captures: Vec<String>,
    is_complete_inventory: bool,
}

/// Read the report ordering manifest. Older runs may not have one, but a
/// present malformed manifest is an artifact error rather than an empty order.
fn read_report_order(tests_dir: &Path) -> Result<Option<ReportOrder>> {
    let path = tests_dir.join(REPORT_ORDER_FILE);
    match fs::read_to_string(&path) {
        Ok(content) => {
            let value: serde_json::Value = serde_json::from_str(&content)
                .with_context(|| format!("Invalid report order manifest '{}'", path.display()))?;
            let (captures, is_complete_inventory) = if value.is_array() {
                let captures: Vec<String> = serde_json::from_value(value).with_context(|| {
                    format!("Invalid report order manifest '{}'", path.display())
                })?;
                (captures, false)
            } else {
                let manifest: ReportOrderManifest =
                    serde_json::from_value(value).with_context(|| {
                        format!("Invalid report order manifest '{}'", path.display())
                    })?;
                if manifest.version != REPORT_ORDER_VERSION {
                    anyhow::bail!(
                        "Unsupported report order manifest version {} in '{}'",
                        manifest.version,
                        path.display()
                    );
                }
                (manifest.captures, true)
            };
            let mut seen = HashSet::with_capacity(captures.len());
            for key in &captures {
                if key.trim().is_empty() || !seen.insert(key) {
                    anyhow::bail!(
                        "Invalid report order manifest '{}': duplicate or empty key",
                        path.display()
                    );
                }
            }
            Ok(Some(ReportOrder {
                captures,
                is_complete_inventory,
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to read report order manifest '{}'", path.display())),
    }
}

fn read_report_thresholds(tests_dir: &Path) -> Result<Option<HashMap<String, f64>>> {
    let path = tests_dir.join(REPORT_THRESHOLDS_FILE);
    match fs::read_to_string(&path) {
        Ok(content) => {
            let thresholds: HashMap<String, f64> =
                serde_json::from_str(&content).with_context(|| {
                    format!("Invalid report threshold manifest '{}'", path.display())
                })?;
            for (key, value) in &thresholds {
                if key.trim().is_empty() || !value.is_finite() || !(0.0..=100.0).contains(value) {
                    anyhow::bail!(
                        "Invalid report threshold manifest '{}': '{}' must be a finite value between 0 and 100",
                        path.display(),
                        key
                    );
                }
            }
            Ok(Some(thresholds))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| {
            format!(
                "Failed to read report threshold manifest '{}'",
                path.display()
            )
        }),
    }
}

/// Read page URLs recorded for a capture run. Older runs predate this manifest
/// and remain reportable without page-link metadata.
fn read_report_urls(tests_dir: &Path) -> Result<Option<BTreeMap<String, ReportPageUrls>>> {
    let path = tests_dir.join(REPORT_URLS_FILE);
    match fs::read_to_string(&path) {
        Ok(content) => {
            let manifest: ReportUrlsManifest = serde_json::from_str(&content)
                .with_context(|| format!("Invalid report URL manifest '{}'", path.display()))?;
            if manifest.version != REPORT_URLS_VERSION {
                anyhow::bail!(
                    "Unsupported report URL manifest version {} in '{}'",
                    manifest.version,
                    path.display()
                );
            }
            for (capture, urls) in &manifest.captures {
                if capture.trim().is_empty()
                    || urls.reference_url.trim().is_empty()
                    || urls.url.trim().is_empty()
                {
                    anyhow::bail!(
                        "Invalid report URL manifest '{}': capture keys and page URLs must not be empty",
                        path.display()
                    );
                }
            }
            Ok(Some(manifest.captures))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to read report URL manifest '{}'", path.display())),
    }
}

fn screenshot_prefix(file_name: &str) -> &str {
    file_name
        .strip_suffix(".png")
        .unwrap_or(file_name)
        .split("__")
        .next()
        .unwrap_or("")
}

fn screenshot_key(file_name: &str) -> &str {
    file_name.strip_suffix(".png").unwrap_or(file_name)
}

/// Position of a screenshot in the ordering manifest, derived from its filename
/// prefix (`<scenario-id>[_<viewport>]`, the part before `__<step|FINAL>`).
/// Screenshots not listed in the manifest sort after all listed ones.
fn screenshot_order_key(file_name: &str, order_index: &HashMap<&str, usize>) -> usize {
    order_index
        .get(screenshot_key(file_name))
        .copied()
        // Older captures stored only the scenario/viewport prefix.
        .or_else(|| order_index.get(screenshot_prefix(file_name)).copied())
        .unwrap_or(usize::MAX)
}

/// Generated diff images live beside captures, but are report artifacts rather
/// than inputs to a later comparison.
fn is_capture_screenshot(file_name: &str) -> bool {
    file_name.ends_with(".png") && !file_name.ends_with("_diff.png")
}

fn collect_capture_screenshots(tests_dir: &Path) -> Result<Vec<String>> {
    let mut screenshots = Vec::new();
    for entry in fs::read_dir(tests_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name = entry.file_name().into_string().map_err(|name| {
            anyhow!(
                "Capture file name in '{}' is not valid UTF-8: {:?}",
                tests_dir.display(),
                name
            )
        })?;
        if is_capture_screenshot(&file_name) {
            screenshots.push(file_name);
        }
    }
    screenshots.sort();
    Ok(screenshots)
}

fn display_capture_keys(captures: &[&str]) -> String {
    const DISPLAY_LIMIT: usize = 5;
    let listed = captures
        .iter()
        .take(DISPLAY_LIMIT)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if captures.len() > DISPLAY_LIMIT {
        format!("{listed}, ...")
    } else {
        listed
    }
}

fn validate_capture_inventory(
    tests_dir: &Path,
    expected_captures: &[String],
    screenshots: &[String],
) -> Result<()> {
    let expected_keys: HashSet<&str> = expected_captures.iter().map(String::as_str).collect();
    let actual_keys: HashSet<&str> = screenshots
        .iter()
        .map(|screenshot| screenshot_key(screenshot))
        .collect();
    let missing: Vec<&str> = expected_captures
        .iter()
        .map(String::as_str)
        .filter(|capture| !actual_keys.contains(capture))
        .collect();
    let unexpected: Vec<&str> = screenshots
        .iter()
        .map(|screenshot| screenshot_key(screenshot))
        .filter(|capture| !expected_keys.contains(capture))
        .collect();

    if missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }

    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!(
            "missing {} screenshot(s) listed in its manifest: {}",
            missing.len(),
            display_capture_keys(&missing)
        ));
    }
    if !unexpected.is_empty() {
        problems.push(format!(
            "contains {} unexpected screenshot(s) absent from its manifest: {}",
            unexpected.len(),
            display_capture_keys(&unexpected)
        ));
    }
    anyhow::bail!(
        "Capture run '{}' {}",
        tests_dir.display(),
        problems.join("; ")
    );
}

/// Validate the exact screenshot inventory for a modern capture run. Legacy
/// array manifests retain their historical order-only behavior.
pub(crate) fn validate_capture_run_inventory(tests_dir: &Path) -> Result<Vec<String>> {
    let screenshots = collect_capture_screenshots(tests_dir)?;
    if let Some(report_order) = read_report_order(tests_dir)?
        && report_order.is_complete_inventory
    {
        validate_capture_inventory(tests_dir, &report_order.captures, &screenshots)?;
    }
    Ok(screenshots)
}

pub fn select_capture_run(
    tests_dir: &Path,
    requested_run_id: Option<&str>,
) -> Result<Option<String>> {
    if let Some(run_id) = requested_run_id {
        let requested_dir = tests_dir.join(run_id);
        let metadata = match fs::symlink_metadata(&requested_dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                anyhow::bail!(
                    "Requested capture run '{}' does not exist: {}",
                    run_id,
                    requested_dir.display()
                );
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to inspect requested capture run '{}': {}",
                        run_id,
                        requested_dir.display()
                    )
                });
            }
        };
        if !metadata.file_type().is_dir() {
            anyhow::bail!(
                "Requested capture run '{}' is not a real directory: {}",
                run_id,
                requested_dir.display()
            );
        }
        return Ok(Some(run_id.to_string()));
    }

    let mut run_ids = Vec::new();
    for entry in fs::read_dir(tests_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let run_id = entry.file_name().into_string().map_err(|name| {
                anyhow!(
                    "Capture run directory in '{}' is not valid UTF-8: {:?}",
                    tests_dir.display(),
                    name
                )
            })?;
            run_ids.push(run_id);
        }
    }
    run_ids.sort();
    Ok(run_ids.pop())
}

fn report_pair(
    label: String,
    screenshot_name: &str,
    relative_test: String,
    threshold: f64,
    page_urls: Option<&ReportPageUrls>,
) -> ReportPair {
    ReportPair {
        label,
        reference: format!("../bitmaps_reference/{}", screenshot_name),
        test: relative_test,
        reference_url: page_urls.map(|urls| urls.reference_url.clone()),
        url: page_urls.map(|urls| urls.url.clone()),
        diff_image: None,
        selector: report_selector(screenshot_name),
        file_name: screenshot_name.to_string(),
        mis_match_threshold: threshold,
        diff: None,
        engine_error_msg: None,
    }
}

fn report_selector(screenshot_name: &str) -> String {
    let target = screenshot_key(screenshot_name)
        .rsplit_once("__")
        .map(|(_, target)| target)
        .unwrap_or("FINAL");
    match target {
        "FINAL" => "document".to_string(),
        "VIEWPORT" => "viewport".to_string(),
        target if target.starts_with('$') => format!("save:{}", &target[1..]),
        target if target.starts_with("SELECTOR-") => target.to_string(),
        target => target.to_string(),
    }
}

fn compare_report_work(
    work: &[ReportWork],
    max_workers: usize,
) -> Result<Vec<Option<ComparisonOutcome>>> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(max_workers.max(1))
        .build()
        .map_err(|error| anyhow!("Failed to create comparison worker pool: {}", error))?;

    Ok(pool.install(|| {
        work.par_iter()
            .map(|entry| match entry {
                ReportWork::Immediate(_) => None,
                ReportWork::Compare(job) => {
                    Some(match compare_images(&job.reference_path, &job.test_path) {
                        Ok((result, diff_png)) => ComparisonOutcome::Compared { result, diff_png },
                        Err(error) => ComparisonOutcome::Failed(error),
                    })
                }
            })
            .collect()
    }))
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
    max_workers: usize,
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

    // Collect captured screenshots only. Diff images from an earlier report
    // regeneration share this directory and must not become test inputs.
    let mut test_screenshots = validate_capture_run_inventory(&tests_dir)?;
    // Order screenshots by the capture manifest (scenario order, then viewport
    // order) so the report mirrors the test suite + viewport file definitions
    // rather than alphabetical filename order. Any screenshot not present in the
    // manifest (e.g. stale files or runs predating the manifest) sorts last, then
    // alphabetically among themselves.
    let report_order = match read_report_order(&tests_dir)? {
        Some(order) => order,
        None => {
            eprintln!(
                "  {} Report order manifest missing; using alphabetical fallback.",
                "WARNING:".yellow()
            );
            ReportOrder {
                captures: Vec::new(),
                is_complete_inventory: false,
            }
        }
    };
    let order = report_order.captures;
    let thresholds = read_report_thresholds(&tests_dir)?.unwrap_or_default();
    let report_urls = read_report_urls(&tests_dir)?;
    if let Some(page_urls) = &report_urls {
        let expected_captures: Vec<String> = page_urls.keys().cloned().collect();
        validate_capture_inventory(&tests_dir, &expected_captures, &test_screenshots)?;
    }
    let mut order_index = HashMap::with_capacity(order.len() * 2);
    for (index, key) in order.iter().enumerate() {
        order_index.insert(key.as_str(), index);
        order_index.entry(screenshot_prefix(key)).or_insert(index);
    }
    test_screenshots.sort_by(|a, b| {
        screenshot_order_key(a, &order_index)
            .cmp(&screenshot_order_key(b, &order_index))
            .then_with(|| a.cmp(b))
    });

    let mut report_work = Vec::with_capacity(test_screenshots.len());
    for screenshot_name in test_screenshots {
        let test_path = tests_dir.join(&screenshot_name);
        let threshold = thresholds
            .get(screenshot_key(&screenshot_name))
            .or_else(|| thresholds.get(screenshot_prefix(&screenshot_name)))
            .copied()
            .unwrap_or(mis_match_threshold);

        let reference_path = reference_dir.join(&screenshot_name);

        // Relative paths for the report (from html_report/ perspective)
        let relative_test = format!("../bitmaps_test/{}/{}", test_date, screenshot_name);

        // Derive label from filename: "scenario-id__FINAL.png" → "scenario-id FINAL"
        let label = screenshot_name
            .trim_end_matches(".png")
            .replace("__", " ")
            .replace('$', "");
        let page_urls = report_urls
            .as_ref()
            .and_then(|urls| urls.get(screenshot_key(&screenshot_name)));
        let mut pair = report_pair(label, &screenshot_name, relative_test, threshold, page_urls);

        // If either side is flagged as a capture failure (via its JSON error
        // marker), the corresponding scenario never produced a real screenshot.
        // Skip the visual comparison and report the failure directly — comparing
        // against a placeholder is meaningless and would only produce noise.
        let test_err = read_error_marker(&test_path);
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

            pair.engine_error_msg = Some(msg);
            report_work.push(ReportWork::Immediate(ReportTest {
                pair,
                status: "fail".to_string(),
            }));
            continue;
        }

        if !reference_path.exists() {
            // No reference image — first run or new scenario
            eprintln!(
                "    {} No reference for {}. Marking as new.",
                "NEW:".yellow(),
                screenshot_name
            );

            pair.engine_error_msg = Some("Reference image not found".to_string());
            report_work.push(ReportWork::Immediate(ReportTest {
                pair,
                status: "fail".to_string(),
            }));
            continue;
        }

        report_work.push(ReportWork::Compare(ComparisonJob {
            pair,
            reference_path,
            test_path,
        }));
    }

    let comparison_count = report_work
        .iter()
        .filter(|entry| matches!(entry, ReportWork::Compare(_)))
        .count();
    if comparison_count > 0 {
        eprintln!(
            "  Comparing {} screenshot pair(s) with up to {} worker(s)...",
            comparison_count,
            max_workers.max(1)
        );
    }
    let outcomes = compare_report_work(&report_work, max_workers)?;
    let mut report_tests = Vec::with_capacity(report_work.len());

    for (entry, outcome) in report_work.into_iter().zip(outcomes) {
        match entry {
            ReportWork::Immediate(test) => report_tests.push(test),
            ReportWork::Compare(job) => match outcome.ok_or_else(|| {
                anyhow!(
                    "Comparison worker did not return an outcome for '{}'",
                    job.pair.file_name
                )
            })? {
                ComparisonOutcome::Compared { result, diff_png } => {
                    let mut pair = job.pair;
                    let diff_image_name = pair.file_name.replace(".png", "_diff.png");
                    let diff_image_path = bitmaps_test_dir.join(&diff_image_name);

                    pair.diff_image = if let Some(diff_data) = diff_png {
                        write_file_atomically(&diff_image_path, &diff_data)?;
                        Some(format!("../bitmaps_test/{}/{}", test_date, diff_image_name))
                    } else {
                        // A previous comparison in this run may have produced a
                        // diff. Do not show that stale artifact for a current pass.
                        if diff_image_path.exists() {
                            fs::remove_file(&diff_image_path)?;
                        }
                        None
                    };
                    let passed = result.raw_mis_match_percentage <= pair.mis_match_threshold
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
                        pair.file_name,
                        status_text,
                    );
                    pair.diff = Some(build_diff(&result));
                    if !result.is_same_dimensions {
                        pair.engine_error_msg = Some(format!(
                            "Dimensions differ: reference {}x{} vs test {}x{}",
                            result.ref_width,
                            result.ref_height,
                            result.test_width,
                            result.test_height
                        ));
                    }
                    report_tests.push(ReportTest {
                        pair,
                        status: status.to_string(),
                    });
                }
                ComparisonOutcome::Failed(error) => {
                    let mut pair = job.pair;
                    eprintln!(
                        "    {} Failed to compare {}: {}",
                        "ERROR:".red(),
                        pair.file_name,
                        error
                    );
                    pair.engine_error_msg = Some(format!("Comparison error: {}", error));
                    report_tests.push(ReportTest {
                        pair,
                        status: "fail".to_string(),
                    });
                }
            },
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
    write_file_atomically(&report_path, report_json.as_bytes())?;

    // Generate config.js for Regressify React UI
    let config_js = format!("report({});", report_json);
    let config_path = html_report_dir.join("config.js");
    write_file_atomically(&config_path, config_js.as_bytes())?;

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

    Ok(())
}

/// Promote test screenshots to reference (approve).
pub fn approve_screenshots(suite_dir: &Path, requested_run_id: Option<&str>) -> Result<()> {
    let bitmaps_test_dir = suite_dir.join("bitmaps_test");
    let reference_dir = suite_dir.join("bitmaps_reference");

    if !bitmaps_test_dir.exists() {
        eprintln!("  {}", "No test screenshots found, skipping.".yellow());
        return Ok(());
    }

    let selected_run = match select_capture_run(&bitmaps_test_dir, requested_run_id)? {
        Some(run) => run,
        None => {
            eprintln!("  {}", "No test run directories found, skipping.".yellow());
            return Ok(());
        }
    };

    let latest_test_dir = bitmaps_test_dir.join(&selected_run);

    eprintln!("  Approving screenshots from run: {}", selected_run.cyan());

    let screenshots = validate_capture_run_inventory(&latest_test_dir)?;

    create_dir_all(&reference_dir)?;

    let mut approved_count = 0;
    for file_name in screenshots {
        let src = latest_test_dir.join(&file_name);
        if read_error_marker(&src).is_some() {
            eprintln!(
                "    {} Skipped failed capture: {}",
                "SKIP:".yellow(),
                file_name
            );
            continue;
        }
        let dst = reference_dir.join(&file_name);
        copy_file_atomically(&src, &dst)?;

        // Promote the error-marker sidecar so the reference retains its
        // failure flag; clear any stale marker when the new capture is good.
        let marker_src = crate::test::capture::error_marker_path(&src);
        let marker_dst = crate::test::capture::error_marker_path(&dst);
        if marker_src.exists() {
            copy_file_atomically(&marker_src, &marker_dst)?;
        } else if marker_dst.exists() {
            fs::remove_file(&marker_dst)?;
        }

        approved_count += 1;
        eprintln!("    Approved: {}", file_name.green());
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn png_bytes(red: u8) -> Vec<u8> {
        lodepng::encode32(&[red, 20, 30, 255], 1, 1).unwrap()
    }

    #[test]
    fn regenerating_report_ignores_prior_diff_images() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        let screenshot_name = "home__FINAL.png";
        fs::write(reference_dir.join(screenshot_name), png_bytes(10)).unwrap();
        fs::write(test_dir.join(screenshot_name), png_bytes(200)).unwrap();

        generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap();
        generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap();

        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(test_dir.join("report.json")).unwrap()).unwrap();
        let tests = report["tests"].as_array().unwrap();

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0]["status"], "fail");
        assert!(test_dir.join("home__FINAL_diff.png").exists());
    }

    #[test]
    fn report_removes_stale_diff_when_capture_becomes_identical() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        let screenshot_name = "home__FINAL.png";
        fs::write(reference_dir.join(screenshot_name), png_bytes(10)).unwrap();
        fs::write(test_dir.join(screenshot_name), png_bytes(200)).unwrap();
        generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap();
        let diff_path = test_dir.join("home__FINAL_diff.png");
        assert!(diff_path.exists());

        fs::write(test_dir.join(screenshot_name), png_bytes(10)).unwrap();
        generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap();

        assert!(!diff_path.exists());
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(test_dir.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["tests"][0]["status"], "pass");
        assert!(report["tests"][0]["pair"].get("diffImage").is_none());
    }

    #[test]
    fn report_includes_reference_and_test_page_urls_from_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        let screenshot_name = "home__FINAL.png";
        fs::write(reference_dir.join(screenshot_name), png_bytes(10)).unwrap();
        fs::write(test_dir.join(screenshot_name), png_bytes(10)).unwrap();
        fs::write(
            test_dir.join(REPORT_URLS_FILE),
            serde_json::to_vec(&ReportUrlsManifest {
                version: REPORT_URLS_VERSION,
                captures: BTreeMap::from([(
                    "home__FINAL".to_string(),
                    ReportPageUrls {
                        reference_url: "https://www.example.com/home".to_string(),
                        url: "https://test.example.com/home".to_string(),
                    },
                )]),
            })
            .unwrap(),
        )
        .unwrap();

        generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap();

        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(test_dir.join("report.json")).unwrap()).unwrap();
        assert_eq!(
            report["tests"][0]["pair"]["referenceUrl"],
            "https://www.example.com/home"
        );
        assert_eq!(
            report["tests"][0]["pair"]["url"],
            "https://test.example.com/home"
        );
        let config = fs::read_to_string(suite_dir.join("html_report/config.js")).unwrap();
        assert!(config.contains("\"referenceUrl\": \"https://www.example.com/home\""));
        assert!(config.contains("\"url\": \"https://test.example.com/home\""));
    }

    #[test]
    fn parallel_comparison_preserves_manifest_order() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        for (index, name) in ["alpha__FINAL.png", "beta__FINAL.png", "gamma__FINAL.png"]
            .iter()
            .enumerate()
        {
            let png = png_bytes((index as u8) + 10);
            fs::write(reference_dir.join(name), &png).unwrap();
            fs::write(test_dir.join(name), png).unwrap();
        }
        fs::write(
            test_dir.join(REPORT_ORDER_FILE),
            serde_json::to_vec(&["gamma", "alpha", "beta"]).unwrap(),
        )
        .unwrap();

        generate_comparison_report(&suite_dir, "basic", "run", 0.0, 2).unwrap();

        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(test_dir.join("report.json")).unwrap()).unwrap();
        let file_names: Vec<_> = report["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|test| test["pair"]["fileName"].as_str().unwrap())
            .collect();

        assert_eq!(
            file_names,
            vec!["gamma__FINAL.png", "alpha__FINAL.png", "beta__FINAL.png"]
        );
    }

    #[test]
    fn approve_replaces_reference_and_clears_stale_error_marker() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        let screenshot_name = "home__FINAL.png";
        let reference = reference_dir.join(screenshot_name);
        fs::write(&reference, png_bytes(10)).unwrap();
        fs::write(
            crate::test::capture::error_marker_path(&reference),
            br#"{"error":true,"message":"old capture failed"}"#,
        )
        .unwrap();
        fs::write(test_dir.join(screenshot_name), png_bytes(200)).unwrap();

        approve_screenshots(&suite_dir, None).unwrap();

        assert_eq!(fs::read(reference).unwrap(), png_bytes(200));
        assert!(
            !crate::test::capture::error_marker_path(&reference_dir.join(screenshot_name)).exists()
        );
    }

    #[test]
    fn approve_skips_capture_with_malformed_error_marker() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        let screenshot_name = "home__FINAL.png";
        let capture = test_dir.join(screenshot_name);
        fs::write(&capture, png_bytes(200)).unwrap();
        fs::write(
            crate::test::capture::error_marker_path(&capture),
            b"invalid marker",
        )
        .unwrap();

        approve_screenshots(&suite_dir, None).unwrap();

        assert!(!reference_dir.join(screenshot_name).exists());
    }

    #[test]
    fn malformed_report_manifests_fail_generation() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();
        fs::write(reference_dir.join("home__FINAL.png"), png_bytes(10)).unwrap();
        fs::write(test_dir.join("home__FINAL.png"), png_bytes(10)).unwrap();

        fs::write(test_dir.join(REPORT_ORDER_FILE), b"not json").unwrap();
        let error = generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap_err();
        assert!(error.to_string().contains("Invalid report order manifest"));

        fs::remove_file(test_dir.join(REPORT_ORDER_FILE)).unwrap();
        fs::write(test_dir.join(REPORT_THRESHOLDS_FILE), b"not json").unwrap();
        let error = generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Invalid report threshold manifest")
        );

        fs::remove_file(test_dir.join(REPORT_THRESHOLDS_FILE)).unwrap();
        fs::write(test_dir.join(REPORT_ORDER_FILE), br#"["home","home"]"#).unwrap();
        let error = generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap_err();
        assert!(error.to_string().contains("duplicate or empty key"));

        fs::remove_file(test_dir.join(REPORT_ORDER_FILE)).unwrap();
        fs::write(test_dir.join(REPORT_THRESHOLDS_FILE), br#"{"home":101}"#).unwrap();
        let error = generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap_err();
        assert!(error.to_string().contains("finite value between 0 and 100"));
    }

    #[test]
    fn complete_manifest_rejects_missing_expected_captures() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        fs::write(reference_dir.join("available__FINAL.png"), png_bytes(10)).unwrap();
        fs::write(test_dir.join("available__FINAL.png"), png_bytes(10)).unwrap();
        fs::write(
            test_dir.join(REPORT_ORDER_FILE),
            serde_json::to_vec(&ReportOrderManifest {
                version: REPORT_ORDER_VERSION,
                captures: vec!["available__FINAL".to_string(), "missing__FINAL".to_string()],
            })
            .unwrap(),
        )
        .unwrap();

        let error = generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap_err();

        assert!(error.to_string().contains("missing 1 screenshot"));
        assert!(error.to_string().contains("missing__FINAL"));
    }

    #[test]
    fn complete_manifest_rejects_unexpected_captures_before_approval() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        fs::write(reference_dir.join("home__FINAL.png"), png_bytes(10)).unwrap();
        fs::write(test_dir.join("home__FINAL.png"), png_bytes(20)).unwrap();
        fs::write(test_dir.join("unexpected__FINAL.png"), png_bytes(30)).unwrap();
        fs::write(
            test_dir.join(REPORT_ORDER_FILE),
            serde_json::to_vec(&ReportOrderManifest {
                version: REPORT_ORDER_VERSION,
                captures: vec!["home__FINAL".to_string()],
            })
            .unwrap(),
        )
        .unwrap();

        let error = approve_screenshots(&suite_dir, None).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("contains 1 unexpected screenshot")
        );
        assert_eq!(
            fs::read(reference_dir.join("home__FINAL.png")).unwrap(),
            png_bytes(10)
        );
    }

    #[test]
    fn capture_run_selection_honors_explicit_id_before_latest_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let runs = temp.path().join("bitmaps_test");
        fs::create_dir_all(runs.join("20260913_120000_000000001")).unwrap();
        fs::create_dir_all(runs.join("z-newer-name")).unwrap();

        assert_eq!(
            select_capture_run(&runs, Some("20260913_120000_000000001")).unwrap(),
            Some("20260913_120000_000000001".to_string())
        );
        assert_eq!(
            select_capture_run(&runs, None).unwrap(),
            Some("z-newer-name".to_string())
        );
        assert!(
            select_capture_run(&runs, Some("missing"))
                .unwrap_err()
                .to_string()
                .contains("Requested capture run 'missing' does not exist")
        );
    }

    #[cfg(unix)]
    #[test]
    fn explicit_capture_run_selection_rejects_symlinked_directories() {
        let temp = tempfile::tempdir().unwrap();
        let runs = temp.path().join("bitmaps_test");
        let outside = temp.path().join("outside-run");
        fs::create_dir_all(&runs).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, runs.join("linked-run")).unwrap();

        let error = select_capture_run(&runs, Some("linked-run")).unwrap_err();

        assert!(error.to_string().contains("not a real directory"));
    }

    #[cfg(unix)]
    #[test]
    fn capture_discovery_rejects_non_utf8_file_and_run_names() {
        use std::os::unix::ffi::OsStringExt;

        let temp = tempfile::tempdir().unwrap();
        let capture_dir = temp.path().join("captures");
        fs::create_dir_all(&capture_dir).unwrap();
        let invalid_file = std::ffi::OsString::from_vec(b"capture-\xff.png".to_vec());
        fs::write(capture_dir.join(invalid_file), []).unwrap();

        let file_error = collect_capture_screenshots(&capture_dir).unwrap_err();
        assert!(file_error.to_string().contains("not valid UTF-8"));

        let run_dir = temp.path().join("runs");
        fs::create_dir_all(&run_dir).unwrap();
        let invalid_run = std::ffi::OsString::from_vec(b"run-\xff".to_vec());
        fs::create_dir(run_dir.join(invalid_run)).unwrap();

        let run_error = select_capture_run(&run_dir, None).unwrap_err();
        assert!(run_error.to_string().contains("not valid UTF-8"));
    }

    #[test]
    fn intermediate_screenshots_use_scenario_order_and_threshold_fallbacks() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("basic");
        let reference_dir = suite_dir.join("bitmaps_reference");
        let test_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&reference_dir).unwrap();
        fs::create_dir_all(&test_dir).unwrap();

        fs::write(reference_dir.join("home__$after-login.png"), png_bytes(10)).unwrap();
        fs::write(test_dir.join("home__$after-login.png"), png_bytes(200)).unwrap();
        fs::write(reference_dir.join("home__FINAL.png"), png_bytes(10)).unwrap();
        fs::write(test_dir.join("home__FINAL.png"), png_bytes(10)).unwrap();
        fs::write(
            test_dir.join(REPORT_ORDER_FILE),
            serde_json::to_vec(&["home__FINAL"]).unwrap(),
        )
        .unwrap();
        fs::write(
            test_dir.join(REPORT_THRESHOLDS_FILE),
            serde_json::to_vec(&HashMap::from([("home".to_string(), 100.0)])).unwrap(),
        )
        .unwrap();

        generate_comparison_report(&suite_dir, "basic", "run", 0.0, 1).unwrap();

        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(test_dir.join("report.json")).unwrap()).unwrap();
        let tests = report["tests"].as_array().unwrap();
        assert_eq!(tests[0]["pair"]["fileName"], "home__$after-login.png");
        assert_eq!(tests[0]["status"], "pass");
        assert_eq!(tests[0]["pair"]["misMatchThreshold"], 100.0);
    }

    #[test]
    fn report_selector_describes_capture_target_from_filename() {
        assert_eq!(report_selector("home__FINAL.png"), "document");
        assert_eq!(report_selector("home__VIEWPORT.png"), "viewport");
        assert_eq!(
            report_selector("home__$after-login.png"),
            "save:after-login"
        );
        assert_eq!(
            report_selector("home__SELECTOR-main-hero-abc123.png"),
            "SELECTOR-main-hero-abc123"
        );
    }
}
