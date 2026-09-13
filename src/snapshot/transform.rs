use anyhow::{Context, Result};
use colored::*;
use regex::Regex;
use std::collections::BTreeMap;
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
    let mut modified_config_text = config_text.clone();

    // Readdir and filter directories
    let entries = fs::read_dir(&bitmap_test_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
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

        // Regressify uses both forward and backslashes in config
        let win_path = format!("bitmaps_test\\\\{}", sub_dir_name);
        let unix_path = format!("bitmaps_test/{}", sub_dir_name);

        if !config_text.contains(&unix_path) && !config_text.contains(&win_path) {
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
            "report({test: '../bitmaps_test/run/home__FINAL.png'});",
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
