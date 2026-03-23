use anyhow::{Result, bail};
use colored::*;
use std::collections::HashMap;
use std::fs;

use crate::{
    cli::args::SnapshotArgs,
    common,
    helpers::{IOExtensions, OnceLockExtensions},
    snapshot::{
        helpers::get_asset_version,
        report::{generate_html_report_summary, generate_shared_files, write_shared_file},
        transform::process_test_suite,
    },
};

pub fn snapshot(args: SnapshotArgs) -> Result<()> {
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    if !regressify_dir.exists() {
        bail!("Regressify directory does not exist: {:?}", regressify_dir);
    }

    let mut summaries = Vec::new();
    let mut hashes = HashMap::new();

    // Using Tera template engine to render the report
    let mut tera = tera::Tera::default();

    tera.add_raw_template(
        "report_template.html",
        include_str!("data/report_template.html"),
    )?;

    tera.add_raw_template(
        "test_suite_report.html",
        include_str!("data/test_suite_report.html"),
    )?;

    let index_bundle_content = include_str!("data/index_bundle.js");
    let bundle_hash = get_asset_version(index_bundle_content.as_bytes(), 20);

    let mut entries: Vec<_> = fs::read_dir(regressify_dir)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path().normalize()?;

        if path.is_dir()
            && let Some(dir_name) = path.file_name().and_then(|s| s.to_str())
        {
            if dir_name
                .chars()
                .any(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
            {
                eprintln!(
                        "{}",
                        format!("Skipping invalid test suite name: {}. Test suite names must be alphanumeric, underscores, or hyphens only.", dir_name).yellow()
                    );
                continue;
            }

            if let Ok(Some(summary)) =
                process_test_suite(regressify_dir, dir_name, &mut hashes, &tera, &bundle_hash)
            {
                summaries.push(summary);
            }
        }
    }

    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    generate_html_report_summary(regressify_dir, &tera, &summaries, &args)?;
    generate_shared_files(regressify_dir)?;

    write_shared_file(
        regressify_dir,
        "index_bundle.js",
        index_bundle_content.as_bytes(),
    )?;

    Ok(())
}
