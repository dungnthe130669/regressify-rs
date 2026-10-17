use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result, ensure};
use walkdir::WalkDir;

use super::{
    PendingLog, RunRecord, RunState, SuiteLease, capture_runs, read_registry,
    require_real_directory, write_registry_with_pending,
};
use crate::diagnostics::record::{OperationId, OperationKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiteCleanup {
    pub id: String,
    pub kept_runs: Vec<String>,
    pub removed_runs: Vec<String>,
    pub bytes: u64,
    pub staging_directories: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupPlan {
    pub output: PathBuf,
    pub keep_runs: usize,
    pub suites: Vec<SuiteCleanup>,
    pub bytes: u64,
    pub warnings: Vec<String>,
    directories: Vec<PathBuf>,
    inventory: Vec<ArtifactStamp>,
    retained: BTreeMap<String, Vec<RunRecord>>,
    pending_before: BTreeMap<String, Vec<PendingLog>>,
    pending_after: BTreeMap<String, Vec<PendingLog>>,
}

impl CleanupPlan {
    pub fn removed_runs(&self) -> usize {
        self.suites
            .iter()
            .map(|suite| suite.removed_runs.len())
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArtifactStamp {
    path: PathBuf,
    directory: bool,
    bytes: u64,
    modified: SystemTime,
}

pub fn output_suites(output: &Path) -> Result<Vec<String>> {
    if !output.try_exists()? {
        return Ok(Vec::new());
    }
    let mut suites = Vec::new();
    for entry in fs::read_dir(output)? {
        let entry = entry?;
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !entry.file_type()?.is_dir()
            || id.starts_with('.')
            || crate::test::is_reserved_suite_identifier(&id)
        {
            continue;
        }
        if entry.path().join("bitmaps_test").try_exists()? {
            crate::test::validate_suite_identifier("Cleanup suite ID", Some(&id))?;
            suites.push(id);
        }
    }
    suites.sort();
    Ok(suites)
}

fn acquire_suites(
    output: &Path,
    suites: &[String],
    keep_runs: usize,
) -> Result<(PathBuf, Vec<SuiteLease>)> {
    ensure!(keep_runs > 0, "Keep count must be at least 1");
    ensure!(
        !suites.is_empty(),
        "Select at least one test suite to clean"
    );
    let root = output
        .canonicalize()
        .context("Cannot resolve cleanup output folder")?;
    require_real_directory(&root)?;
    let mut leases = Vec::new();
    for id in suites.iter().collect::<BTreeSet<_>>() {
        crate::test::validate_suite_identifier("Cleanup suite ID", Some(id))?;
        let suite = root.join(id);
        require_real_directory(&suite)?;
        leases.push(SuiteLease::for_cleanup(&suite)?);
    }
    Ok((root, leases))
}

pub fn preview(output: &Path, suites: &[String], keep_runs: usize) -> Result<CleanupPlan> {
    let (root, _leases) = acquire_suites(output, suites, keep_runs)?;
    plan_locked(&root, suites, keep_runs)
}

pub fn command(output: &Path, args: crate::cli::args::CleanArgs) -> Result<()> {
    let filters = crate::compare::compile_suite_filters(&args.test_suite)?;
    let selected = output_suites(output)?
        .into_iter()
        .filter(|suite| crate::compare::suite_matches_filters(suite, &filters))
        .collect::<Vec<_>>();
    ensure!(
        !selected.is_empty(),
        "No output test suites matched the requested --test-suite filter"
    );
    let plan = preview(output, &selected, args.keep_runs)?;
    println!("Output: {}", plan.output.display());
    for suite in &plan.suites {
        println!(
            "{}: keep [{}], remove {} run(s) and {} staging folder(s), {}",
            suite.id,
            suite.kept_runs.join(", "),
            suite.removed_runs.len(),
            suite.staging_directories,
            crate::helpers::format_size(suite.bytes)
        );
    }
    println!(
        "Estimated space to reclaim: {}",
        crate::helpers::format_size(plan.bytes)
    );
    for warning in &plan.warnings {
        eprintln!("{warning}");
    }
    if args.yes {
        execute(&plan)?;
        println!(
            "Removed {} old test run(s). Reference baselines were preserved.",
            plan.removed_runs()
        );
    } else {
        println!("Preview only. Add --yes to delete these artifacts.");
    }
    Ok(())
}

fn append_directory(plan: &mut CleanupPlan, directory: &Path) -> Result<u64> {
    require_real_directory(directory)?;
    ensure!(
        directory.canonicalize()?.starts_with(&plan.output),
        "Cleanup path escapes output folder"
    );
    let mut bytes = 0u64;
    for entry in WalkDir::new(directory).follow_links(false) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            metadata.file_type().is_dir() || metadata.file_type().is_file(),
            "Cleanup refuses links or special files: {}",
            entry.path().display()
        );
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        bytes = bytes
            .checked_add(size)
            .context("Cleanup size exceeds supported range")?;
        plan.inventory.push(ArtifactStamp {
            path: entry.path().strip_prefix(&plan.output)?.to_path_buf(),
            directory: metadata.is_dir(),
            bytes: size,
            modified: metadata.modified()?,
        });
    }
    plan.directories
        .push(directory.strip_prefix(&plan.output)?.to_path_buf());
    Ok(bytes)
}

fn plan_locked(root: &Path, suites: &[String], keep_runs: usize) -> Result<CleanupPlan> {
    let mut plan = CleanupPlan {
        output: root.to_path_buf(),
        keep_runs,
        suites: Vec::new(),
        bytes: 0,
        warnings: Vec::new(),
        directories: Vec::new(),
        inventory: Vec::new(),
        retained: BTreeMap::new(),
        pending_before: BTreeMap::new(),
        pending_after: BTreeMap::new(),
    };
    for id in suites.iter().collect::<BTreeSet<_>>() {
        let suite = root.join(id);
        let tests = suite.join("bitmaps_test");
        let mut runs = if tests.try_exists()? {
            require_real_directory(&tests)?;
            capture_runs(&tests)?
        } else {
            Vec::new()
        };
        for run in &mut runs {
            if run.state == RunState::Running {
                run.state = RunState::Interrupted;
            }
        }
        let mut kept = BTreeSet::new();
        if let Some(default) = runs.iter().find(|run| run.is_default_candidate()) {
            kept.insert(default.id.clone());
        }
        for run in &runs {
            if kept.len() == keep_runs {
                break;
            }
            if run.state == RunState::Completed {
                kept.insert(run.id.clone());
            }
        }
        let mut summary = SuiteCleanup {
            id: id.clone(),
            kept_runs: kept.iter().cloned().collect(),
            removed_runs: Vec::new(),
            bytes: 0,
            staging_directories: 0,
        };
        for run in &runs {
            if kept.contains(&run.id) {
                continue;
            }
            summary.removed_runs.push(run.id.clone());
            summary.bytes += append_directory(&mut plan, &tests.join(&run.id))?;
            let raw = suite.join("tests");
            if raw.try_exists()? {
                require_real_directory(&raw)?;
                if raw.join(&run.id).try_exists()? {
                    summary.bytes += append_directory(&mut plan, &raw.join(&run.id))?;
                }
            }
        }
        for entry in fs::read_dir(&suite)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".compare-staging-"))
            {
                summary.bytes += append_directory(&mut plan, &entry.path())?;
                summary.staging_directories += 1;
            }
        }
        plan.retained.insert(
            id.clone(),
            runs.into_iter()
                .filter(|run| kept.contains(&run.id))
                .collect(),
        );
        plan.bytes += summary.bytes;
        plan.suites.push(summary);
    }
    plan_logs(&mut plan)?;
    plan.inventory
        .sort_by(|left, right| left.path.cmp(&right.path));
    plan.directories.sort();
    Ok(plan)
}

