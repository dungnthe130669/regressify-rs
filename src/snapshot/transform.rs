use anyhow::{Context, Result};
use colored::*;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    helpers::write_file_atomically,
    snapshot::helpers::get_asset_version,
    types::{HtmlReportSummary, RegressifyReport},
};

const STALE_HTML_REPORT_ARTIFACTS: &[&str] = &[
    "assets",
    "diff.js",
    "diverged.js",
    "divergedWorker.js",
    "index_bundle.js.LICENSE.txt",
];

fn cache_report_image_hash(
    suite_dir: &Path,
    html_report_dir: &Path,
    hashes: &mut BTreeMap<String, String>,
    relative_path: &Option<String>,
) -> Result<()> {
    let Some(relative_path) = relative_path else {
        return Ok(());
    };
    if hashes.contains_key(relative_path) {
        return Ok(());
    }

    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        anyhow::bail!("Report image path '{}' must be relative", relative_path);
    }
    let candidate = html_report_dir.join(relative);
    if !candidate.exists() {
        return Ok(());
    }

    let canonical_suite_dir = suite_dir.canonicalize().with_context(|| {
        format!(
            "Failed to resolve snapshot suite directory '{}'",
            suite_dir.display()
        )
    })?;
    let canonical_candidate = candidate.canonicalize().with_context(|| {
        format!(
            "Failed to resolve report image path '{}'",
            candidate.display()
        )
    })?;
    if !canonical_candidate.starts_with(&canonical_suite_dir) {
        anyhow::bail!(
            "Report image path '{}' escapes snapshot suite '{}'",
            relative_path,
            suite_dir.display()
        );
    }

    let content = fs::read(&canonical_candidate).with_context(|| {
        format!(
            "Failed to read report image '{}' for hashing",
            canonical_candidate.display()
        )
    })?;
    hashes.insert(relative_path.clone(), get_asset_version(&content, 20));
    Ok(())
}

/// Replace a report image literal whether it is unversioned or has a hash from
/// an earlier snapshot generation.
fn version_report_image_path(config_text: String, file_path: &str, hash: &str) -> Result<String> {
    let mut versioned = config_text;
    for path in [file_path.to_string(), file_path.replace('\\', "\\\\")] {
        for quote in ["'", "\""] {
            let expression = format!(
                "{}(?:\\?v=[^{}]*)?{}",
                regex::escape(&format!("{quote}{path}")),
                regex::escape(quote),
                regex::escape(quote),
            );
            let pattern = Regex::new(&expression).map_err(|error| {
                anyhow::anyhow!("Failed to build report image version pattern: {}", error)
            })?;
            let replacement = format!("{quote}{path}?v={hash}{quote}");
            versioned = pattern.replace_all(&versioned, replacement).into_owned();
        }
    }
    Ok(versioned)
}

/// Parse the generated `report(<JSON>);` configuration before it is trusted to
/// identify capture directories eligible for cleanup.
fn parse_report_config(config_text: &str) -> Result<RegressifyReport> {
    let config_json = config_text
        .trim()
        .strip_prefix("report(")
        .and_then(|content| content.strip_suffix(");"))
        .ok_or_else(|| anyhow::anyhow!("Invalid snapshot config.js wrapper; expected report(<JSON>);"))?;
    serde_json::from_str(config_json).context("Invalid snapshot config.js report data")
}

/// Return the capture run directories referenced by the generated report.
/// Query hashes added by snapshot generation and legacy Windows separators are
/// normalized before extracting the run component.
fn referenced_capture_runs(report: &RegressifyReport) -> Result<BTreeSet<String>> {
    let mut runs = BTreeSet::new();
    for (index, test) in report.tests.iter().enumerate() {
        let path = test.pair.test.as_deref().ok_or_else(|| {
            anyhow::anyhow!("Snapshot config test {} has no capture image path", index + 1)
        })?;
        let normalized = path
            .split_once('?')
            .map(|(path, _)| path)
            .unwrap_or(path)
            .replace('\\', "/");
        let relative = normalized.strip_prefix("../bitmaps_test/").ok_or_else(|| {
            anyhow::anyhow!(
                "Snapshot config test {} has an invalid capture image path '{}'",
                index + 1,
                path
            )
        })?;
        let mut components = relative.split('/');
        let run_id = components.next().filter(|component| {
            !component.is_empty() && *component != "." && *component != ".."
        });
        let file_name = components.next().filter(|component| !component.is_empty());
        if run_id.is_none() || file_name.is_none() || components.next().is_some() {
            anyhow::bail!(
                "Snapshot config test {} has an invalid capture image path '{}'",
                index + 1,
                path
            );
        }
        runs.insert(run_id.unwrap().to_string());
    }
    if runs.is_empty() {
        anyhow::bail!("Snapshot config.js contains no capture image paths; refusing cleanup");
    }
    Ok(runs)
}

