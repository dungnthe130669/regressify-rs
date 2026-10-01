use std::{
    collections::BTreeSet,
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
    test::capture::{CaptureLog, read_capture_log},
};

use super::runner::{RunCommand, RunRequest};

pub const PAGE_RECORDS: usize = 500;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub severity: Option<Severity>,
    pub stage: Option<Stage>,
    pub search: String,
    pub offset: usize,
}

impl LogQuery {
    fn matches(&self, record: &DiagnosticRecordV1) -> bool {
        self.severity.is_none_or(|level| record.severity >= level)
            && self.stage.is_none_or(|stage| record.stage == stage)
            && (self.search.is_empty()
                || format_record(record)
                    .to_lowercase()
                    .contains(&self.search.to_lowercase()))
    }
}

#[derive(Clone, Debug)]
pub struct LogPage {
    pub state: ArchiveState,
    pub records: Vec<DiagnosticRecordV1>,
    pub matched: usize,
    pub offset: usize,
    pub sources: Vec<CaptureLog>,
}

impl LogPage {
    pub fn unavailable(state: ArchiveState) -> Self {
        Self {
            state,
            records: Vec::new(),
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
    if request.command == RunCommand::Deploy {
        return vec![match &request.options.diagnostic_operation_id {
            Some(operation) => {
                let state = inspect_archive(&output, Some(operation), true);
                CaptureLog {
                    state,
                    archive: Some(ArchiveReference {
                        operation_id: operation.clone(),
                        execution_id: None,
                        state,
                        missing_records: None,
                    }),
                }
            }
            None => CaptureLog {
                state: ArchiveState::LegacyUnavailable,
                archive: None,
            },
        }];
    }
    if request.options.run_id.is_empty() {
        return vec![CaptureLog {
            state: ArchiveState::LegacyUnavailable,
            archive: None,
        }];
    }
    if crate::cli::args::run_id_parser(&request.options.run_id).is_err() {
        return vec![CaptureLog {
            state: ArchiveState::LoadingError,
            archive: None,
        }];
    }
    let mut sources = Vec::new();
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
            sources.push(read_capture_log(&output, &image).unwrap_or(CaptureLog {
                state: ArchiveState::LegacyUnavailable,
                archive: None,
            }));
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
    let mut page = LogPage {
        state: ArchiveState::Complete,
        records: Vec::new(),
        matched: 0,
        offset: query.offset,
        sources: sources.clone(),
    };
    let mut seen = BTreeSet::new();
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
        let state = inspect_archive(output, Some(&reference.operation_id), true);
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
                if query.matches(&record) {
                    if page.matched >= query.offset && page.records.len() < PAGE_RECORDS {
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