fn linked_operations(
    root: &Path,
    directory: &Path,
    removed: &[PathBuf],
) -> Result<BTreeSet<OperationId>> {
    let mut operations = BTreeSet::new();
    if !directory.try_exists()? {
        return Ok(operations);
    }
    require_real_directory(directory)?;
    for entry in WalkDir::new(directory)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !removed
                .iter()
                .any(|path| entry.path().starts_with(root.join(path)))
        })
    {
        let entry = entry?;
        ensure!(
            !entry.file_type().is_symlink(),
            "Cannot inspect log references through a link: {}",
            entry.path().display()
        );
        if entry.file_type().is_file()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".log.json"))
        {
            let log: crate::test::capture::CaptureLog =
                crate::diagnostics::journal::read_json(entry.path(), root, 64 * 1024)?;
            if let Some(archive) = log.archive {
                operations.insert(archive.operation_id);
            }
        }
    }
    Ok(operations)
}

fn retained_log_operations(plan: &CleanupPlan) -> Result<BTreeSet<OperationId>> {
    let mut operations = BTreeSet::new();
    for entry in fs::read_dir(&plan.output)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with('.') || crate::test::is_reserved_suite_identifier(name) {
            continue;
        }
        if !entry.file_type()?.is_dir() {
            ensure!(
                !entry.file_type()?.is_symlink(),
                "Cannot inspect retained log references through linked suites"
            );
            continue;
        }
        for captures in ["bitmaps_reference", "bitmaps_test"] {
            operations.extend(linked_operations(
                &plan.output,
                &entry.path().join(captures),
                &plan.directories,
            )?);
        }
    }
    Ok(operations)
}

