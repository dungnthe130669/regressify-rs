use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;

use crate::helpers::IOExtensions;
use crate::types::HtmlReportSummary;

pub fn generate_html_report_summary(
    backstop_dir: &Path,
    tera: &tera::Tera,
    summaries: &[HtmlReportSummary],
) -> Result<()> {
    let mut total_tests = 0;
    let mut total_passed = 0;
    let mut total_failed = 0;

    for summary in summaries {
        total_tests += summary.total_tests;
        total_passed += summary.total_passed;
        total_failed += summary.total_failed;
    }

    let mut context = tera::Context::new();

    context.insert("summaries", summaries);
    context.insert("total_tests", &total_tests);
    context.insert("total_passed", &total_passed);
    context.insert("total_failed", &total_failed);

    let replaced_html = tera.render("report_template.html", &context)?;

    let report_path = backstop_dir.join("index.html");
    fs::write(&report_path, replaced_html)?;
    eprintln!(
        "{}",
        format!(
            "Snapshot report summary generated at: {:?}",
            report_path.normalize_string(false)?
        )
        .cyan()
    );

    let swa_config = include_bytes!("_staticwebapp.config.json");
    let swa_config_path = backstop_dir.join("staticwebapp.config.json");
    fs::write(&swa_config_path, swa_config)?;

    Ok(())
}
