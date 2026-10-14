use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};

use crate::{
    diagnostics::{
        adapter::format_record,
        journal::{ArchiveState, JournalReader, inspect_archive},
        record::{DiagnosticRecordV1, Severity, Stage},
        runtime::ArchiveReference,
    },
    progress::Job,
    test::capture::{CaptureLog, read_capture_log_cached},
};

use super::runner::{RunCommand, RunRequest};

pub const PAGE_RECORDS: usize = 500;

pub fn export_complete(
    output: &Path,
    sources: &[CaptureLog],
    query: &LogQuery,
    destination: &Path,
) -> anyhow::Result<()> {
    use std::io::Write;
    anyhow::ensure!(!sources.is_empty(), "No archive is available to export");
    let mut seen = BTreeSet::new();
    let mut verified_operations = BTreeSet::new();
    let mut readers = Vec::new();
    for source in sources {
        let reference = source
            .archive
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Incomplete or unavailable logs cannot be exported"))?;
        if verified_operations.insert(reference.operation_id.clone()) {
            anyhow::ensure!(
                inspect_archive(output, Some(&reference.operation_id), true)
                    == ArchiveState::Complete,
                "Incomplete or unavailable logs cannot be exported"
            );
        }
        if seen.insert((
            reference.operation_id.clone(),
            reference.execution_id.clone(),
        )) {
            readers.push((
                JournalReader::open(output, &reference.operation_id)?,
                reference.execution_id.clone(),
            ));
        }
    }
    if let Ok(metadata) = std::fs::symlink_metadata(destination) {
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Export destination must be a regular file"
        );
    }
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Export destination has no parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    writeln!(temporary, "[Loaded from archive]")?;
    let search = query.search.to_lowercase();
    for (reader, execution) in readers {
        let mut written = Ok(());
        reader.visit(|record| {
            if written.is_err()
                || execution.as_ref().is_some_and(|expected| {
                    record
                        .execution
                        .as_ref()
                        .is_some_and(|actual| &actual.execution_id != expected)
                })
            {
                return;
            }
            let Some(formatted) = query.matches_formatted(&record, &search) else {
                return;
            };
            written = writeln!(
                temporary,
                "[{}ms #{}] {}",
                record.elapsed_ms, record.sequence, formatted
            );
        })?;
        written?;
        reader.verified_index()?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(destination)?;
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub severity: Option<Severity>,
    pub stage: Option<Stage>,
    pub search: String,
    pub offset: usize,
}

impl LogQuery {
    fn matches_with_search(&self, record: &DiagnosticRecordV1, search_lower: &str) -> bool {
        self.severity.is_none_or(|level| record.severity >= level)
            && self.stage.is_none_or(|stage| record.stage == stage)
            && (search_lower.is_empty()
                || format_record(record).to_lowercase().contains(search_lower))
    }

    fn matches_formatted(&self, record: &DiagnosticRecordV1, search_lower: &str) -> Option<String> {
        if !self.severity.is_none_or(|level| record.severity >= level)
            || !self.stage.is_none_or(|stage| record.stage == stage)
        {
            return None;
        }
        let formatted = format_record(record);
        (search_lower.is_empty() || formatted.to_lowercase().contains(search_lower))
            .then_some(formatted)
    }
}

#[derive(Clone, Debug)]
pub struct LogPage {
    pub state: ArchiveState,
    pub records: Vec<DiagnosticRecordV1>,
    pub execution: Option<crate::diagnostics::record::ExecutionIdentity>,
    pub matched: usize,
    pub offset: usize,
    pub sources: Vec<CaptureLog>,
}

impl LogPage {
    pub fn page_count(&self) -> usize {
        self.matched.div_ceil(PAGE_RECORDS).max(1)
    }

    pub fn page_number(&self) -> usize {
        self.offset / PAGE_RECORDS + 1
    }

    pub fn offset_for_page(&self, page: usize) -> Option<usize> {
        (page <= self.page_count())
            .then_some(page)?
            .checked_sub(1)?
            .checked_mul(PAGE_RECORDS)
    }

