use anyhow::Result;
use colored::*;
use std::path::Path;

use crate::cli::args::SnapshotArgs;
use crate::config::WorkspaceConfig;
use crate::helpers::{IOExtensions, write_file_atomically};
use crate::snapshot::helpers::get_asset_version;
use crate::types::HtmlReportSummary;

pub fn generate_html_report_summary(
    regressify_dir: &Path,
    tera: &tera::Tera,
    summaries: &[HtmlReportSummary],
    args: &SnapshotArgs,
    workspace_config: &WorkspaceConfig,
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

    let title = args
        .title
        .as_deref()
        .or(workspace_config.snapshot_title.as_deref())
        .unwrap_or("Visual Tests Summary | Regressify");
    let heading = args
        .heading
        .as_deref()
        .or(workspace_config.snapshot_heading.as_deref())
        .unwrap_or("Visual Tests Summary");
    let subheading_markdown = args
        .subheading
        .as_deref()
        .or(workspace_config.snapshot_subheading.as_deref())
        .unwrap_or("Snapshot regression test results");
    context.insert("title", title);
    context.insert("heading", heading);

    let parser = pulldown_cmark::Parser::new(subheading_markdown);
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

    write_shared_file(regressify_dir, "_headers", include_bytes!("data/_headers"))?;

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
    fn snapshot_report_text_respects_cli_workspace_and_default_precedence() {
        use crate::cli::{Cli, Command};
        use crate::config::load_workspace_config;
        use clap::Parser;

        let defaults = [
            "Visual Tests Summary | Regressify",
            "Visual Tests Summary",
            "<p>Snapshot regression test results</p>",
        ];
        let workspace_yaml = "snapshotTitle: 'Shop <report>'\nsnapshotHeading: 'Shop & results'\nsnapshotSubheading: '**Staging** regression results'\n";
        let configured = [
            "Shop &lt;report&gt;",
            "Shop &amp; results",
            "<p><strong>Staging</strong> regression results</p>",
        ];
        let cases = [
            (None, vec![], defaults),
            (Some(workspace_yaml), vec![], configured),
            (
                Some("snapshotHeading: Only heading\n"),
                vec!["--title", "Only title"],
                ["Only title", "Only heading", defaults[2]],
            ),
            (
                Some(workspace_yaml),
                vec!["--title", "CLI <report>"],
                ["CLI &lt;report&gt;", configured[1], configured[2]],
            ),
            (
                Some(workspace_yaml),
                vec!["--heading", "CLI & summary"],
                [configured[0], "CLI &amp; summary", configured[2]],
            ),
            (
                Some(workspace_yaml),
                vec!["--subheading", "*CLI* notes"],
                [configured[0], configured[1], "<p><em>CLI</em> notes</p>"],
            ),
            (
                Some(workspace_yaml),
                vec![
                    "--title",
                    defaults[0],
                    "--heading",
                    defaults[1],
                    "--subheading",
                    "Snapshot regression test results",
                ],
                defaults,
            ),
            (
                Some(workspace_yaml),
                vec!["--title", "", "--heading", "", "--subheading", ""],
                ["", "", ""],
            ),
            (
                Some("snapshotTitle: ''\nsnapshotHeading: ''\nsnapshotSubheading: ''\n"),
                vec![],
                ["", "", ""],
            ),
            (
                Some("snapshotTitle: null\nsnapshotHeading: null\nsnapshotSubheading: null\n"),
                vec![],
                defaults,
            ),
        ];
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "report_template.html",
            include_str!("data/report_template.html"),
        )
        .unwrap();

        for (workspace_yaml, overrides, expected) in cases {
            let directory = tempfile::tempdir().unwrap();
            if let Some(workspace_yaml) = workspace_yaml {
                std::fs::write(directory.path().join("regressify.yaml"), workspace_yaml).unwrap();
            }
            let workspace_config = load_workspace_config(directory.path()).unwrap();
            let mut arguments = vec!["regressify", "snapshot"];
            arguments.extend(overrides);
            let Command::Snapshot(args) = Cli::try_parse_from(arguments).unwrap().command else {
                panic!("Expected snapshot command");
            };
            generate_html_report_summary(directory.path(), &tera, &[], &args, &workspace_config)
                .unwrap();

            let report = std::fs::read_to_string(directory.path().join("index.html")).unwrap();
            for fragment in [
                format!("<title>{}</title>", expected[0]),
                format!("<h1>{}</h1>", expected[1]),
                format!("<div class=\"subtitle\">{}</div>", expected[2]),
            ] {
                assert!(
                    report.contains(&fragment),
                    "Missing report text: {fragment}"
                );
            }
        }
    }

    #[test]
    fn netlify_headers_match_azure_configuration() {
        use std::collections::BTreeMap;

        let directory = tempfile::tempdir().unwrap();
        let mut tera = tera::Tera::default();
        tera.add_raw_template("report_template.html", "<html>{{ title }}</html>")
            .unwrap();
        let args = SnapshotArgs {
            test_suite: Vec::new(),
            title: Some("Report".into()),
            heading: Some("Summary".into()),
            subheading: Some(String::new()),
        };
        std::fs::write(directory.path().join("_headers"), "stale policy").unwrap();
        generate_html_report_summary(
            directory.path(),
            &tera,
            &[],
            &args,
            &WorkspaceConfig::default(),
        )
        .unwrap();

        let config: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.path().join("staticwebapp.config.json")).unwrap(),
        )
        .unwrap();
        let mut expected = BTreeMap::<String, BTreeMap<String, String>>::new();
        expected.insert(
            "/*".to_owned(),
            serde_json::from_value(config["globalHeaders"].clone()).unwrap(),
        );
        for route in config["routes"].as_array().unwrap() {
            let extensions = route["route"]
                .as_str()
                .unwrap()
                .strip_prefix("/*.{")
                .unwrap()
                .strip_suffix('}')
                .unwrap();
            for extension in extensions.split(',') {
                expected.insert(
                    format!("/*.{extension}"),
                    serde_json::from_value(route["headers"].clone()).unwrap(),
                );
            }
        }
        for (extension, mime_type) in config["mimeTypes"].as_object().unwrap() {
            expected
                .entry(format!("/*{extension}"))
                .or_default()
                .insert(
                    "Content-Type".to_owned(),
                    mime_type.as_str().unwrap().to_owned(),
                );
        }

        let headers = std::fs::read_to_string(directory.path().join("_headers"))
            .unwrap()
            .replace("\r\n", "\n");
        let actual = headers
            .trim()
            .split("\n\n")
            .map(|block| {
                let mut lines = block.lines();
                let path = lines.next().unwrap().to_owned();
                let headers = lines
                    .map(|line| {
                        let (name, value) = line.trim().split_once(": ").unwrap();
                        (name.to_owned(), value.to_owned())
                    })
                    .collect();
                (path, headers)
            })
            .collect::<BTreeMap<_, BTreeMap<_, _>>>();
        assert_eq!(actual, expected);
    }

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
