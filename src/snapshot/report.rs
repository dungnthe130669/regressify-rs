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

    let swa_config = include_bytes!("data/staticwebapp.config.json");
    let swa_config_path = backstop_dir.join("staticwebapp.config.json");
    fs::write(&swa_config_path, swa_config)?;

    Ok(())
}

pub fn generate_shared_files(backstop_dir: &Path) -> Result<()> {
    write_shared_file(
        backstop_dir,
        "lato-bold-webfont.woff",
        include_bytes!("data/lato-bold-webfont.woff"),
    )?;
    write_shared_file(
        backstop_dir,
        "lato-bold-webfont.woff2",
        include_bytes!("data/lato-bold-webfont.woff2"),
    )?;
    write_shared_file(
        backstop_dir,
        "Lato-Bold.ttf",
        include_bytes!("data/Lato-Bold.ttf"),
    )?;
    write_shared_file(
        backstop_dir,
        "lato-regular-webfont.woff",
        include_bytes!("data/lato-regular-webfont.woff"),
    )?;
    write_shared_file(
        backstop_dir,
        "lato-regular-webfont.woff2",
        include_bytes!("data/lato-regular-webfont.woff2"),
    )?;
    write_shared_file(
        backstop_dir,
        "Lato-Regular.ttf",
        include_bytes!("data/Lato-Regular.ttf"),
    )?;

    write_shared_file(backstop_dir, "diff.js", include_bytes!("data/diff.js"))?;
    write_shared_file(
        backstop_dir,
        "diverged.js",
        include_bytes!("data/diverged.js"),
    )?;
    write_shared_file(
        backstop_dir,
        "divergedWorker.js",
        include_bytes!("data/divergedWorker.js"),
    )?;

    Ok(())
}

pub fn write_shared_file(backstop_dir: &Path, file_name: &str, content: &[u8]) -> Result<()> {
    let file_path = backstop_dir.join(file_name);
    fs::write(&file_path, content)?;
    Ok(())
}
