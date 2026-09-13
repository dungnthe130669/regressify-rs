use anyhow::{Result, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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

    for dir_name in snapshot_suite_directories(regressify_dir)? {
        // Hash paths are relative to each suite's html_report directory;
        // sharing them across suites can apply the wrong cache version.
        let mut hashes = BTreeMap::new();
        if let Some(summary) =
            process_test_suite(regressify_dir, &dir_name, &mut hashes, &tera, &bundle_hash)
                .map_err(|error| {
                    anyhow::anyhow!("Failed to process snapshot suite '{}': {}", dir_name, error)
                })?
        {
            summaries.push(summary);
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

/// List all UTF-8 output directories in stable order. Capture validation owns
/// identifier policy; snapshot only needs to discover potential suite outputs.
fn snapshot_suite_directories(regressify_dir: &Path) -> Result<Vec<String>> {
    let mut entries: Vec<_> = fs::read_dir(regressify_dir)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut directories = Vec::new();
    for entry in entries {
        let path = entry.path().normalize()?;
        if path.is_dir()
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            directories.push(name.to_string());
        }
    }
    Ok(directories)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_directory_discovery_accepts_portable_utf8_suite_ids() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("store.frontend")).unwrap();
        std::fs::create_dir_all(temp.path().join("caf\u{00E9}")).unwrap();
        std::fs::write(temp.path().join("not-a-suite.txt"), []).unwrap();

        let directories = snapshot_suite_directories(temp.path()).unwrap();

        assert_eq!(directories, vec!["caf\u{00E9}", "store.frontend"]);
    }
}