pub fn process_test_suite(
    regressify_dir: &Path,
    id: &str,
    hashes: &mut BTreeMap<String, String>,
    tera: &tera::Tera,
    bundle_hash: &str,
) -> Result<Option<HtmlReportSummary>> {
    let test_dir = regressify_dir.join(id);
    if !test_dir.exists() {
        eprintln!(
            "{}",
            format!("Test directory does not exist: {:?}", test_dir).red()
        );
        return Ok(None);
    }

    let html_report_dir = test_dir.join("html_report");
    let bitmap_test_dir = test_dir.join("bitmaps_test");

    if !html_report_dir.exists() || !bitmap_test_dir.exists() {
        return Ok(None);
    }

    for file_name in STALE_HTML_REPORT_ARTIFACTS {
        let path = html_report_dir.join(file_name);
        if path.is_dir() {
            fs::remove_dir_all(&path).with_context(|| {
                format!(
                    "Failed to remove stale report directory '{}'",
                    path.display()
                )
            })?;
        } else if path.is_file() {
            fs::remove_file(&path).with_context(|| {
                format!("Failed to remove stale report file '{}'", path.display())
            })?;
        }
    }

    let config_path = html_report_dir.join("config.js");
    let html_index_path = html_report_dir.join("index.html");

    if !config_path.exists() {
        return Ok(None);
    }

    let config_text = fs::read_to_string(&config_path)?;
    let referenced_runs = referenced_capture_runs(&parse_report_config(&config_text)?)?;
    let mut modified_config_text = config_text.clone();

    // Remove only run directories explicitly referenced by neither the parsed
    // config nor a validated report path.
    let entries = fs::read_dir(&bitmap_test_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let Ok(sub_dir_name) = entry.file_name().into_string() else {
            eprintln!(
                "{}",
                format!(
                    "Skipping bitmap subdirectory with non-UTF8 name: {:?}",
                    entry.file_name()
                )
                .yellow()
            );
            continue;
        };

        if !referenced_runs.contains(&sub_dir_name) {
            fs::remove_dir_all(&path).with_context(|| {
                format!(
                    "Failed to remove unreferenced snapshot directory '{}'",
                    path.display()
                )
            })?;
            eprintln!(
                "{}",
                format!("Removed unreferenced snapshot directory: {:?}", path).yellow()
            );
        } else {
            let report_json_path = path.join("report.json");
            if report_json_path.exists() {
                let report_content = fs::read_to_string(&report_json_path)?;
                let report: RegressifyReport = serde_json::from_str(&report_content)?;

                let pass_count = report.tests.iter().filter(|t| t.status == "pass").count();
                let fail_count = report.tests.iter().filter(|t| t.status == "fail").count();

                for test in &report.tests {
                    cache_report_image_hash(
                        &test_dir,
                        &html_report_dir,
                        hashes,
                        &test.pair.reference,
                    )?;
                    cache_report_image_hash(&test_dir, &html_report_dir, hashes, &test.pair.test)?;
                    cache_report_image_hash(
                        &test_dir,
                        &html_report_dir,
                        hashes,
                        &test.pair.diff_image,
                    )?;
                }

                // Apply hash versioning to config.js.
                for (file_path, hash) in hashes.iter() {
                    modified_config_text =
                        version_report_image_path(modified_config_text, file_path, hash)?;
                }
                write_file_atomically(&config_path, modified_config_text.as_bytes())?;

                // Handle index.html config.js versioning
                let config_hash = get_asset_version(modified_config_text.as_bytes(), 20);

                let mut context = tera::Context::new();
                context.insert("test_suite_name", id);
                context.insert("config_hash", &config_hash);
                context.insert("index_bundle_hash", &bundle_hash);

                let replaced_html = tera.render("test_suite_report.html", &context)?;

                write_file_atomically(&html_index_path, replaced_html.as_bytes())?;

                eprintln!(
                    "Test suite: {}, Passed: {}, Failed: {}",
                    id.cyan(),
                    pass_count.to_string().green(),
                    if fail_count > 0 {
                        fail_count.to_string().red()
                    } else {
                        fail_count.to_string().normal()
                    }
                );

                let status = if fail_count > 0 { "failed" } else { "passed" };
                let percent = if !report.tests.is_empty() {
                    let percent = (pass_count as f32 / report.tests.len() as f32) * 100.0;
                    format!("{:.2}", percent)
                } else {
                    "0".to_string()
                };

                let passed_class = if fail_count == 0 {
                    Some("success")
                } else {
                    None
                };
                let failed_class = if fail_count == 0 {
                    None
                } else {
                    Some("danger")
                };

                return Ok(Some(HtmlReportSummary {
                    id: report.id,
                    total_tests: report.tests.len(),
                    total_passed: pass_count,
                    total_failed: fail_count,
                    has_engine_errors: report
                        .tests
                        .iter()
                        .any(|t| t.pair.engine_error_msg.is_some()),
                    percent,
                    status,
                    passed_class,
                    failed_class,
                }));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_report_json_is_a_snapshot_error() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("suite");
        let html_report_dir = suite_dir.join("html_report");
        let run_dir = suite_dir.join("bitmaps_test").join("run");
        fs::create_dir_all(&html_report_dir).unwrap();
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(
            html_report_dir.join("config.js"),
            r#"report({"id":"suite","tests":[{"pair":{"test":"../bitmaps_test/run/home__FINAL.png"},"status":"pass"}]});"#,
        )
        .unwrap();
        fs::write(run_dir.join("report.json"), "not json").unwrap();

        let error = process_test_suite(
            temp.path(),
            "suite",
            &mut BTreeMap::new(),
            &tera::Tera::default(),
            "bundle",
        )
        .unwrap_err();

        assert!(error.to_string().contains("expected ident"));
    }

    #[test]
    fn malformed_config_does_not_remove_capture_directories() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("suite");
        let html_report_dir = suite_dir.join("html_report");
        let current_run = suite_dir.join("bitmaps_test/current");
        let prior_run = suite_dir.join("bitmaps_test/prior");
        fs::create_dir_all(&html_report_dir).unwrap();
        fs::create_dir_all(&current_run).unwrap();
        fs::create_dir_all(&prior_run).unwrap();
        fs::write(html_report_dir.join("config.js"), "corrupt report data").unwrap();

        let error = process_test_suite(
            temp.path(),
            "suite",
            &mut BTreeMap::new(),
            &tera::Tera::default(),
            "bundle",
        )
        .unwrap_err();

        assert!(error.to_string().contains("Invalid snapshot config.js wrapper"));
        assert!(current_run.exists());
        assert!(prior_run.exists());
    }

    #[test]
    fn valid_config_removes_only_unreferenced_capture_directories() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("suite");
        let html_report_dir = suite_dir.join("html_report");
        let current_run = suite_dir.join("bitmaps_test/current");
        let prior_run = suite_dir.join("bitmaps_test/prior");
        fs::create_dir_all(&html_report_dir).unwrap();
        fs::create_dir_all(&current_run).unwrap();
        fs::create_dir_all(&prior_run).unwrap();
        fs::write(
            html_report_dir.join("config.js"),
            r#"report({"id":"suite","tests":[{"pair":{"test":"../bitmaps_test/current/home__FINAL.png"},"status":"pass"}]});"#,
        )
        .unwrap();

        process_test_suite(
            temp.path(),
            "suite",
            &mut BTreeMap::new(),
            &tera::Tera::default(),
            "bundle",
        )
        .unwrap();

        assert!(current_run.exists());
        assert!(!prior_run.exists());
    }

    #[test]
    fn capture_run_extraction_accepts_versioned_windows_paths() {
        let report: RegressifyReport = serde_json::from_str(
            r#"{"id":"suite","tests":[{"pair":{"test":"..\\bitmaps_test\\run\\home.png?v=hash"},"status":"pass"}]}"#,
        )
        .unwrap();

        assert_eq!(
            referenced_capture_runs(&report).unwrap(),
            BTreeSet::from(["run".to_string()])
        );
    }

    #[test]
    fn report_image_hashing_accepts_suite_paths_and_rejects_escapes() {
        let temp = tempfile::tempdir().unwrap();
        let suite_dir = temp.path().join("suite");
        let html_report_dir = suite_dir.join("html_report");
        let reference_dir = suite_dir.join("bitmaps_reference");
        fs::create_dir_all(&html_report_dir).unwrap();
        fs::create_dir_all(&reference_dir).unwrap();
        fs::write(reference_dir.join("home.png"), b"reference").unwrap();
        fs::write(temp.path().join("outside.png"), b"outside").unwrap();

        let mut hashes = BTreeMap::new();
        cache_report_image_hash(
            &suite_dir,
            &html_report_dir,
            &mut hashes,
            &Some("../bitmaps_reference/home.png".to_string()),
        )
        .unwrap();
        assert!(hashes.contains_key("../bitmaps_reference/home.png"));

        let error = cache_report_image_hash(
            &suite_dir,
            &html_report_dir,
            &mut hashes,
            &Some("../../outside.png".to_string()),
        )
        .unwrap_err();
        assert!(error.to_string().contains("escapes snapshot suite"));
    }

    #[test]
    fn report_image_versions_replace_prior_cache_hashes() {
        let path = "../bitmaps_test/run/home__FINAL.png";
        let config = format!("report({{test: '{path}?v=old-hash', reference: \"{path}\"}});");

        let versioned = version_report_image_path(config, path, "new-hash").unwrap();

        assert_eq!(versioned.matches("?v=new-hash").count(), 2);
        assert!(!versioned.contains("old-hash"));
    }

    #[test]
    fn report_image_versions_handle_backslash_escaped_paths() {
        let path = r"..\bitmaps_test\run\home__FINAL.png";
        let config = r"report({test: '..\\bitmaps_test\\run\\home__FINAL.png?v=old-hash'});";

        let versioned = version_report_image_path(config.to_string(), path, "new-hash").unwrap();

        assert!(versioned.contains("?v=new-hash"));
        assert!(!versioned.contains("old-hash"));
    }
}
