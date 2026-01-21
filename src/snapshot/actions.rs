use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use colored::*;
use sha2::{Digest, Sha384};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::types::{BackstopReport, HtmlReportSummary};

fn get_asset_version(content: &[u8]) -> String {
    let mut hasher = Sha384::new();
    hasher.update(content);
    let result = hasher.finalize();
    // Node's 'base64url' (URL-safe, no padding)
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(result);
    encoded.chars().take(10).collect()
}

fn process_test_suite(
    backstop_dir: &Path,
    id: &str,
    hashes: &mut HashMap<String, String>,
) -> Result<Option<HtmlReportSummary>> {
    let test_dir = backstop_dir.join(id);
    if !test_dir.exists() {
        println!(
            "{}",
            format!("Test directory does not exist: {:?}", test_dir).red()
        );
        return Ok(None);
    }

    let html_report_dir = test_dir.join("html_report");
    let bitmap_test_dir = test_dir.join("bitmaps_test");

    if !html_report_dir.exists() {
        return Ok(None);
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
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let sub_dir_name = entry.file_name().into_string().unwrap();

        // Backstop uses both forward and backslashes in config
        let win_path = format!("bitmaps_test\\\\{}", sub_dir_name);
        let unix_path = format!("bitmaps_test/{}", sub_dir_name);

        if !config_text.contains(&unix_path) && !config_text.contains(&win_path) {
            fs::remove_dir_all(&path).ok();
            println!(
                "{}",
                format!("Removed unreferenced snapshot directory: {:?}", path).yellow()
            );
        } else {
            let report_json_path = path.join("report.json");
            if report_json_path.exists() {
                let report_content = fs::read_to_string(&report_json_path)?;
                let report: BackstopReport = serde_json::from_str(&report_content)?;

                let pass_count = report.tests.iter().filter(|t| t.status == "pass").count();
                let fail_count = report.tests.iter().filter(|t| t.status == "fail").count();

                for test in &report.tests {
                    let mut check_and_hash = |img_path_str: &Option<String>| {
                        if let Some(img_rel_path) = img_path_str {
                            let full_path = bitmap_test_dir.join(img_rel_path);
                            if full_path.exists() && !hashes.contains_key(img_rel_path) {
                                let content = fs::read(&full_path).unwrap_or_default();
                                let hash = get_asset_version(&content);
                                hashes.insert(img_rel_path.clone(), hash.clone());
                                println!("Calculated hash for image: {} => {}", img_rel_path, hash);
                            }
                        }
                    };

                    check_and_hash(&test.pair.reference);
                    check_and_hash(&test.pair.test);
                    check_and_hash(&test.pair.diff_image);
                }

                // Apply hash versioning to config.js
                for (file_path, hash) in hashes.iter() {
                    if modified_config_text.contains(file_path) {
                        let win_file_path = file_path.replace("\\", "\\\\");
                        let replacements = [
                            (
                                format!("'{}'", file_path),
                                format!("'{}?v={}'", file_path, hash),
                            ),
                            (
                                format!("\"{}\"", file_path),
                                format!("\"{}?v={}'", file_path, hash),
                            ),
                            (
                                format!("'{}'", win_file_path),
                                format!("'{}?v={}'", win_file_path, hash),
                            ),
                            (
                                format!("\"{}\"", win_file_path),
                                format!("\"{}?v={}'", win_file_path, hash),
                            ),
                        ];

                        for (old, new) in replacements {
                            modified_config_text = modified_config_text.replace(&old, &new);
                        }
                    }
                }
                fs::write(&config_path, &modified_config_text).ok();

                // Handle index.html config.js versioning
                let config_hash = get_asset_version(modified_config_text.as_bytes());
                if let Ok(html_content) = fs::read_to_string(&html_index_path) {
                    let updated_html = html_content
                        .replace("'config.js'", &format!("'config.js?v={}'", config_hash))
                        .replace("\"config.js\"", &format!("\"config.js?v={}\"", config_hash));
                    fs::write(&html_index_path, updated_html).ok();
                }

                println!(
                    "{}",
                    format!(
                        "Snapshot directory: {}, Passed: {}, Failed: {}",
                        sub_dir_name, pass_count, fail_count
                    )
                    .green()
                );

                return Ok(Some(HtmlReportSummary {
                    id: report.id,
                    total_tests: report.tests.len(),
                    total_passed: pass_count,
                    total_failed: fail_count,
                }));
            }
        }
    }

    Ok(None)
}

pub fn generate_html_report_summary(backstop_dir: &Path, summaries: &[HtmlReportSummary]) {
    let mut rows_html = String::new();

    for summary in summaries {
        let success_class = if summary.total_passed == summary.total_tests {
            "class=\"success\""
        } else {
            ""
        };
        let danger_class = if summary.total_failed > 0 {
            "class=\"danger\""
        } else {
            ""
        };

        rows_html.push_str(&format!(
            "<tr>
                <td><a href=\"./{id}/html_report/index.html\">{id}</a></td>
                <td>{total}</td>
                <td {s_class}>{passed}</td>
                <td {d_class}>{failed}</td>
            </tr>",
            id = summary.id,
            total = summary.total_tests,
            passed = summary.total_passed,
            failed = summary.total_failed,
            s_class = success_class,
            d_class = danger_class
        ));
    }

    let template = include_str!("report_template.html"); // Or use a raw string
    let final_html = template.replace("", &rows_html);

    let report_path = backstop_dir.join("index.html");
    fs::write(&report_path, final_html).expect("Unable to write summary report");
    println!(
        "{}",
        format!("Snapshot report summary generated at: {:?}", report_path).blue()
    );
}

fn snapshot(ids: Vec<String>, backstop_dir_name: &str) {
    let current_dir = std::env::current_dir().unwrap();
    let backstop_dir = current_dir.join(backstop_dir_name);

    if !backstop_dir.exists() {
        println!(
            "{}",
            format!("Backstop directory does not exist: {:?}", backstop_dir).red()
        );
        return;
    }

    let mut summaries = Vec::new();
    let mut hashes = HashMap::new();

    for id in ids {
        if let Ok(Some(summary)) = process_test_suite(&backstop_dir, &id, &mut hashes) {
            summaries.push(summary);
        }
    }

    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    generate_html_report_summary(&backstop_dir, &summaries);
}

pub fn snapshots(ids: Vec<String>) -> Result<()> {
    snapshot(ids, "backstop_data");

    Ok(())
}
