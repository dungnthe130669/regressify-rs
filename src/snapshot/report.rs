use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;

use crate::types::HtmlReportSummary;

pub fn generate_html_report_summary(
    backstop_dir: &Path,
    summaries: &[HtmlReportSummary],
) -> Result<()> {
    let mut rows_html = String::new();

    let mut total_tests = 0;
    let mut total_passed = 0;
    let mut total_failed = 0;

    for summary in summaries {
        total_tests += summary.total_tests;
        total_passed += summary.total_passed;
        total_failed += summary.total_failed;

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

    let template = include_str!("_report_template.html"); // Or use a raw string

    let passed_class = if total_passed == total_tests {
        "success"
    } else {
        ""
    };

    let failed_class = if total_failed > 0 { "danger" } else { "" };

    let replaced_html = template
        .replace("<!-- REPORT_PLACEHOLDER -->", &rows_html)
        .replace("<!-- TOTAL_TESTS -->", &total_tests.to_string())
        .replace("<!-- TOTAL_PASSED -->", &total_passed.to_string())
        .replace("<!-- TOTAL_FAILED -->", &total_failed.to_string())
        .replace("[[ PASSED_CLASS ]]", passed_class)
        .replace("[[ FAILED_CLASS ]]", failed_class);

    let report_path = backstop_dir.join("index.html");
    fs::write(&report_path, replaced_html)?;
    println!(
        "{}",
        format!("Snapshot report summary generated at: {:?}", report_path).blue()
    );

    let swa_config = include_bytes!("_staticwebapp.config.json");
    let swa_config_path = backstop_dir.join("staticwebapp.config.json");
    fs::write(&swa_config_path, swa_config)?;

    Ok(())
}
