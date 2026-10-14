use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use walkdir::WalkDir;

use crate::helpers::write_file_atomically;

const MAX_REPORT_BYTES: u64 = 32 * 1024 * 1024;

type Inventory = BTreeMap<PathBuf, (bool, u64, SystemTime)>;

struct Replacement {
    source: PathBuf,
    destination: Option<PathBuf>,
    staged: Option<PathBuf>,
    backup: PathBuf,
    inventory: Inventory,
    backed_up: bool,
    installed: bool,
}

#[derive(Default)]
pub(super) struct ArtifactUpdate {
    directories: Vec<tempfile::TempDir>,
    replacements: Vec<Replacement>,
}

fn same_existing_path(first: &Path, second: &Path) -> bool {
    match (first.canonicalize(), second.canonicalize()) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    }
}

pub(super) fn existing_root(root: &Path) -> Result<Option<PathBuf>> {
    for ancestor in root.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Output roots must use real directories: {}",
                ancestor.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("Cannot inspect output directory"),
        }
    }
    Ok(Some(root.canonicalize()?))
}

fn inventory(path: &Path) -> Result<Inventory> {
    let mut inventory = BTreeMap::new();
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            !metadata.file_type().is_symlink() && (metadata.is_file() || metadata.is_dir()),
            "Suite artifacts must not contain links or special files: {}",
            entry.path().display()
        );
        inventory.insert(
            entry.path().strip_prefix(path)?.to_path_buf(),
            (metadata.is_dir(), metadata.len(), metadata.modified()?),
        );
    }
    Ok(inventory)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let target = destination.join(entry.path().strip_prefix(source)?);
        ensure!(
            !entry.file_type().is_symlink(),
            "Suite artifacts cannot contain symlinks"
        );
        if entry.file_type().is_dir() {
            fs::create_dir_all(target)?;
        } else {
            ensure!(
                entry.file_type().is_file(),
                "Suite artifacts must be regular files"
            );
            // Staged edits must replace files atomically, never mutate these shared inodes.
            if fs::hard_link(entry.path(), &target).is_err() {
                fs::copy(entry.path(), target)?;
            }
        }
    }
    Ok(())
}

fn update_report(
    path: &Path,
    old_id: &str,
    new_id: &str,
    display_name: Option<&str>,
    javascript: bool,
) -> Result<()> {
    let mut text = String::new();
    fs::File::open(path)?
        .take(MAX_REPORT_BYTES + 1)
        .read_to_string(&mut text)?;
    ensure!(
        text.len() as u64 <= MAX_REPORT_BYTES,
        "Suite report exceeds the 32 MiB limit"
    );
    let json = if javascript {
        text.trim()
            .strip_prefix("report(")
            .and_then(|text| text.strip_suffix(");"))
            .context("Invalid report config.js wrapper")?
    } else {
        text.as_str()
    };
    let mut report: Value = serde_json::from_str(json).context("Invalid suite report")?;
    ensure!(
        report["id"].as_str() == Some(old_id) && report["tests"].is_array(),
        "Report does not belong to the selected suite"
    );
    report["id"] = Value::String(new_id.into());
    if let Some(suite) = report.get_mut("testSuite") {
        ensure!(
            suite.as_str() == Some(old_id),
            "Report suite identities disagree"
        );
        *suite = Value::String(new_id.into());
    }
    let object = report
        .as_object_mut()
        .context("Invalid suite report object")?;
    if let Some(name) = display_name.filter(|name| !name.trim().is_empty()) {
        object.insert("displayName".into(), Value::String(name.into()));
    } else {
        object.remove("displayName");
    }
    let text = serde_json::to_string_pretty(&report)?;
    let text = if javascript {
        format!("report({text});")
    } else {
        text
    };
    write_file_atomically(path, text.as_bytes())
}

