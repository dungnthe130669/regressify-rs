use std::{collections::BTreeMap, path::Path};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use super::site::{ExecutionLog, store_asset, versioned_path};
use crate::{
    diagnostics::{
        journal::{ArchiveState, JournalReader, inspect_archive},
        record::{DiagnosticRecordV1, ExecutionId, OperationId, OperationKind},
    },
    test::capture::CaptureLog,
};

pub(super) const MAX_CHUNK_BYTES: usize = 256 * 1024;
pub(super) const MAX_CHUNK_RECORDS: usize = 250;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LogChunk {
    pub version: u16,
    pub operation_id: OperationId,
    pub records: Vec<DiagnosticRecordV1>,
}

pub(super) type LogCache = BTreeMap<(OperationId, Option<ExecutionId>), ExecutionLog>;

pub(super) fn publish(
    root: &Path,
    output: &Path,
    source: Option<CaptureLog>,
    cache: &mut LogCache,
) -> Result<Option<ExecutionLog>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let mut log = ExecutionLog {
        state: source.state,
        operation_id: None,
        execution_id: None,
        chunks: Vec::new(),
        records: 0,
        missing_records: None,
        build: None,
    };
    let Some(reference) = source.archive else {
        return Ok(Some(log));
    };
    let key = (
        reference.operation_id.clone(),
        reference.execution_id.clone(),
    );
    if let Some(cached) = cache.get(&key) {
        return Ok(Some(cached.clone()));
    }
    log.operation_id = Some(reference.operation_id.clone());
    log.execution_id = reference.execution_id.clone();
    log.missing_records = reference.missing_records;
    let reader = match JournalReader::open(root, &reference.operation_id) {
        Ok(reader) if reader.metadata.operation_kind == OperationKind::Capture => reader,
        Ok(_) => {
            log.state = ArchiveState::LoadingError;
            log.missing_records = None;
            cache.insert(key, log.clone());
            return Ok(Some(log));
        }
        Err(_) => {
            log.state = match inspect_archive(root, Some(&reference.operation_id), true) {
                ArchiveState::Missing => ArchiveState::Missing,
                _ => {
                    log.missing_records = None;
                    ArchiveState::LoadingError
                }
            };
            cache.insert(key, log.clone());
            return Ok(Some(log));
        }
    };
    let (state, missing_records) = reader.archive_state_and_missing();
    log.state = state;
    if matches!(log.state, ArchiveState::Complete | ArchiveState::Partial) {
        log.build = Some(reader.metadata.build.clone());
        log.missing_records = missing_records;
        let mut chunk = LogChunk {
            version: 1,
            operation_id: reference.operation_id.clone(),
            records: Vec::new(),
        };
        let mut bytes = 128;
        let mut published = Ok(());
        let mut invalid_identity = false;
        let read = reader.visit(|record| {
            if published.is_err() || invalid_identity {
                return;
            }
            if record.operation_kind != OperationKind::Capture
                || reference.execution_id.as_ref().is_some_and(|expected| {
                    record
                        .execution
                        .as_ref()
                        .is_some_and(|execution| &execution.execution_id != expected)
                })
            {
                invalid_identity = true;
                return;
            }
            let length = match serde_json::to_vec(&record) {
                Ok(bytes) => bytes.len(),
                Err(error) => {
                    published = Err(error.into());
                    return;
                }
            };
            if !chunk.records.is_empty()
                && (chunk.records.len() == MAX_CHUNK_RECORDS
                    || bytes + length + 1 > MAX_CHUNK_BYTES)
            {
                published = flush(output, &mut chunk, &mut log);
                bytes = 128;
            }
            if published.is_ok() {
                chunk.records.push(record);
                bytes += length + 1;
            }
        });
        published?;
        if read.is_err() || invalid_identity {
            for published_chunk in log.chunks.drain(..) {
                if let Ok(relative) = versioned_path(&published_chunk, "data") {
                    let _ = std::fs::remove_file(output.join(relative));
                }
            }
            log.state = ArchiveState::LoadingError;
            log.records = 0;
            log.missing_records = None;
            log.build = None;
        } else {
            flush(output, &mut chunk, &mut log)?;
            if log.state == ArchiveState::Complete {
                log.missing_records = Some(0);
            }
        }
    } else {
        log.missing_records = None;
    }
    cache.insert(key, log.clone());
    Ok(Some(log))
}

fn flush(output: &Path, chunk: &mut LogChunk, log: &mut ExecutionLog) -> Result<()> {
    if chunk.records.is_empty() {
        return Ok(());
    }
    ensure!(
        log.chunks.len() < 16_384,
        "Capture log {} exceeds the publication chunk limit; reduce the selected report scope",
        chunk.operation_id
    );
    let bytes = serde_json::to_vec(chunk)?;
    ensure!(
        bytes.len() <= MAX_CHUNK_BYTES,
        "Capture log {} has an oversized public record",
        chunk.operation_id
    );
    let path = format!(
        "_snapshot/data/logs/{}/chunk-{:06}.json",
        chunk.operation_id,
        log.chunks.len()
    );
    log.chunks.push(store_asset(output, &path, &bytes)?);
    log.records += chunk.records.len() as u64;
    chunk.records.clear();
    Ok(())
}

