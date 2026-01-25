use anyhow::{Result, bail};
use colored::*;
use std::collections::HashMap;
use std::fs;

use crate::{
    common,
    helpers::{IOExtensions, OnceLockExtensions},
    snapshot::{report::generate_html_report_summary, transform::process_test_suite},
};

pub fn snapshot() -> Result<()> {
    let backstop_dir = common::BACKSTOP_DIR.get_required()?;

    if !backstop_dir.exists() {
        bail!("Backstop directory does not exist: {:?}", backstop_dir);
    }

    let mut summaries = Vec::new();
    let mut hashes = HashMap::new();

    let child_dirs = fs::read_dir(backstop_dir)?;
    for entry in child_dirs.flatten() {
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

            if let Ok(Some(summary)) = process_test_suite(backstop_dir, dir_name, &mut hashes) {
                summaries.push(summary);
            }
        }
    }

    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    generate_html_report_summary(backstop_dir, &summaries)?;

    Ok(())
}
