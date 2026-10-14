use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use crate::{
    cli::args::SnapshotArgs,
    common,
    compare::{compile_suite_filters, require_explicit_filter_match, suite_matches_filters},
    config::load_workspace_config,
    helpers::OnceLockExtensions,
    snapshot::site::generate_site,
};

pub fn snapshot(args: SnapshotArgs) -> Result<()> {
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;

    if !regressify_dir.exists() {
        bail!("Regressify directory does not exist: {:?}", regressify_dir);
    }

    let root = common::ROOT_DIR.get_required()?;
    let workspace_config = load_workspace_config(root).with_context(|| {
        format!(
            "Failed to load snapshot workspace configuration in '{}'",
            root.display()
        )
    })?;
    generate_site(
        regressify_dir,
        &selected_snapshot_suites(regressify_dir, &args.test_suite)?,
        &args,
        &workspace_config,
    )
}

/// List all UTF-8 output directories in stable order. Capture validation owns
/// identifier policy; snapshot only needs to discover potential suite outputs.
fn selected_snapshot_suites(regressify_dir: &Path, patterns: &[String]) -> Result<Vec<String>> {
    let filters = compile_suite_filters(patterns)?;
    let directories = snapshot_suite_directories(regressify_dir)?
        .into_iter()
        .filter(|suite| suite_matches_filters(suite, &filters))
        .collect::<Vec<_>>();
    require_explicit_filter_match(&filters, !directories.is_empty())?;
    Ok(directories)
}

fn snapshot_suite_directories(regressify_dir: &Path) -> Result<Vec<String>> {
    let mut entries: Vec<_> = fs::read_dir(regressify_dir)
        .and_then(|entries| entries.collect::<std::io::Result<_>>())
        .with_context(|| {
            format!(
                "Failed to list snapshot suite directories in '{}'",
                regressify_dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut directories = Vec::new();
    for entry in entries {
        let path = entry.path();
        if entry
            .file_type()
            .with_context(|| {
                format!(
                    "Failed to inspect snapshot suite directory '{}'",
                    path.display()
                )
            })?
            .is_dir()
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
            && super::site::valid_suite_id(name)
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
    fn snapshots_filter_exact_names_globs_and_multiple_suites() {
        let root = tempfile::tempdir().unwrap();
        for suite in ["shop", "shop-mobile", "account"] {
            std::fs::create_dir(root.path().join(suite)).unwrap();
        }
        assert_eq!(
            selected_snapshot_suites(root.path(), &["shop".into()]).unwrap(),
            vec!["shop"]
        );
        assert_eq!(
            selected_snapshot_suites(root.path(), &["shop*".into()]).unwrap(),
            vec!["shop", "shop-mobile"]
        );
        assert_eq!(
            selected_snapshot_suites(root.path(), &["account".into(), "shop".into()]).unwrap(),
            vec!["account", "shop"]
        );
        assert_eq!(selected_snapshot_suites(root.path(), &[]).unwrap().len(), 3);
        assert_eq!(
            selected_snapshot_suites(root.path(), &["all".into()])
                .unwrap()
                .len(),
            3
        );
        assert!(selected_snapshot_suites(root.path(), &["missing".into()]).is_err());
        assert!(selected_snapshot_suites(root.path(), &["[".into()]).is_err());
    }

    #[test]
    fn snapshot_io_errors_identify_suite_discovery_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("missing-output");
        let error = selected_snapshot_suites(&path, &[]).unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        let message = format!("{error:#}");
        assert!(
            message.contains("Failed to list snapshot suite directories"),
            "{message}"
        );
        assert!(message.contains(&path.display().to_string()), "{message}");
    }

    #[test]
    fn snapshot_directory_discovery_accepts_portable_utf8_suite_ids() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("store.frontend")).unwrap();
        std::fs::create_dir_all(temp.path().join("caf\u{00E9}")).unwrap();
        std::fs::write(temp.path().join("not-a-suite.txt"), []).unwrap();

        let directories = snapshot_suite_directories(temp.path()).unwrap();

        assert_eq!(directories, vec!["caf\u{00E9}", "store.frontend"]);
    }

    #[test]
    fn snapshot_directory_discovery_ignores_reserved_internal_directories() {
        let temp = tempfile::tempdir().unwrap();
        for name in [
            "_snapshot",
            "_diagnostics",
            "states",
            ".snapshot-staging",
            ".suite-update-staging",
        ] {
            std::fs::create_dir_all(temp.path().join(name)).unwrap();
        }
        assert!(snapshot_suite_directories(temp.path()).unwrap().is_empty());
        for pattern in ["_diagnostics", "states", "_*", ".*"] {
            assert!(
                selected_snapshot_suites(temp.path(), &[pattern.into()]).is_err(),
                "{pattern}"
            );
        }
    }
}
