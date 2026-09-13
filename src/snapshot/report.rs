use anyhow::Result;
use colored::*;
use std::path::Path;

use crate::cli::args::SnapshotArgs;
use crate::helpers::{IOExtensions, write_file_atomically};
use crate::snapshot::helpers::get_asset_version;
use crate::types::HtmlReportSummary;

pub fn generate_html_report_summary(
    regressify_dir: &Path,
    tera: &tera::Tera,
    summaries: &[HtmlReportSummary],
    args: &SnapshotArgs,
) -> Result<()> {
    let mut total_tests = 0;
    let mut total_passed = 0;
    let mut total_failed = 0;

    for summary in summaries {
        total_tests += summary.total_tests;
        total_passed += summary.total_passed;
        total_failed += summary.total_failed;
    }

    let percent = if total_tests == 0 {
        0.0
    } else {
        (total_passed as f64 / total_tests as f64 * 10000.0).round() / 100.0
    };

    let passed_class = if total_failed == 0 { "success" } else { "" };

    let failed_class = if total_failed > 0 {
        "danger"
    } else {
        "success"
    };

    let mut context = tera::Context::new();

    context.insert("summaries", summaries);
    context.insert("total_tests", &total_tests);
    context.insert("total_passed", &total_passed);
    context.insert("total_failed", &total_failed);
    context.insert("percent", &percent);
    context.insert("passed_class", passed_class);
    context.insert("failed_class", failed_class);

    context.insert("title", &args.title);
    context.insert("heading", &args.heading);

    let parser = pulldown_cmark::Parser::new(&args.subheading);
    let mut subheading = String::new();
    pulldown_cmark::html::push_html(&mut subheading, parser);
    context.insert("subheading", subheading.trim());

    generate_assets_file(
        regressify_dir,
        &mut context,
        "styles.css",
        include_bytes!("data/styles.css"),
        Some("styles_hash"),
    )?;

    generate_assets_file(
        regressify_dir,
        &mut context,
        "script.js",
        include_bytes!("data/script.js"),
        Some("script_hash"),
    )?;

    let replaced_html = tera.render("report_template.html", &context)?;

    let report_path = regressify_dir.join("index.html");
    write_file_atomically(&report_path, replaced_html.as_bytes())?;
    eprintln!(
        "Snapshot report summary generated at: {}",
        report_path.normalize_string(false)?.cyan()
    );

    let swa_config = include_bytes!("data/staticwebapp.config.json");
    let swa_config_path = regressify_dir.join("staticwebapp.config.json");
    write_file_atomically(&swa_config_path, swa_config)?;

    Ok(())
}

pub fn generate_assets_file(
    regressify_dir: &Path,
    context: &mut tera::Context,
    path: &str,
    content_bytes: &[u8],
    version_name: Option<&str>,
) -> Result<()> {
    if let Some(version_name) = version_name {
        let script_hash = get_asset_version(content_bytes, 20);
        context.insert(version_name, &script_hash);
    }

    write_shared_file(regressify_dir, path, content_bytes)?;

    Ok(())
}

pub fn generate_shared_files(regressify_dir: &Path) -> Result<()> {
    write_shared_file(
        regressify_dir,
        ".gitignore",
        include_bytes!("data/gitignore"),
    )?;

    write_shared_file(
        regressify_dir,
        "lato-bold-webfont.woff",
        include_bytes!("data/lato-bold-webfont.woff"),
    )?;
    write_shared_file(
        regressify_dir,
        "lato-bold-webfont.woff2",
        include_bytes!("data/lato-bold-webfont.woff2"),
    )?;
    write_shared_file(
        regressify_dir,
        "Lato-Bold.ttf",
        include_bytes!("data/Lato-Bold.ttf"),
    )?;
    write_shared_file(
        regressify_dir,
        "lato-regular-webfont.woff",
        include_bytes!("data/lato-regular-webfont.woff"),
    )?;
    write_shared_file(
        regressify_dir,
        "lato-regular-webfont.woff2",
        include_bytes!("data/lato-regular-webfont.woff2"),
    )?;
    write_shared_file(
        regressify_dir,
        "Lato-Regular.ttf",
        include_bytes!("data/Lato-Regular.ttf"),
    )?;

    write_shared_file(regressify_dir, "diff.js", include_bytes!("data/diff.js"))?;
    write_shared_file(
        regressify_dir,
        "diverged.js",
        include_bytes!("data/diverged.js"),
    )?;
    write_shared_file(
        regressify_dir,
        "divergedWorker.js",
        include_bytes!("data/divergedWorker.js"),
    )?;

    write_shared_file(
        regressify_dir,
        "favicon.ico",
        include_bytes!("data/favicon.ico"),
    )?;

    write_shared_file(
        regressify_dir,
        "favicon.svg",
        include_bytes!("data/favicon.svg"),
    )?;

    write_shared_file(
        regressify_dir,
        "favicon-16x16.png",
        include_bytes!("data/favicon-16x16.png"),
    )?;

    write_shared_file(
        regressify_dir,
        "favicon-32x32.png",
        include_bytes!("data/favicon-32x32.png"),
    )?;

    write_shared_file(
        regressify_dir,
        "apple-touch-icon.png",
        include_bytes!("data/apple-touch-icon.png"),
    )?;

    Ok(())
}

pub fn write_shared_file(regressify_dir: &Path, file_name: &str, content: &[u8]) -> Result<()> {
    let file_path = regressify_dir.join(file_name);
    write_file_atomically(&file_path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_report_files_replace_existing_content_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("script.js");
        std::fs::write(&path, b"old").unwrap();

        write_shared_file(temp.path(), "script.js", b"new").unwrap();

        assert_eq!(std::fs::read(path).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}
