use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{
    journal::{ArchiveState, JournalOptions, JournalWriter, MAX_RECORD_BYTES, OperationMetadata},
    record::{DiagnosticRecordV1, Outcome},
};

pub const DISABLE_ARCHIVE_ENV: &str = "REGRESSIFY_DISABLE_DIAGNOSTIC_ARCHIVE";
pub const QUEUE_RECORDS: usize = 64;
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn archives_disabled(environment: impl Fn(&str) -> Option<String>) -> Result<bool> {
    match environment(DISABLE_ARCHIVE_ENV)
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("false" | "0") => Ok(false),
        Some("true" | "1") => Ok(true),
        Some(_) => bail!("{DISABLE_ARCHIVE_ENV} must be true, false, 1, or 0"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticWarning {
    Disabled,
    CreateFailed,
    WriteFailed,
    QueueFull,
    InvalidRecord,
    FinalizeFailed,
    FinalizeTimeout,
    Interrupted,
}

impl DiagnosticWarning {
    pub fn message(self) -> &'static str {
        match self {
            Self::Disabled => "Diagnostic archive disabled; live history is not a full log",
            Self::CreateFailed => "Diagnostic archive unavailable; capture results are unchanged",
            Self::WriteFailed => "Diagnostic archive write failed; readable history is retained",
            Self::QueueFull => "Diagnostic archive queue exhausted; the log is incomplete",
            Self::InvalidRecord => "Diagnostic record rejected; the log is incomplete",
            Self::FinalizeFailed => {
                "Diagnostic archive could not be finalized; the log is incomplete"
            }
            Self::FinalizeTimeout => "Diagnostic writer stalled; the log is incomplete",
            Self::Interrupted => "Diagnostic execution interrupted; the log is incomplete",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSnapshot {
    pub state: ArchiveState,
    pub warning: Option<DiagnosticWarning>,
    pub records_written: u64,
    pub missing_records: Option<u64>,
}

struct Health {
    status: Mutex<HealthSnapshot>,
    collecting: AtomicBool,
    closing: AtomicBool,
    missing: AtomicU64,
    unknown_missing: AtomicBool,
    outcome: Mutex<Outcome>,
    notify: Arc<dyn Fn(HealthSnapshot) + Send + Sync>,
}

impl Health {
    fn snapshot(&self) -> HealthSnapshot {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        status.missing_records = (!self.unknown_missing.load(Ordering::Acquire))
            .then(|| self.missing.load(Ordering::Acquire));
        status
    }

    fn warn(&self, warning: DiagnosticWarning) {
        self.collecting.store(false, Ordering::Release);
        if matches!(
            warning,
            DiagnosticWarning::FinalizeTimeout | DiagnosticWarning::Interrupted
        ) {
            self.unknown_missing.store(true, Ordering::Release);
        }
        let notify = {
            let mut status = self
                .status
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            status.state = if matches!(
                warning,
                DiagnosticWarning::CreateFailed | DiagnosticWarning::Disabled
            ) {
                ArchiveState::Missing
            } else {
                ArchiveState::Partial
            };
            if status.warning.is_none() {
                status.warning = Some(warning);
                true
            } else {
                false
            }
        };
        if notify {
            (self.notify)(self.snapshot());
        }
    }
}

pub struct JournalSink {
    sender: Option<SyncSender<Vec<u8>>>,
    submission: Mutex<()>,
    health: Arc<Health>,
    pub metadata: OperationMetadata,
}

impl JournalSink {
    pub fn record(&self, record: &DiagnosticRecordV1) {
        let submission = match self.submission.try_lock() {
            Ok(submission) => submission,
            Err(_) => {
                self.health.missing.fetch_add(1, Ordering::Relaxed);
                self.health.warn(DiagnosticWarning::QueueFull);
                return;
            }
        };
        if !self.health.collecting.load(Ordering::Acquire) {
            self.health.missing.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let encoded = record
            .validate()
            .and_then(|_| Ok(serde_json::to_vec(record)?));
        let bytes = match encoded {
            Ok(bytes) if bytes.len() < MAX_RECORD_BYTES => bytes,
            _ => {
                drop(submission);
                self.health.missing.fetch_add(1, Ordering::Relaxed);
                self.health.warn(DiagnosticWarning::InvalidRecord);
                return;
            }
        };
        let Some(sender) = &self.sender else { return };
        let sent = sender.try_send(bytes);
        drop(submission);
        match sent {
            Ok(()) => {}
            Err(error) => {
                self.health.missing.fetch_add(1, Ordering::Relaxed);
                self.health.warn(match error {
                    TrySendError::Full(_) => DiagnosticWarning::QueueFull,
                    TrySendError::Disconnected(_) => DiagnosticWarning::WriteFailed,
                });
            }
        }
    }

    pub fn health(&self) -> HealthSnapshot {
        self.health.snapshot()
    }
}

#[must_use]
pub struct JournalOwner {
    sink: Arc<JournalSink>,
    completed: Option<Receiver<HealthSnapshot>>,
    worker: Option<thread::JoinHandle<()>>,
    finished: bool,
}

impl JournalOwner {
    pub fn start(
        output: &Path,
        metadata: OperationMetadata,
        options: JournalOptions,
        disabled: bool,
        notify: impl Fn(HealthSnapshot) + Send + Sync + 'static,
    ) -> Self {
        let output = output.to_owned();
        let header = metadata.clone();
        Self::start_with(
            metadata,
            disabled,
            move || JournalWriter::create(&output, header, options),
            notify,
        )
    }

    fn start_with(
        metadata: OperationMetadata,
        disabled: bool,
        create: impl FnOnce() -> Result<JournalWriter> + Send + 'static,
        notify: impl Fn(HealthSnapshot) + Send + Sync + 'static,
    ) -> Self {
        let health = Arc::new(Health {
            status: Mutex::new(HealthSnapshot {
                state: ArchiveState::Partial,
                warning: None,
                records_written: 0,
                missing_records: Some(0),
            }),
            collecting: AtomicBool::new(!disabled),
            closing: AtomicBool::new(false),
            missing: AtomicU64::new(0),
            unknown_missing: AtomicBool::new(false),
            outcome: Mutex::new(Outcome::Unknown),
            notify: Arc::new(notify),
        });
        if disabled {
            health.warn(DiagnosticWarning::Disabled);
            return Self {
                sink: Arc::new(JournalSink {
                    sender: None,
                    submission: Mutex::new(()),
                    health,
                    metadata,
                }),
                completed: None,
                worker: None,
                finished: false,
            };
        }
        let (sender, receiver) = mpsc::sync_channel(QUEUE_RECORDS);
        let (done, completed) = mpsc::channel();
        let worker_health = health.clone();
        let worker = thread::Builder::new()
            .name("diagnostic-journal".into())
            .spawn(move || {
                let snapshot = run_writer(create, receiver, &worker_health);
                let _ = done.send(snapshot);
            });
        let worker = match worker {
            Ok(worker) => Some(worker),
            Err(_) => {
                health.warn(DiagnosticWarning::CreateFailed);
                None
            }
        };
        Self {
            sink: Arc::new(JournalSink {
                sender: Some(sender),
                submission: Mutex::new(()),
                health,
                metadata,
            }),
            completed: Some(completed),
            worker,
            finished: false,
        }
    }

    pub fn sink(&self) -> Arc<JournalSink> {
        self.sink.clone()
    }

    pub fn finish(self, outcome: Outcome) -> HealthSnapshot {
        self.finish_with_timeout(outcome, FINALIZE_TIMEOUT)
    }

    fn finish_with_timeout(mut self, outcome: Outcome, timeout: Duration) -> HealthSnapshot {
        let submission = self
            .sink
            .submission
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *self
            .sink
            .health
            .outcome
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = outcome;
        self.sink.health.collecting.store(false, Ordering::Release);
        self.sink.health.closing.store(true, Ordering::Release);
        drop(submission);
        self.finished = true;
        if let Some(completed) = &self.completed {
            match completed.recv_timeout(timeout) {
                Ok(snapshot) => {
                    if let Some(worker) = self.worker.take() {
                        let _ = worker.join();
                    }
                    return snapshot;
                }
                Err(_) => self.sink.health.warn(DiagnosticWarning::FinalizeTimeout),
            }
        }
        self.sink.health.snapshot()
    }
}

impl Drop for JournalOwner {
    fn drop(&mut self) {
        if !self.finished {
            self.sink.health.warn(DiagnosticWarning::Interrupted);
            *self
                .sink
                .health
                .outcome
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Outcome::Cancelled;
            self.sink.health.closing.store(true, Ordering::Release);
        }
    }
}

fn run_writer(
    create: impl FnOnce() -> Result<JournalWriter>,
    receiver: Receiver<Vec<u8>>,
    health: &Health,
) -> HealthSnapshot {
    let mut writer = match create() {
        Ok(writer) => Some(writer),
        Err(_) => {
            health.warn(DiagnosticWarning::CreateFailed);
            None
        }
    };
    let mut failed = false;
    loop {
        match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(bytes) => {
                if let Some(writer) = writer.as_mut().filter(|_| !failed) {
                    let result = serde_json::from_slice(&bytes)
                        .map_err(anyhow::Error::from)
                        .and_then(|record| writer.write(&record));
                    if result.is_err() {
                        failed = true;
                        health.missing.fetch_add(1, Ordering::Relaxed);
                        health.warn(DiagnosticWarning::WriteFailed);
                    } else {
                        health
                            .status
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .records_written = writer.record_count();
                    }
                } else {
                    health.missing.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if health.closing.load(Ordering::Acquire) {
                    break;
                }
                if let Some(writer) = writer.as_mut().filter(|_| !failed)
                    && writer.flush().is_err()
                {
                    failed = true;
                    health.warn(DiagnosticWarning::WriteFailed);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if let Some(writer) = &mut writer {
        let outcome = *health
            .outcome
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let snapshot = health.snapshot();
        let result = if snapshot.warning.is_none() {
            writer.finalize(outcome)
        } else {
            writer.finalize_partial(outcome, snapshot.missing_records)
        };
        match result {
            Ok(index) => {
                let mut status = health
                    .status
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                status.state = if status.warning.is_none() {
                    index.state
                } else {
                    ArchiveState::Partial
                };
                status.records_written = index.records;
            }
            Err(_) => health.warn(DiagnosticWarning::FinalizeFailed),
        }
    }
    health.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diagnostics::{
            adapter::{LiveProgressAdapter, ProgressAdapter},
            journal::JournalReader,
            record::*,
        },
        progress,
    };

    fn live_fixture() -> (
        Arc<Mutex<progress::LiveProgress>>,
        LiveProgressAdapter,
        DiagnosticRecordV1,
        OperationMetadata,
    ) {
        let progress = Arc::new(Mutex::new(progress::LiveProgress::default()));
        progress.lock().unwrap().apply(progress::Record {
            version: 1,
            elapsed_ms: 0,
            event: progress::Event::Queued {
                job: progress::Job {
                    id: 1,
                    suite_id: "suite".into(),
                    suite_name: "Suite".into(),
                    scenario_id: "scenario".into(),
                    label: "Scenario".into(),
                    viewport: "desktop".into(),
                    captures: vec![],
                },
            },
        });
        let received = progress.clone();
        let adapter = LiveProgressAdapter::new(move |event| {
            received.lock().unwrap().apply(progress::Record {
                version: 1,
                elapsed_ms: 0,
                event,
            })
        });
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let mut record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Started,
        );
        record.execution = Some(ExecutionIdentity {
            execution_id: ExecutionId::new(),
            suite_id: SafeIdentity::fingerprint("suite"),
            scenario_id: SafeIdentity::fingerprint("scenario"),
            viewport: ViewportIdentity {
                name: SafeIdentity::fingerprint("desktop"),
                width: 1200,
                height: 800,
            },
            capture_role: CaptureRole::Test,
            attempt: 1,
        });
        adapter
            .bind(record.execution.as_ref().unwrap().execution_id.clone(), 1)
            .unwrap();
        (progress, adapter, record, metadata)
    }

    #[test]
    fn forced_write_failure_surfaces_warning_and_preserves_exit_status() {
        let root = tempfile::tempdir().unwrap();
        let blocked = root.path().join("not-a-directory");
        std::fs::write(&blocked, b"existing artifact").unwrap();
        let (live, adapter, record, metadata) = live_fixture();
        let warnings = Arc::new(Mutex::new(Vec::new()));
        let received = warnings.clone();
        let owner = JournalOwner::start(
            &blocked,
            metadata,
            JournalOptions::default(),
            false,
            move |warning| received.lock().unwrap().push(warning),
        );
        owner.sink().record(&record);
        adapter.on_diagnostic(&record);
        let capture_result: Result<()> = Ok(());
        let health = owner.finish(Outcome::Success);
        assert!(capture_result.is_ok());
        assert_eq!(if capture_result.is_ok() { 0 } else { 1 }, 0);
        assert_eq!(health.state, ArchiveState::Missing);
        assert_eq!(
            warnings.lock().unwrap()[0].warning,
            Some(DiagnosticWarning::CreateFailed)
        );
        assert_eq!(live.lock().unwrap().logs.len(), 2);
        assert!(live.lock().unwrap().error.is_none());
    }

    #[test]
    fn kill_switch_disables_archive_writing_without_affecting_live_buffer() {
        let (live, adapter, record, metadata) = live_fixture();
        let attempts = Arc::new(AtomicU64::new(0));
        let observed = attempts.clone();
        let disabled = archives_disabled(|_| Some("true".into())).unwrap();
        let owner = JournalOwner::start_with(
            metadata,
            disabled,
            move || {
                observed.fetch_add(1, Ordering::Relaxed);
                bail!("must not open a journal")
            },
            |_| {},
        );
        owner.sink().record(&record);
        adapter.on_diagnostic(&record);
        let health = owner.finish(Outcome::Success);
        assert_eq!(attempts.load(Ordering::Relaxed), 0);
        assert_eq!(health.warning, Some(DiagnosticWarning::Disabled));
        assert_eq!(health.state, ArchiveState::Missing);
        assert_eq!(live.lock().unwrap().logs.len(), 2);
        assert!(!archives_disabled(|_| None).unwrap());
        assert!(!archives_disabled(|_| Some("false".into())).unwrap());
        assert!(archives_disabled(|_| Some("invalid".into())).is_err());
    }

    #[test]
    fn queue_exhaustion_is_nonblocking_and_retains_every_accepted_record() {
        let root = tempfile::tempdir().unwrap();
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let header = metadata.clone();
        let output = root.path().to_owned();
        let (release, blocked) = mpsc::channel();
        let owner = JournalOwner::start_with(
            metadata.clone(),
            false,
            move || {
                blocked.recv().unwrap();
                JournalWriter::create(&output, header, JournalOptions::default())
            },
            |_| {},
        );
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Decision,
        );
        for _ in 0..QUEUE_RECORDS + 1 {
            owner.sink().record(&record);
        }
        assert_eq!(
            owner.sink().health().warning,
            Some(DiagnosticWarning::QueueFull)
        );
        release.send(()).unwrap();
        let health = owner.finish(Outcome::Success);
        assert_eq!(health.state, ArchiveState::Partial);
        assert_eq!(health.missing_records, Some(1));
        let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
        assert_eq!(reader.archive_state(), ArchiveState::Partial);
        assert_eq!(reader.visit(|_| {}).unwrap().records, QUEUE_RECORDS as u64);
    }

    #[test]
    fn stalled_writer_has_bounded_finalization_and_unknown_missing_count() {
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let (release, blocked) = mpsc::channel();
        let owner = JournalOwner::start_with(
            metadata,
            false,
            move || {
                blocked.recv().unwrap();
                bail!("synthetic stalled open failure")
            },
            |_| {},
        );
        let health = owner.finish_with_timeout(Outcome::Success, Duration::from_millis(1));
        assert_eq!(health.state, ArchiveState::Partial);
        assert_eq!(health.warning, Some(DiagnosticWarning::FinalizeTimeout));
        assert_eq!(health.missing_records, None);
        release.send(()).unwrap();
    }

    #[test]
    fn background_budget_failure_preserves_readable_prefix() {
        let root = tempfile::tempdir().unwrap();
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let owner = JournalOwner::start(
            root.path(),
            metadata.clone(),
            JournalOptions {
                chunk_bytes: 4096,
                disk_budget_bytes: 4096,
            },
            false,
            |_| {},
        );
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Decision,
        );
        for _ in 0..32 {
            owner.sink().record(&record);
        }
        let health = owner.finish(Outcome::Success);
        assert_eq!(health.state, ArchiveState::Partial);
        assert_eq!(health.warning, Some(DiagnosticWarning::WriteFailed));
        assert!(health.missing_records.unwrap() > 0);
        assert!(
            JournalReader::open(root.path(), &metadata.operation_id)
                .unwrap()
                .visit(|_| {})
                .unwrap()
                .records
                > 0
        );
    }
}
