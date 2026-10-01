use std::{path::Path, time::Instant};

use super::{
    health::archives_disabled,
    install::emergency_warning,
    journal::{JournalOptions, JournalWriter, OperationMetadata},
    record::*,
};

pub struct StartupLog {
    writer: Option<JournalWriter>,
    started: Instant,
}

impl StartupLog {
    pub fn begin() -> Self {
        if archives_disabled(|name| std::env::var(name).ok()).unwrap_or(true) {
            return Self {
                writer: None,
                started: Instant::now(),
            };
        }
        match directories::BaseDirs::new() {
            Some(directories) => Self::at(
                &directories
                    .data_local_dir()
                    .join("regressify/diagnostics/startup"),
            ),
            None => {
                emergency_warning("Startup diagnostic directory is unavailable");
                Self {
                    writer: None,
                    started: Instant::now(),
                }
            }
        }
    }

    fn at(root: &Path) -> Self {
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::Preflight,
            DiagnosticEvent::Started,
        );
        let writer = JournalWriter::create(root, metadata, JournalOptions::default()).and_then(
            |mut writer| {
                writer.write(&record)?;
                writer.flush()?;
                Ok(writer)
            },
        );
        if writer.is_err() {
            emergency_warning(
                "Startup diagnostics could not be opened; command behavior is unchanged",
            );
        }
        Self {
            writer: writer.ok(),
            started: Instant::now(),
        }
    }

    pub fn finish(mut self, result: &anyhow::Result<()>) {
        if let Some(mut writer) = self.writer.take() {
            let outcome = if result.is_ok() {
                Outcome::Success
            } else {
                Outcome::Failure
            };
            let mut record = DiagnosticRecordV1::new(
                writer.metadata().operation_id.clone(),
                OperationKind::Capture,
                Stage::Finish,
                DiagnosticEvent::StageFinished,
            );
            record.elapsed_ms = self.started.elapsed().as_millis() as u64;
            record.duration_ms = Some(record.elapsed_ms);
            record.outcome = Some(outcome);
            record.error_cause = result.as_ref().err().map(SanitizedError::capture);
            if writer
                .write(&record)
                .and_then(|_| writer.finalize(outcome).map(|_| ()))
                .is_err()
            {
                emergency_warning(
                    "Startup diagnostics are incomplete; command behavior is unchanged",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::journal::{ArchiveState, JournalReader};

    #[test]
    fn startup_failures_are_durable_sanitized_and_never_pruned_on_restart() {
        let root = tempfile::tempdir().unwrap();
        for result in [
            Ok(()),
            Err(anyhow::anyhow!("Invalid config password=synthetic-secret")),
        ] {
            let log = StartupLog::at(root.path());
            let operation = log.writer.as_ref().unwrap().metadata().operation_id.clone();
            log.finish(&result);
            let reader = JournalReader::open(root.path(), &operation).unwrap();
            assert_eq!(reader.archive_state(), ArchiveState::Complete);
            reader
                .visit(|record| {
                    assert!(
                        !serde_json::to_string(&record)
                            .unwrap()
                            .contains("synthetic-secret")
                    )
                })
                .unwrap();
        }
        assert_eq!(
            std::fs::read_dir(root.path().join("_diagnostics"))
                .unwrap()
                .count(),
            2
        );
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, b"existing").unwrap();
        StartupLog::at(&blocked).finish(&Ok(()));
        assert_eq!(std::fs::read(blocked).unwrap(), b"existing");
    }
}