impl ArtifactUpdate {
    pub(super) fn prepare(
        roots: &[PathBuf],
        old_id: &str,
        new_id: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<Self> {
        let mut update = Self::default();
        for root in roots {
            let directory = tempfile::Builder::new()
                .prefix(".suite-update-")
                .tempdir_in(root)?;
            let stage = directory.path().join("stage");
            let backup = directory.path().join("backup");
            fs::create_dir(&stage)?;
            fs::create_dir(&backup)?;
            let source = root.join(old_id);
            if let Some(new_id) = new_id.filter(|id| *id != old_id) {
                let destination = root.join(new_id);
                ensure!(
                    same_existing_path(&source, &destination)
                        || fs::symlink_metadata(&destination)
                            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                    "Destination suite artifacts already exist"
                );
            }
            match fs::symlink_metadata(&source) {
                Ok(metadata) => {
                    ensure!(
                        metadata.is_dir() && !metadata.file_type().is_symlink(),
                        "Suite output must be a real directory"
                    );
                    let original = inventory(&source)?;
                    let staged = if let Some(new_id) = new_id {
                        let staged = stage.join(new_id);
                        copy_tree(&source, &staged)?;
                        for relative in original.keys() {
                            let javascript = relative == Path::new("html_report/config.js");
                            let report = relative.components().count() == 3
                                && relative.starts_with("bitmaps_test")
                                && relative
                                    .file_name()
                                    .is_some_and(|name| name == "report.json");
                            if javascript || report {
                                update_report(
                                    &staged.join(relative),
                                    old_id,
                                    new_id,
                                    display_name,
                                    javascript,
                                )?;
                            }
                        }
                        crate::compare::report_gen::write_suite_metadata(&staged, display_name)?;
                        Some(staged)
                    } else {
                        None
                    };
                    update.replacements.push(Replacement {
                        source,
                        destination: new_id.map(|id| root.join(id)),
                        staged,
                        backup: backup.join("suite"),
                        inventory: original,
                        backed_up: false,
                        installed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("Cannot inspect suite output"),
            }
            let snapshot = root.join("_snapshot");
            match fs::symlink_metadata(&snapshot) {
                Ok(metadata) => {
                    ensure!(
                        metadata.is_dir() && !metadata.file_type().is_symlink(),
                        "Snapshot output must be a real directory"
                    );
                    let original = inventory(&snapshot)?;
                    let index_inventory = inventory(&root.join("index.html"))?;
                    copy_tree(&snapshot, &stage.join("_snapshot"))?;
                    fs::copy(root.join("index.html"), stage.join("index.html"))?;
                    for (name, _) in crate::snapshot::FRONTEND_FILES {
                        if *name != "gitignore" && !name.starts_with("_snapshot/") {
                            inventory(&root.join(name))?;
                            fs::copy(root.join(name), stage.join(name))?;
                        }
                    }
                    crate::snapshot::update_suite(&stage, old_id, new_id, display_name)?;
                    for (name, original) in
                        [("_snapshot", original), ("index.html", index_inventory)]
                    {
                        update.replacements.push(Replacement {
                            source: root.join(name),
                            destination: Some(root.join(name)),
                            staged: Some(stage.join(name)),
                            backup: backup.join(name),
                            inventory: original,
                            backed_up: false,
                            installed: false,
                        });
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("Cannot inspect published reports"),
            }
            update.directories.push(directory);
        }
        Ok(update)
    }

    pub(super) fn commit<T>(mut self, write_document: impl FnOnce() -> Result<T>) -> Result<T> {
        // Keep originals until the document write succeeds; this is not a crash-atomic transaction.
        let result = (|| {
            for replacement in &self.replacements {
                ensure!(
                    inventory(&replacement.source)? == replacement.inventory,
                    "Suite artifacts changed during the operation; retry"
                );
                if let Some(destination) = &replacement.destination
                    && destination != &replacement.source
                {
                    ensure!(
                        same_existing_path(&replacement.source, destination)
                            || fs::symlink_metadata(destination)
                                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                        "Destination suite artifacts appeared during the operation"
                    );
                }
            }
            for replacement in &mut self.replacements {
                fs::rename(&replacement.source, &replacement.backup)
                    .context("Cannot stage existing suite artifacts")?;
                replacement.backed_up = true;
                if let (Some(staged), Some(destination)) =
                    (&replacement.staged, &replacement.destination)
                {
                    fs::rename(staged, destination)
                        .context("Cannot publish updated suite artifacts")?;
                    replacement.installed = true;
                }
            }
            write_document()
        })();
        if result.is_err() {
            let rollback = self.rollback();
            if let Err(error) = rollback {
                // A failed rollback must not let TempDir drop delete the retained originals.
                let retained = self
                    .directories
                    .drain(..)
                    .map(|directory| directory.keep().display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                return result.context(format!(
                    "Artifact rollback failed ({error:#}); originals retained at {retained}"
                ));
            }
        }
        result
    }

    fn rollback(&mut self) -> Result<()> {
        for replacement in self.replacements.iter_mut().rev() {
            if replacement.installed {
                let destination = replacement
                    .destination
                    .as_ref()
                    .context("Missing replacement path")?;
                let staged = replacement.staged.as_ref().context("Missing staged path")?;
                fs::rename(destination, staged)?;
                replacement.installed = false;
            }
            if replacement.backed_up {
                fs::rename(&replacement.backup, &replacement.source)?;
                replacement.backed_up = false;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn published_log_suites(
        root: &Path,
    ) -> (PathBuf, Vec<crate::diagnostics::record::OperationId>) {
        let mut operations = Vec::new();
        let output = root.join(".regressify");
        let image = lodepng::encode32(&[10_u8, 20, 30, 255], 1, 1).unwrap();
        for id in ["one", "two"] {
            let (_, _, reference) = crate::desktop::logs::tests::fixture(root, 3);
            let suite = output.join(id);
            fs::rename(output.join("suite"), &suite).unwrap();
            fs::create_dir_all(suite.join("bitmaps_reference")).unwrap();
            fs::write(suite.join("bitmaps_reference/page__FINAL.png"), &image).unwrap();
            fs::write(suite.join("bitmaps_test/run/page__FINAL.png"), &image).unwrap();
            crate::compare::report_gen::generate_comparison_report(&suite, id, "run", 0.0, 1)
                .unwrap();
            operations.push(reference.operation_id);
        }
        crate::snapshot::generate_site(
            &output,
            &["one".into(), "two".into()],
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        (output.canonicalize().unwrap(), operations)
    }

    #[test]
    fn suite_rename_preserves_log_assets() {
        let root = tempfile::tempdir().unwrap();
        let (output, operations) = published_log_suites(root.path());
        let before = crate::snapshot::artifact_paths(&output, &["one".into(), "two".into()])
            .unwrap()
            .into_iter()
            .filter(|path| path.starts_with("_snapshot/data/logs"))
            .collect::<Vec<_>>();
        ArtifactUpdate::prepare(
            std::slice::from_ref(&output),
            "one",
            Some("renamed"),
            Some("Renamed suite"),
        )
        .unwrap()
        .commit(|| Ok(()))
        .unwrap();
        let after = crate::snapshot::artifact_paths(&output, &["renamed".into(), "two".into()])
            .unwrap()
            .into_iter()
            .filter(|path| path.starts_with("_snapshot/data/logs"))
            .collect::<Vec<_>>();
        assert_eq!(before, after);
        assert_eq!(after.len(), 2);
        let log = crate::test::capture::read_capture_log(
            &output,
            &output.join("renamed/bitmaps_test/run/page__FINAL.png"),
        )
        .unwrap();
        assert_eq!(log.archive.unwrap().operation_id, operations[0]);
        assert!(!output.join("one").exists());
    }

    #[test]
    fn suite_delete_removes_only_unreferenced_logs_and_preserves_active_history() {
        use crate::diagnostics::{journal::*, record::*};
        let root = tempfile::tempdir().unwrap();
        let (output, operations) = published_log_suites(root.path());
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let mut active =
            JournalWriter::create(&output, metadata.clone(), JournalOptions::default()).unwrap();
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Started,
        );
        active.write(&record).unwrap();
        ArtifactUpdate::prepare(std::slice::from_ref(&output), "one", None, None)
            .unwrap()
            .commit(|| Ok(()))
            .unwrap();
        let paths = crate::snapshot::artifact_paths(&output, &["two".into()]).unwrap();
        let logs = paths
            .iter()
            .filter(|path| path.starts_with("_snapshot/data/logs"))
            .collect::<Vec<_>>();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].to_string_lossy().contains(operations[1].as_str()));
        assert!(
            !output
                .join(format!("_snapshot/data/logs/{}", operations[0]))
                .exists()
        );
        for operation in operations {
            assert!(archive_directory(&output, &operation).is_dir());
        }
        active.write(&record).unwrap();
        active.finalize(Outcome::Success).unwrap();
        assert_eq!(
            JournalReader::open(&output, &metadata.operation_id)
                .unwrap()
                .archive_state(),
            ArchiveState::Complete
        );
    }

    #[test]
    fn suite_artifact_rollback_restores_all_roots_after_document_failure() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let roots = [first.path().to_path_buf(), second.path().to_path_buf()];
        for root in &roots {
            fs::create_dir(root.join("shop")).unwrap();
            fs::write(root.join("shop/reference.png"), b"original bytes").unwrap();
        }
        for target in [Some("new"), None, Some("shop")] {
            let update = ArtifactUpdate::prepare(&roots, "shop", target, Some("Label")).unwrap();
            assert!(
                update
                    .commit(|| -> Result<()> { anyhow::bail!("document conflict") })
                    .is_err()
            );
            for root in &roots {
                assert_eq!(
                    fs::read(root.join("shop/reference.png")).unwrap(),
                    b"original bytes"
                );
                assert!(!root.join("new").exists());
                assert!(!root.join("shop/suite.json").exists());
            }
        }
    }

    #[test]
    fn suite_artifact_conflict_is_detected_before_publication() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("shop")).unwrap();
        let update =
            ArtifactUpdate::prepare(&[root.path().into()], "shop", Some("new"), None).unwrap();
        fs::write(root.path().join("shop/concurrent"), b"external").unwrap();
        assert!(update.commit(|| Ok(())).is_err());
        assert!(root.path().join("shop/concurrent").exists());
        assert!(!root.path().join("new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn suite_artifacts_reject_symlinks_without_touching_targets() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("shop")).unwrap();
        fs::write(outside.path().join("sentinel"), b"private").unwrap();
        symlink(outside.path(), root.path().join("shop/linked")).unwrap();
        assert!(ArtifactUpdate::prepare(&[root.path().into()], "shop", None, None).is_err());
        assert_eq!(
            fs::read(outside.path().join("sentinel")).unwrap(),
            b"private"
        );
    }
}