pub(super) fn validate_chunk(bytes: &[u8], log: &ExecutionLog, expected: &mut u64) -> Result<()> {
    ensure!(
        bytes.len() <= MAX_CHUNK_BYTES,
        "Snapshot log chunk exceeds 256 KiB"
    );
    let chunk: LogChunk = serde_json::from_slice(bytes)?;
    ensure!(
        chunk.version == 1
            && Some(&chunk.operation_id) == log.operation_id.as_ref()
            && !chunk.records.is_empty()
            && chunk.records.len() <= MAX_CHUNK_RECORDS,
        "Invalid snapshot log chunk metadata"
    );
    for record in chunk.records {
        record.validate()?;
        ensure!(
            record.operation_kind == OperationKind::Capture
                && record.operation_id == chunk.operation_id
                && record.sequence == *expected,
            "Snapshot log record identity or ordering mismatch"
        );
        ensure!(
            log.execution_id.as_ref().is_none_or(|expected| record
                .execution
                .as_ref()
                .is_none_or(|execution| &execution.execution_id == expected)),
            "Snapshot log execution mismatch"
        );
        *expected += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{
        journal::{JournalOptions, JournalWriter, OperationMetadata},
        record::{DiagnosticEvent, Outcome, Stage},
        runtime::ArchiveReference,
    };

    #[test]
    fn partial_publication_retains_readable_prefix_and_unknown_missing_count() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let mut writer =
            JournalWriter::create(root.path(), metadata.clone(), JournalOptions::default())
                .unwrap();
        for _ in 0..600 {
            writer
                .write(&DiagnosticRecordV1::new(
                    metadata.operation_id.clone(),
                    OperationKind::Capture,
                    Stage::QueueSetup,
                    DiagnosticEvent::Decision,
                ))
                .unwrap();
        }
        writer.flush().unwrap();
        drop(writer);
        let source = CaptureLog {
            state: ArchiveState::Partial,
            archive: Some(ArchiveReference {
                operation_id: metadata.operation_id,
                execution_id: None,
                state: ArchiveState::Partial,
                missing_records: Some(0),
            }),
        };
        let log = publish(
            root.path(),
            output.path(),
            Some(source),
            &mut LogCache::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(log.state, ArchiveState::Partial);
        assert_eq!(log.records, 600);
        assert_eq!(log.missing_records, None);
        assert_eq!(log.chunks.len(), 3);
    }

    #[test]
    fn public_capture_logs_are_bounded_shared_and_never_include_deployment_history() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        for kind in [
            OperationKind::Capture,
            OperationKind::Deployment,
            OperationKind::Command,
            OperationKind::Application,
        ] {
            let metadata = OperationMetadata::new(kind, None);
            let mut writer =
                JournalWriter::create(root.path(), metadata.clone(), JournalOptions::default())
                    .unwrap();
            let record = DiagnosticRecordV1::new(
                metadata.operation_id.clone(),
                kind,
                Stage::QueueSetup,
                DiagnosticEvent::Decision,
            );
            for _ in 0..501 {
                writer.write(&record).unwrap();
            }
            writer.finalize(Outcome::Success).unwrap();
            let source = CaptureLog {
                state: ArchiveState::Complete,
                archive: Some(ArchiveReference {
                    operation_id: metadata.operation_id,
                    execution_id: None,
                    state: ArchiveState::Complete,
                    missing_records: Some(0),
                }),
            };
            let mut cache = LogCache::new();
            let log = publish(root.path(), output.path(), Some(source.clone()), &mut cache)
                .unwrap()
                .unwrap();
            if kind == OperationKind::Capture {
                assert_eq!(log.records, 502);
                assert_eq!(log.chunks.len(), 3);
                assert_eq!(
                    log,
                    publish(root.path(), output.path(), Some(source), &mut cache)
                        .unwrap()
                        .unwrap()
                );
                let mut expected = 0;
                for chunk in &log.chunks {
                    let path = super::super::site::versioned_path(chunk, "data").unwrap();
                    let bytes = std::fs::read(output.path().join(path)).unwrap();
                    assert!(bytes.len() <= MAX_CHUNK_BYTES);
                    validate_chunk(&bytes, &log, &mut expected).unwrap();
                }
                assert_eq!(expected, 502);
            } else {
                assert_eq!(log.state, ArchiveState::LoadingError);
                assert!(log.chunks.is_empty());
            }
        }
    }

    #[test]
    fn mismatched_execution_or_corrupt_tail_resets_published_log_to_loading_error() {
        use crate::diagnostics::record::{
            CaptureRole, ExecutionId, ExecutionIdentity, SafeIdentity, ViewportIdentity,
        };

        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let mut writer =
            JournalWriter::create(root.path(), metadata.clone(), JournalOptions::default())
                .unwrap();
        let recorded_execution = ExecutionId::new();
        let mut record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Decision,
        );
        record.execution = Some(ExecutionIdentity {
            execution_id: recorded_execution,
            suite_id: SafeIdentity::source("suite"),
            scenario_id: SafeIdentity::source("home"),
            viewport: ViewportIdentity {
                name: SafeIdentity::source("desktop"),
                width: 1280,
                height: 720,
            },
            capture_role: CaptureRole::Test,
            attempt: 1,
        });
        for _ in 0..300 {
            writer.write(&record).unwrap();
        }
        writer.finalize(Outcome::Success).unwrap();

        let mismatched = CaptureLog {
            state: ArchiveState::Complete,
            archive: Some(ArchiveReference {
                operation_id: metadata.operation_id.clone(),
                execution_id: Some(ExecutionId::new()),
                state: ArchiveState::Complete,
                missing_records: Some(0),
            }),
        };
        let log = publish(
            root.path(),
            output.path(),
            Some(mismatched),
            &mut LogCache::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(log.state, ArchiveState::LoadingError);
        assert!(log.chunks.is_empty());
        assert_eq!(log.records, 0);
        assert_eq!(log.missing_records, None);
        assert!(log.build.is_none());
        assert!(
            !output
                .path()
                .join(format!(
                    "_snapshot/data/logs/{}/chunk-000000.json",
                    metadata.operation_id
                ))
                .exists()
        );
    }
}