    pub fn unavailable(state: ArchiveState) -> Self {
        Self {
            state,
            records: Vec::new(),
            execution: None,
            matched: 0,
            offset: 0,
            sources: Vec::new(),
        }
    }

    pub fn has_more(&self) -> bool {
        self.offset + self.records.len() < self.matched
    }

    pub fn text(&self) -> String {
        let heading = match self.state {
            ArchiveState::Complete => "Loaded from archive",
            ArchiveState::Partial => "Partial / interrupted archive; some records may be missing",
            ArchiveState::Missing => "Log unavailable for this run: expected archive is missing",
            ArchiveState::LegacyUnavailable => "Legacy capture: log not available",
            ArchiveState::LoadingError => "Archive could not be read",
        };
        let mut text = format!("[{heading}]\n");
        for record in &self.records {
            text.push_str(&format!(
                "[{}ms #{}] {}\n",
                record.elapsed_ms,
                record.sequence,
                format_record(record)
            ));
        }
        if self.offset > 0 || self.has_more() {
            text.push_str(&format!(
                "[Records {}-{} of {} matching records]\n",
                self.offset + 1,
                self.offset + self.records.len(),
                self.matched
            ));
        }
        text
    }
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.ends_with(['.', ' '])
        && !value.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
        && !value.chars().any(char::is_control)
        && !crate::helpers::is_windows_reserved_path_component(value)
        && Path::new(value).components().count() == 1
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

pub fn run_sources(request: &RunRequest, jobs: &[Job]) -> Vec<CaptureLog> {
    let output = request.options.output_directory(&request.root);
    let mut sources = Vec::new();
    let mut archive_states = BTreeMap::new();
    if let Some(operation) = &request.options.diagnostic_operation_id {
        let state = inspect_archive(&output, Some(operation), true);
        archive_states.insert(operation.clone(), state);
        sources.push(CaptureLog {
            state,
            archive: Some(ArchiveReference {
                operation_id: operation.clone(),
                execution_id: None,
                state,
                missing_records: None,
            }),
        });
    }
    if !request.command.captures() && !sources.is_empty() {
        return sources;
    }
    if request.command == RunCommand::Deploy || request.options.run_id.is_empty() {
        if sources.is_empty() {
            sources.push(CaptureLog {
                state: ArchiveState::LegacyUnavailable,
                archive: None,
            });
        }
        return sources;
    }
    if crate::cli::args::run_id_parser(&request.options.run_id).is_err() {
        sources.push(CaptureLog {
            state: ArchiveState::LoadingError,
            archive: None,
        });
        return sources;
    }
    for job in jobs {
        if !valid_component(&job.suite_id)
            || job.captures.iter().any(|capture| !valid_component(capture))
        {
            sources.push(CaptureLog {
                state: ArchiveState::LoadingError,
                archive: None,
            });
            continue;
        }
        for capture in &job.captures {
            let image = output
                .join(&job.suite_id)
                .join("bitmaps_test")
                .join(&request.options.run_id)
                .join(format!("{capture}.png"));
            sources.push(
                read_capture_log_cached(&output, &image, &mut archive_states).unwrap_or(
                    CaptureLog {
                        state: ArchiveState::LegacyUnavailable,
                        archive: None,
                    },
                ),
            );
        }
    }
    sources
}

pub fn load_run_page(request: &RunRequest, jobs: &[Job], query: &LogQuery) -> LogPage {
    load_page(
        &request.options.output_directory(&request.root),
        run_sources(request, jobs),
        query,
    )
}

pub fn load_page(output: &Path, sources: Vec<CaptureLog>, query: &LogQuery) -> LogPage {
    load_page_inner(output, sources, query, false)
}

pub fn load_run_tail(request: &RunRequest, jobs: &[Job], query: &LogQuery) -> LogPage {
    load_page_inner(
        &request.options.output_directory(&request.root),
        run_sources(request, jobs),
        query,
        true,
    )
}

fn load_page_inner(
    output: &Path,
    sources: Vec<CaptureLog>,
    query: &LogQuery,
    tail: bool,
) -> LogPage {
    let mut latest = std::collections::VecDeque::new();
    let mut page = LogPage {
        state: ArchiveState::Complete,
        records: Vec::new(),
        execution: None,
        matched: 0,
        offset: 0,
        sources: sources.clone(),
    };
    let mut seen = BTreeSet::new();
    let mut archive_states = BTreeMap::new();
    let search = query.search.to_lowercase();
    if sources.is_empty() {
        page.state = ArchiveState::LegacyUnavailable;
    }
    for source in sources {
        let Some(reference) = source.archive else {
            page.state = worse(page.state, source.state);
            continue;
        };
        if !seen.insert((
            reference.operation_id.clone(),
            reference.execution_id.clone(),
        )) {
            continue;
        }
        let state = *archive_states
            .entry(reference.operation_id.clone())
            .or_insert_with(|| inspect_archive(output, Some(&reference.operation_id), true));
        page.state = worse(page.state, state);
        if !matches!(state, ArchiveState::Complete | ArchiveState::Partial) {
            continue;
        }
        let result = JournalReader::open(output, &reference.operation_id).and_then(|reader| {
            reader.visit(|record| {
                if reference.execution_id.as_ref().is_some_and(|expected| {
                    record
                        .execution
                        .as_ref()
                        .is_some_and(|execution| &execution.execution_id != expected)
                }) {
                    return;
                }
                if page.execution.is_none()
                    && let Some(execution) = &record.execution
                {
                    page.execution = Some(execution.clone());
                }
                if query.matches_with_search(&record, &search) {
                    if tail {
                        if latest.len() == PAGE_RECORDS {
                            latest.pop_front();
                        }
                        latest.push_back(record);
                    } else if page.matched / PAGE_RECORDS <= query.offset / PAGE_RECORDS {
                        // Retain at most one aligned page, including the last page when totals shrink.
                        if page.matched.is_multiple_of(PAGE_RECORDS) {
                            page.records.clear();
                            page.offset = page.matched;
                        }
                        page.records.push(record);
                    }
                    page.matched += 1;
                }
            })
        });
        if result.is_err() {
            page.state = worse(page.state, ArchiveState::LoadingError);
        }
    }
    if tail {
        page.records = latest.into();
        page.offset = page.matched.saturating_sub(page.records.len());
    }
    page
}

fn worse(left: ArchiveState, right: ArchiveState) -> ArchiveState {
    let rank = |state| match state {
        ArchiveState::Complete => 0,
        ArchiveState::LegacyUnavailable => 1,
        ArchiveState::Partial => 2,
        ArchiveState::Missing => 3,
        ArchiveState::LoadingError => 4,
    };
    if rank(right) > rank(left) {
        right
    } else {
        left
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::diagnostics::{
        journal::{JournalOptions, JournalWriter, OperationMetadata},
        record::*,
    };

    #[test]
    fn log_pagination_boundaries_and_reconciled_offsets_remain_bounded() {
        for count in [0_usize, 1, 500, 501] {
            let root = tempfile::tempdir().unwrap();
            let (request, jobs, reference) = fixture(root.path(), count);
            for offset in [0, 1, 500, usize::MAX] {
                let page = load_run_page(
                    &request,
                    &jobs,
                    &LogQuery {
                        stage: Some(Stage::ActionsFrames),
                        offset,
                        ..Default::default()
                    },
                );
                let expected_offset = (offset / PAGE_RECORDS)
                    .min(count.saturating_sub(1) / PAGE_RECORDS)
                    * PAGE_RECORDS;
                assert_eq!(page.offset, expected_offset);
                assert_eq!(page.matched, count);
                assert_eq!(
                    page.records.len(),
                    (count - expected_offset).min(PAGE_RECORDS)
                );
                assert_eq!(page.page_count(), count.div_ceil(PAGE_RECORDS).max(1));
                assert_eq!(page.page_number(), expected_offset / PAGE_RECORDS + 1);
                assert_eq!(
                    page.offset_for_page(page.page_number()),
                    Some(expected_offset)
                );
                assert_eq!(page.offset_for_page(0), None);
                assert_eq!(page.offset_for_page(usize::MAX), None);
                assert_eq!(page.state, ArchiveState::Complete);
                if let Some(record) = page.records.first() {
                    assert_eq!(record.sequence, expected_offset as u64);
                }
            }
            let page = load_run_page(
                &request,
                &jobs,
                &LogQuery {
                    stage: Some(Stage::Finish),
                    offset: 500,
                    ..Default::default()
                },
            );
            assert_eq!(page.offset, 0);
            assert_eq!(page.matched, 1);
            assert_eq!(page.records.len(), 1);
            if count == 501 {
                let output = request.options.output_directory(root.path());
                std::fs::remove_file(
                    crate::diagnostics::journal::archive_directory(
                        &output,
                        &reference.operation_id,
                    )
                    .join("index.json"),
                )
                .unwrap();
                let partial = load_run_page(
                    &request,
                    &jobs,
                    &LogQuery {
                        stage: Some(Stage::ActionsFrames),
                        offset: usize::MAX,
                        ..Default::default()
                    },
                );
                assert_eq!(partial.state, ArchiveState::Partial);
                assert_eq!(partial.page_count(), 2);
                assert_eq!(partial.offset, 500);
                assert_eq!(partial.records.len(), 1);
                assert!(!partial.state.can_export());
            }
        }
    }

    pub(crate) fn fixture(root: &Path, count: usize) -> (RunRequest, Vec<Job>, ArchiveReference) {
        let output = root.join(".regressify");
        let metadata = OperationMetadata::new(
            OperationKind::Capture,
            Some(CaptureRunId::try_from("run".to_owned()).unwrap()),
        );
        let mut writer =
            JournalWriter::create(&output, metadata.clone(), JournalOptions::default()).unwrap();
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::ActionsFrames,
            DiagnosticEvent::Decision,
        );
        for _ in 0..count {
            writer.write(&record).unwrap();
        }
        writer.finalize(Outcome::Success).unwrap();
        let reference = ArchiveReference {
            operation_id: metadata.operation_id,
            execution_id: None,
            state: ArchiveState::Complete,
            missing_records: Some(0),
        };
        let images = output.join("suite/bitmaps_test/run");
        std::fs::create_dir_all(&images).unwrap();
        crate::test::capture::write_capture_log(
            &output,
            &images.join("page__FINAL.png"),
            &reference,
        )
        .unwrap();
        let request = RunRequest {
            root: root.to_owned(),
            command: RunCommand::Test,
            options: super::super::runner::RunOptions {
                run_id: "run".into(),
                ..Default::default()
            },
        };
        let jobs = vec![Job {
            id: 1,
            suite_id: "suite".into(),
            suite_name: "Suite".into(),
            scenario_id: "page".into(),
            label: "Page".into(),
            viewport: "desktop".into(),
            captures: vec!["page__FINAL".into()],
        }];
        (request, jobs, reference)
    }

    #[test]
    fn run_log_filter_search_download() {
        let root = tempfile::tempdir().unwrap();
        let (request, jobs, reference) = fixture(root.path(), 10_001);
        let output = request.options.output_directory(root.path());
        let query = LogQuery {
            severity: Some(Severity::Info),
            stage: Some(Stage::Finish),
            search: "success".into(),
            offset: 0,
        };
        let page = load_run_page(&request, &jobs, &query);
        assert_eq!(page.records.len(), 1);
        let destination = root.path().join("filtered.txt");
        export_complete(&output, &page.sources, &query, &destination).unwrap();
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), page.text());
        export_complete(&output, &page.sources, &LogQuery::default(), &destination).unwrap();
        assert_eq!(
            std::fs::read_to_string(&destination)
                .unwrap()
                .lines()
                .count(),
            10_003
        );
        let index =
            crate::diagnostics::journal::archive_directory(&output, &reference.operation_id)
                .join("index.json");
        std::fs::remove_file(index).unwrap();
        let before = std::fs::read(&destination).unwrap();
        assert!(export_complete(&output, &page.sources, &query, &destination).is_err());
        assert_eq!(std::fs::read(destination).unwrap(), before);
    }

    #[test]
    fn run_log_loads_archive_after_restart() {
        let root = tempfile::tempdir().unwrap();
        let (request, jobs, _) = fixture(root.path(), 5001);
        let mut query = LogQuery::default();
        let mut count = 0;
        loop {
            let page = load_run_page(&request, &jobs, &query);
            assert_eq!(page.state, ArchiveState::Complete);
            assert!(page.text().contains("Loaded from archive"));
            assert!(!page.text().contains("bounded history"));
            assert!(page.records.len() <= PAGE_RECORDS);
            count += page.records.len();
            if !page.has_more() {
                break;
            }
            query.offset = count;
        }
        assert_eq!(count, 5002);
    }

    #[test]
    fn run_log_non_capture_command_uses_saved_operation_identity() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join(".regressify");
        let mut operation = None;
        for _ in 0..2 {
            let metadata = OperationMetadata::new(OperationKind::Command, None);
            let mut writer =
                JournalWriter::create(&output, metadata.clone(), JournalOptions::default())
                    .unwrap();
            let mut record = DiagnosticRecordV1::new(
                metadata.operation_id.clone(),
                OperationKind::Command,
                Stage::Preflight,
                DiagnosticEvent::Started,
            );
            record
                .fields
                .insert(Field::Command, FieldValue::Kind(SafeValue::Optimize));
            writer.write(&record).unwrap();
            writer.finalize(Outcome::Success).unwrap();
            operation.get_or_insert(metadata.operation_id);
        }
        for command in [
            RunCommand::Optimize,
            RunCommand::Compare,
            RunCommand::Approve,
            RunCommand::Snapshot,
        ] {
            let mut request = RunRequest {
                root: root.path().to_owned(),
                command,
                options: super::super::runner::RunOptions {
                    diagnostic_operation_id: operation.clone(),
                    ..Default::default()
                },
            };
            let saved: RunRequest =
                serde_json::from_slice(&serde_json::to_vec(&request).unwrap()).unwrap();
            let page = load_run_page(&saved, &[], &LogQuery::default());
            assert_eq!(page.state, ArchiveState::Complete);
            assert_eq!(page.records.len(), 2);
            assert!(
                page.records
                    .iter()
                    .all(|record| Some(&record.operation_id) == operation.as_ref())
            );
            let destination = root.path().join("command-log.txt");
            export_complete(&output, &page.sources, &LogQuery::default(), &destination).unwrap();
            assert!(
                std::fs::read_to_string(destination)
                    .unwrap()
                    .contains("Command=Optimize")
            );
            request.options.diagnostic_operation_id = Some(OperationId::new());
            assert_eq!(
                load_run_page(&request, &[], &LogQuery::default()).state,
                ArchiveState::Missing
            );
            request.options.diagnostic_operation_id = None;
            assert_eq!(
                load_run_page(&request, &[], &LogQuery::default()).state,
                ArchiveState::LegacyUnavailable
            );
        }
    }

    #[test]
    fn run_log_shows_loading_error_state_for_corrupt_archive() {
        let root = tempfile::tempdir().unwrap();
        let (request, jobs, reference) = fixture(root.path(), 3);
        let directory = crate::diagnostics::journal::archive_directory(
            &request.options.output_directory(root.path()),
            &reference.operation_id,
        );
        std::fs::write(directory.join("segment-000000.jsonl"), b"corrupt").unwrap();
        let page = load_run_page(&request, &jobs, &LogQuery::default());
        assert_eq!(page.state, ArchiveState::LoadingError);
        assert!(page.text().contains("Archive could not be read"));
        assert!(!page.text().contains("Loaded from archive"));
    }

    #[test]
    fn shared_timelines_are_deduplicated_and_untrusted_paths_rejected() {
        let root = tempfile::tempdir().unwrap();
        let (request, mut jobs, _) = fixture(root.path(), 3);
        jobs.push(jobs[0].clone());
        assert_eq!(
            load_run_page(&request, &jobs, &LogQuery::default()).matched,
            4
        );
        jobs[0].suite_id = "../private".into();
        assert_eq!(
            load_run_page(&request, &jobs, &LogQuery::default()).state,
            ArchiveState::LoadingError
        );
    }
}
