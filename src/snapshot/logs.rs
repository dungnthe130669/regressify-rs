use std::{collections::BTreeMap, path::Path};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use super::site::{ExecutionLog, store_asset};
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
    let prefix = format!(
        "{{\"version\":1,\"operationId\":{},\"records\":[",
        serde_json::to_string(&reference.operation_id)?
    );
    let mut chunk_bytes = prefix.as_bytes().to_vec();
    let mut chunk_records = 0usize;
    let mut staged_chunks: Vec<(Vec<u8>, u64)> = Vec::new();
    let mut published = Ok(());
    let mut invalid_identity = false;
    let mut observed_execution = reference.execution_id.clone();
    let (state, missing_records, read) = reader.visit_with_state(|record| {
        if published.is_err() || invalid_identity {
            return;
        }
        if record.operation_kind != OperationKind::Capture {
            invalid_identity = true;
            return;
        }
        if let Some(execution) = &record.execution {
            if let Some(expected) = &observed_execution {
                if &execution.execution_id != expected {
                    invalid_identity = true;
                    return;
                }
            } else {
                observed_execution = Some(execution.execution_id.clone());
            }
        }
        let encoded = match serde_json::to_vec(&record) {
            Ok(bytes) => bytes,
            Err(error) => {
                published = Err(error.into());
                return;
            }
        };
        if chunk_records > 0
            && (chunk_records == MAX_CHUNK_RECORDS
                || chunk_bytes.len() + 1 + encoded.len() + 2 > MAX_CHUNK_BYTES)
        {
            published = stage_chunk(
                &reference.operation_id,
                &prefix,
                &mut chunk_bytes,
                &mut chunk_records,
                &mut staged_chunks,
            );
            if published.is_err() {
                return;
            }
        }
        if chunk_records > 0 {
            chunk_bytes.push(b',');
        }
        chunk_bytes.extend_from_slice(&encoded);
        chunk_records += 1;
        if chunk_bytes.len() + 2 > MAX_CHUNK_BYTES {
            published = Err(anyhow::anyhow!(
                "Capture log {} has an oversized public record",
                reference.operation_id
            ));
        }
    });
    published?;
    log.state = state;
    if matches!(log.state, ArchiveState::Complete | ArchiveState::Partial) {
        if read.is_err() || invalid_identity {
            log.state = ArchiveState::LoadingError;
            log.records = 0;
            log.missing_records = None;
            log.build = None;
        } else {
            log.build = Some(reader.metadata.build.clone());
            log.missing_records = if log.state == ArchiveState::Complete {
                Some(0)
            } else {
                missing_records
            };
            stage_chunk(
                &reference.operation_id,
                &prefix,
                &mut chunk_bytes,
                &mut chunk_records,
                &mut staged_chunks,
            )?;
            for (index, (bytes, count)) in staged_chunks.into_iter().enumerate() {
                let path = format!(
                    "_snapshot/data/logs/{}/chunk-{index:06}.json",
                    reference.operation_id
                );
                log.chunks.push(store_asset(output, &path, &bytes)?);
                log.records += count;
            }
        }
    } else {
        log.missing_records = None;
    }
    cache.insert(key, log.clone());
    Ok(Some(log))
}

fn stage_chunk(
    operation_id: &OperationId,
    prefix: &str,
    chunk_bytes: &mut Vec<u8>,
    chunk_records: &mut usize,
    staged_chunks: &mut Vec<(Vec<u8>, u64)>,
) -> Result<()> {
    if *chunk_records == 0 {
        return Ok(());
    }
    ensure!(
        staged_chunks.len() < 16_384,
        "Capture log {operation_id} exceeds the publication chunk limit; reduce the selected report scope"
    );
    chunk_bytes.extend_from_slice(b"]}");
    ensure!(
        chunk_bytes.len() <= MAX_CHUNK_BYTES,
        "Capture log {operation_id} has an oversized public record"
    );
    let completed = std::mem::replace(chunk_bytes, prefix.as_bytes().to_vec());
    staged_chunks.push((completed, *chunk_records as u64));
    *chunk_records = 0;
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
    let mut observed_execution = log.execution_id.clone();
    for record in chunk.records {
        record.validate()?;
        ensure!(
            record.operation_kind == OperationKind::Capture
                && record.operation_id == chunk.operation_id
                && record.sequence == *expected,
            "Snapshot log record identity or ordering mismatch"
        );
        if let Some(execution) = &record.execution {
            if let Some(expected_execution) = &observed_execution {
                ensure!(
                    &execution.execution_id == expected_execution,
                    "Snapshot log execution mismatch"
                );
            } else {
                observed_execution = Some(execution.execution_id.clone());
            }
        }
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
            execution_id: recorded_execution.clone(),
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

        let mut shared_cache = LogCache::new();
        let valid = CaptureLog {
            state: ArchiveState::Complete,
            archive: Some(ArchiveReference {
                operation_id: metadata.operation_id.clone(),
                execution_id: Some(recorded_execution),
                state: ArchiveState::Complete,
                missing_records: Some(0),
            }),
        };
        let valid_log = publish(root.path(), output.path(), Some(valid), &mut shared_cache)
            .unwrap()
            .unwrap();
        assert_eq!(valid_log.state, ArchiveState::Complete);
        assert_eq!(valid_log.chunks.len(), 2);
        let later_mismatch = CaptureLog {
            state: ArchiveState::Complete,
            archive: Some(ArchiveReference {
                operation_id: metadata.operation_id.clone(),
                execution_id: Some(ExecutionId::new()),
                state: ArchiveState::Complete,
                missing_records: Some(0),
            }),
        };
        let failed_log = publish(
            root.path(),
            output.path(),
            Some(later_mismatch),
            &mut shared_cache,
        )
        .unwrap()
        .unwrap();
        assert_eq!(failed_log.state, ArchiveState::LoadingError);
        assert!(failed_log.chunks.is_empty());
        assert!(
            output
                .path()
                .join(format!(
                    "_snapshot/data/logs/{}/chunk-000000.json",
                    metadata.operation_id
                ))
                .exists()
        );
    }
}