fn plan_logs(plan: &mut CleanupPlan) -> Result<()> {
    let protected = match retained_log_operations(plan) {
        Ok(operations) => Some(operations),
        Err(error) => {
            plan.warnings.push(format!("Diagnostic logs will be retained because references could not be verified: {error:#}"));
            None
        }
    };
    let mut scheduled = BTreeSet::new();
    for suite in plan.suites.clone() {
        let mut candidates: BTreeSet<_> = read_registry(&plan.output.join(&suite.id))?
            .map(|registry| registry.pending_logs)
            .unwrap_or_default()
            .into_iter()
            .collect();
        for run_id in &suite.removed_runs {
            let directory = plan
                .output
                .join(&suite.id)
                .join("bitmaps_test")
                .join(run_id);
            match linked_operations(&plan.output, &directory, &[]) {
                Ok(operations) => {
                    candidates.extend(operations.into_iter().map(|operation_id| PendingLog {
                        run_id: run_id.clone(),
                        operation_id,
                    }))
                }
                Err(error) => plan.warnings.push(format!(
                    "Some diagnostic links in '{}/{}' could not be read: {error:#}",
                    suite.id, run_id
                )),
            }
        }
        plan.pending_before
            .insert(suite.id.clone(), candidates.iter().cloned().collect());
        let mut pending = Vec::new();
        for candidate in candidates {
            if protected
                .as_ref()
                .is_none_or(|operations| operations.contains(&candidate.operation_id))
            {
                pending.push(candidate);
                continue;
            }
            let directory = crate::diagnostics::journal::archive_directory(
                &plan.output,
                &candidate.operation_id,
            );
            let eligible = (|| -> Result<bool> {
                if !directory.try_exists()? {
                    return Ok(false);
                }
                require_real_directory(&plan.output.join("_diagnostics"))?;
                require_real_directory(&directory)?;
                let metadata: crate::diagnostics::journal::OperationMetadata =
                    crate::diagnostics::journal::read_json(
                        &directory.join("operation.json"),
                        &plan.output,
                        64 * 1024,
                    )?;
                ensure!(
                    metadata.version == 1
                        && metadata.operation_id == candidate.operation_id
                        && metadata.operation_kind == OperationKind::Capture
                        && metadata
                            .capture_run_id
                            .as_ref()
                            .is_some_and(|run| run.as_str() == candidate.run_id),
                    "Diagnostic operation does not belong to the removed capture run"
                );
                Ok(true)
            })();
            match eligible {
                Ok(true) => {
                    if scheduled.insert(candidate.operation_id.clone()) {
                        let bytes = append_directory(plan, &directory)?;
                        plan.bytes += bytes;
                        if let Some(summary) = plan
                            .suites
                            .iter_mut()
                            .find(|summary| summary.id == suite.id)
                        {
                            summary.bytes += bytes;
                        }
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    plan.warnings.push(format!(
                        "Diagnostic cleanup deferred for {}: {error:#}",
                        candidate.operation_id.as_str()
                    ));
                    pending.push(candidate);
                }
            }
        }
        plan.pending_after.insert(suite.id, pending);
    }
    Ok(())
}

pub fn execute(preview: &CleanupPlan) -> Result<()> {
    let suites: Vec<_> = preview
        .suites
        .iter()
        .map(|suite| suite.id.clone())
        .collect();
    let (root, _leases) = acquire_suites(&preview.output, &suites, preview.keep_runs)?;
    let current = plan_locked(&root, &suites, preview.keep_runs)?;
    ensure!(
        &current == preview,
        "Run artifacts changed after preview; review a fresh cleanup preview"
    );
    // Save retry ownership before removing the sidecars that identify these journals.
    for (id, pending) in &current.pending_before {
        let suite = root.join(id);
        let runs = if suite.join("bitmaps_test").try_exists()? {
            capture_runs(&suite.join("bitmaps_test"))?
        } else {
            Vec::new()
        };
        write_registry_with_pending(&suite, runs, pending.clone())?;
    }
    for relative in &current.directories {
        let directory = root.join(relative);
        require_real_directory(&directory)?;
        fs::remove_dir_all(&directory)
            .with_context(|| format!("Cannot clean run artifacts '{}'", directory.display()))?;
    }
    for (id, retained) in current.retained {
        let pending = current.pending_after.get(&id).cloned().unwrap_or_default();
        write_registry_with_pending(&root.join(id), retained, pending)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_history::{ArtifactLease, RunLease};

    fn run(root: &Path, suite: &str, id: &str, full: bool, state: RunState) {
        let suite = root.join(suite);
        let mut lease = RunLease::begin(&suite, id, full).unwrap();
        fs::write(
            suite.join("bitmaps_test").join(id).join("capture.png"),
            b"capture",
        )
        .unwrap();
        lease.finish(state, false).unwrap();
    }

    #[test]
    fn cleanup_keeps_strict_limit_and_latest_full_suite_with_newer_partial_runs() {
        let temp = tempfile::tempdir().unwrap();
        run(temp.path(), "shop", "old", true, RunState::Completed);
        run(temp.path(), "shop", "full", true, RunState::Completed);
        run(temp.path(), "shop", "partial", false, RunState::Completed);
        run(temp.path(), "shop", "stopped", true, RunState::Interrupted);
        run(temp.path(), "other", "old", true, RunState::Completed);
        let references = temp.path().join("shop/bitmaps_reference");
        fs::create_dir(&references).unwrap();
        fs::write(references.join("baseline.png"), b"baseline").unwrap();
        let plan = preview(temp.path(), &["shop".into()], 1).unwrap();
        assert_eq!(plan.suites[0].kept_runs, ["full"]);
        assert_eq!(plan.removed_runs(), 3);
        assert_eq!(plan.bytes, 21);
        assert!(temp.path().join("shop/bitmaps_test/old").is_dir());
        execute(&plan).unwrap();
        assert_eq!(
            capture_runs(&temp.path().join("shop/bitmaps_test"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            fs::read(references.join("baseline.png")).unwrap(),
            b"baseline"
        );
        assert!(temp.path().join("other/bitmaps_test/old").is_dir());
    }

    #[test]
    fn cleanup_supports_multiple_suites_and_completed_partial_slots() {
        let temp = tempfile::tempdir().unwrap();
        for suite in ["shop", "account"] {
            run(temp.path(), suite, "full", true, RunState::Completed);
            run(temp.path(), suite, "partial", false, RunState::Completed);
            run(temp.path(), suite, "failed", true, RunState::Failed);
        }
        let plan = preview(temp.path(), &["shop".into(), "account".into()], 2).unwrap();
        assert_eq!(plan.removed_runs(), 2);
        assert!(plan.suites.iter().all(|suite| suite.kept_runs.len() == 2));
        execute(&plan).unwrap();
    }

    #[test]
    fn clean_command_previews_by_default_and_deletes_only_explicit_matches() {
        let temp = tempfile::tempdir().unwrap();
        for suite in ["shop", "account"] {
            run(temp.path(), suite, "old", true, RunState::Completed);
            run(temp.path(), suite, "full", true, RunState::Completed);
        }
        for yes in [false, true] {
            command(
                temp.path(),
                crate::cli::args::CleanArgs {
                    test_suite: vec!["sho*".into()],
                    keep_runs: 1,
                    yes,
                },
            )
            .unwrap();
            assert_eq!(temp.path().join("shop/bitmaps_test/old").exists(), !yes);
            assert!(temp.path().join("account/bitmaps_test/old").is_dir());
        }
        assert!(
            command(
                temp.path(),
                crate::cli::args::CleanArgs {
                    test_suite: vec!["missing".into()],
                    keep_runs: 1,
                    yes: true,
                }
            )
            .is_err()
        );
    }

    #[test]
    fn cleanup_rejects_active_readers_invalid_limits_and_changed_previews() {
        let temp = tempfile::tempdir().unwrap();
        run(temp.path(), "shop", "old", true, RunState::Completed);
        run(temp.path(), "shop", "full", true, RunState::Completed);
        let selected = ["shop".into()];
        assert!(preview(temp.path(), &selected, 0).is_err());
        assert!(preview(temp.path(), &[], 1).is_err());
        assert!(preview(temp.path(), &["../outside".into()], 1).is_err());
        let reader = ArtifactLease::read(&temp.path().join("shop")).unwrap();
        assert!(preview(temp.path(), &selected, 1).is_err());
        drop(reader);
        let plan = preview(temp.path(), &selected, 1).unwrap();
        run(temp.path(), "shop", "new", true, RunState::Completed);
        assert!(
            execute(&plan)
                .unwrap_err()
                .to_string()
                .contains("changed after preview")
        );
        assert!(temp.path().join("shop/bitmaps_test/old").is_dir());
    }

    #[test]
    fn cleanup_imports_legacy_and_removes_abandoned_comparison_staging() {
        let temp = tempfile::tempdir().unwrap();
        for id in ["old", "new"] {
            fs::create_dir_all(temp.path().join("shop/bitmaps_test").join(id)).unwrap();
        }
        let stage = temp.path().join("shop/.compare-staging-old");
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("diff.png"), b"diff").unwrap();
        let plan = preview(temp.path(), &["shop".into()], 1).unwrap();
        assert_eq!(plan.suites[0].staging_directories, 1);
        execute(&plan).unwrap();
        assert!(!stage.exists());
        assert!(temp.path().join("shop/runs.json").is_file());
    }

    fn log(root: &Path, run_id: &str) -> PathBuf {
        let metadata = crate::diagnostics::journal::OperationMetadata::new(
            OperationKind::Capture,
            Some(crate::diagnostics::record::CaptureRunId::try_from(run_id.to_owned()).unwrap()),
        );
        let directory =
            crate::diagnostics::journal::archive_directory(root, &metadata.operation_id);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("operation.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        fs::write(directory.join("segment.jsonl"), b"capture log").unwrap();
        let sidecar = crate::test::capture::CaptureLog {
            state: crate::diagnostics::journal::ArchiveState::Complete,
            archive: Some(crate::diagnostics::runtime::ArchiveReference {
                operation_id: metadata.operation_id,
                execution_id: None,
                state: crate::diagnostics::journal::ArchiveState::Complete,
                missing_records: None,
            }),
        };
        fs::write(
            root.join("shop/bitmaps_test")
                .join(run_id)
                .join("capture.log.json"),
            serde_json::to_vec(&sidecar).unwrap(),
        )
        .unwrap();
        directory
    }

    #[test]
    fn cleanup_removes_owned_logs_but_retries_logs_still_used_by_references() {
        let temp = tempfile::tempdir().unwrap();
        run(temp.path(), "shop", "old", true, RunState::Completed);
        let old_log = log(temp.path(), "old");
        run(temp.path(), "shop", "promoted", true, RunState::Completed);
        let promoted_log = log(temp.path(), "promoted");
        run(temp.path(), "shop", "full", true, RunState::Completed);
        let full_log = log(temp.path(), "full");
        let reference = temp.path().join("shop/bitmaps_reference");
        fs::create_dir(&reference).unwrap();
        fs::copy(
            temp.path()
                .join("shop/bitmaps_test/promoted/capture.log.json"),
            reference.join("capture.log.json"),
        )
        .unwrap();
        execute(&preview(temp.path(), &["shop".into()], 1).unwrap()).unwrap();
        assert!(!old_log.exists());
        assert!(promoted_log.is_dir());
        assert!(full_log.is_dir());
        assert_eq!(
            read_registry(&temp.path().join("shop"))
                .unwrap()
                .unwrap()
                .pending_logs
                .len(),
            1
        );
        fs::remove_file(reference.join("capture.log.json")).unwrap();
        execute(&preview(temp.path(), &["shop".into()], 1).unwrap()).unwrap();
        assert!(!promoted_log.exists());
        assert!(full_log.is_dir());
    }

    #[test]
    fn cleanup_preserves_logs_when_retained_links_are_unreadable() {
        let temp = tempfile::tempdir().unwrap();
        run(temp.path(), "shop", "old", true, RunState::Completed);
        let old_log = log(temp.path(), "old");
        run(temp.path(), "shop", "full", true, RunState::Completed);
        let bad_link = temp.path().join("shop/bitmaps_test/full/capture.log.json");
        fs::write(&bad_link, "not json").unwrap();
        let plan = preview(temp.path(), &["shop".into()], 1).unwrap();
        assert!(!plan.warnings.is_empty());
        execute(&plan).unwrap();
        assert!(old_log.is_dir());
        fs::remove_file(bad_link).unwrap();
        execute(&preview(temp.path(), &["shop".into()], 1).unwrap()).unwrap();
        assert!(!old_log.exists());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_refuses_symlinks_inside_removed_runs() {
        let temp = tempfile::tempdir().unwrap();
        run(temp.path(), "shop", "old", true, RunState::Completed);
        run(temp.path(), "shop", "full", true, RunState::Completed);
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"keep").unwrap();
        std::os::unix::fs::symlink(
            outside.path(),
            temp.path().join("shop/bitmaps_test/old/link"),
        )
        .unwrap();
        assert!(preview(temp.path(), &["shop".into()], 1).is_err());
        assert!(outside.path().join("keep").is_file());
    }
}
