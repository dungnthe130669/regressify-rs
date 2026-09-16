//! Structured run telemetry, independent of console output and the UI framework.

use std::{
    collections::{BTreeMap, HashMap},
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    sync::{Mutex, OnceLock},
    time::Instant,
};

use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize};

const VERSION: u8 = 1;
const MAX_EVENT_BYTES: usize = 1024 * 1024;
static SINK: OnceLock<Mutex<EventWriter>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: usize,
    pub suite_id: String,
    pub suite_name: String,
    pub scenario_id: String,
    pub label: String,
    pub viewport: String,
    pub captures: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    Phase {
        name: String,
    },
    SuiteQueued {
        id: String,
        name: String,
    },
    Queued {
        job: Job,
    },
    Started {
        id: usize,
    },
    Captured {
        id: usize,
        error: Option<String>,
        infrastructure: bool,
    },
    Comparing {
        suite_id: String,
    },
    Compared {
        suite_id: String,
        capture: String,
        label: String,
        passed: bool,
        error: Option<String>,
    },
    SuiteFinished {
        id: String,
    },
    Finished {
        error: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub version: u8,
    pub elapsed_ms: u64,
    pub event: Event,
}

struct EventWriter {
    output: BufWriter<File>,
    started: Instant,
    error: Option<String>,
}

impl EventWriter {
    fn write(&mut self, event: Event) -> Result<()> {
        let record = Record {
            version: VERSION,
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            event,
        };
        let bytes = serde_json::to_vec(&record)?;
        ensure!(
            bytes.len() <= MAX_EVENT_BYTES,
            "Progress event exceeds 1 MiB"
        );
        self.output.write_all(&bytes)?;
        self.output.write_all(b"\n")?;
        self.output.flush()?;
        Ok(())
    }
}

/// The parent creates this private file; the worker only opens the existing path.
pub fn install(path: &Path) -> Result<()> {
    let output = OpenOptions::new().write(true).truncate(true).open(path)?;
    SINK.set(Mutex::new(EventWriter {
        output: BufWriter::new(output),
        started: Instant::now(),
        error: None,
    }))
    .map_err(|_| anyhow!("Progress transport is already initialized"))
}

pub fn enabled() -> bool {
    SINK.get().is_some()
}

/// Telemetry failures must not interrupt sibling scenarios; finish reports them.
pub fn emit(event: Event) {
    if let Some(sink) = SINK.get()
        && let Ok(mut sink) = sink.lock()
        && sink.error.is_none()
        && let Err(error) = sink.write(event)
    {
        sink.error = Some(error.to_string());
    }
}

pub fn finish() -> Result<()> {
    if let Some(sink) = SINK.get() {
        let mut sink = sink
            .lock()
            .map_err(|_| anyhow!("Progress writer is unavailable"))?;
        if let Some(error) = &sink.error {
            return Err(anyhow!("Progress transport failed: {error}"));
        }
        sink.output.flush()?;
    }
    Ok(())
}

pub struct EventReader {
    input: BufReader<File>,
    pending: Vec<u8>,
}

impl EventReader {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            input: BufReader::new(File::open(path)?),
            pending: Vec::new(),
        })
    }

    /// Regular-file EOF is temporary while the worker is still appending records.
    pub fn drain(&mut self, mut receive: impl FnMut(Record)) -> Result<()> {
        loop {
            let available = self.input.fill_buf()?;
            if available.is_empty() {
                return Ok(());
            }
            let end = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|position| position + 1)
                .unwrap_or(available.len());
            self.pending.extend_from_slice(&available[..end]);
            self.input.consume(end);
            ensure!(
                self.pending.len() <= MAX_EVENT_BYTES + 1,
                "Progress record exceeds 1 MiB"
            );
            if self.pending.last() == Some(&b'\n') {
                let record: Record = serde_json::from_slice(&self.pending)?;
                ensure!(
                    record.version == VERSION,
                    "Unsupported progress protocol version"
                );
                receive(record);
                self.pending.clear();
            }
        }
    }

    pub fn complete(&self) -> Result<()> {
        ensure!(
            self.pending.is_empty(),
            "Worker ended with an incomplete progress record"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Queued,
    Running,
    Captured,
    Comparing,
    Passed,
    Failed,
    Cancelled,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Captured => "Captured",
            Self::Comparing => "Comparing",
            Self::Passed => "Passed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
    pub fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Comparing)
    }
}

#[derive(Clone, Debug)]
pub struct ScenarioProgress {
    pub job: Job,
    pub status: Status,
    pub message: Option<String>,
    pub infrastructure: bool,
    pub started_ms: Option<u64>,
    pub finished_ms: Option<u64>,
    compared: HashMap<String, bool>,
}

#[derive(Clone, Debug)]
pub struct SuiteProgress {
    pub id: String,
    pub name: String,
    pub finished: bool,
}

#[derive(Clone, Debug, Default)]
pub struct LiveProgress {
    pub phase: String,
    pub suites: BTreeMap<String, SuiteProgress>,
    pub scenarios: BTreeMap<usize, ScenarioProgress>,
    pub elapsed_ms: u64,
    pub finished: bool,
    pub error: Option<String>,
    captures: HashMap<(String, String), usize>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub total: usize,
    pub queued: usize,
    pub running: usize,
    pub captured: usize,
    pub passed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

impl Counts {
    pub fn complete(self) -> usize {
        self.captured + self.passed + self.failed + self.cancelled
    }
    pub fn fraction(self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.complete() as f32 / self.total as f32
        }
    }
}

impl LiveProgress {
    pub fn apply(&mut self, record: Record) {
        self.elapsed_ms = self.elapsed_ms.max(record.elapsed_ms);
        match record.event {
            Event::Phase { name } => self.phase = name,
            Event::SuiteQueued { id, name } => {
                self.suites.entry(id.clone()).or_insert(SuiteProgress {
                    id,
                    name,
                    finished: false,
                });
            }
            Event::Queued { job } => self.queue(job),
            Event::Started { id } => {
                if let Some(scenario) = self.scenarios.get_mut(&id) {
                    scenario.status = Status::Running;
                    scenario.started_ms = Some(record.elapsed_ms);
                }
            }
            Event::Captured {
                id,
                error,
                infrastructure,
            } => {
                if let Some(scenario) = self.scenarios.get_mut(&id) {
                    scenario.status = if error.is_some() {
                        Status::Failed
                    } else {
                        Status::Captured
                    };
                    scenario.message = error;
                    scenario.infrastructure = infrastructure;
                    scenario.finished_ms = Some(record.elapsed_ms);
                }
            }
            Event::Comparing { suite_id } => {
                self.phase = "Comparing screenshots".into();
                self.suites
                    .entry(suite_id.clone())
                    .or_insert(SuiteProgress {
                        id: suite_id.clone(),
                        name: suite_id.clone(),
                        finished: false,
                    });
                for scenario in self.scenarios.values_mut().filter(|scenario| {
                    scenario.job.suite_id == suite_id && scenario.status == Status::Captured
                }) {
                    scenario.status = Status::Comparing;
                }
            }
            Event::Compared {
                suite_id,
                capture,
                label,
                passed,
                error,
            } => {
                let key = (suite_id.clone(), capture.clone());
                let id = if let Some(id) = self.captures.get(&key) {
                    *id
                } else {
                    let id = self.scenarios.keys().next_back().copied().unwrap_or(0) + 1;
                    self.queue(Job {
                        id,
                        suite_id: suite_id.clone(),
                        suite_name: suite_id,
                        scenario_id: capture.clone(),
                        label,
                        viewport: String::new(),
                        captures: vec![capture.clone()],
                    });
                    id
                };
                if let Some(scenario) = self.scenarios.get_mut(&id) {
                    scenario.compared.insert(capture, passed);
                    if scenario.status != Status::Failed {
                        scenario.status = if !passed {
                            Status::Failed
                        } else if scenario.compared.len() == scenario.job.captures.len() {
                            Status::Passed
                        } else {
                            Status::Comparing
                        };
                        if error.is_some() {
                            scenario.message = error;
                        }
                    }
                    if !scenario.status.active() {
                        scenario.finished_ms = Some(record.elapsed_ms);
                    }
                }
            }
            Event::SuiteFinished { id } => {
                if let Some(suite) = self.suites.get_mut(&id) {
                    suite.finished = true;
                }
            }
            Event::Finished { error } => self.end(false, error),
        }
    }

    fn queue(&mut self, job: Job) {
        self.suites
            .entry(job.suite_id.clone())
            .or_insert(SuiteProgress {
                id: job.suite_id.clone(),
                name: job.suite_name.clone(),
                finished: false,
            });
        for capture in &job.captures {
            self.captures
                .insert((job.suite_id.clone(), capture.clone()), job.id);
        }
        self.scenarios.entry(job.id).or_insert(ScenarioProgress {
            job,
            status: Status::Queued,
            message: None,
            infrastructure: false,
            started_ms: None,
            finished_ms: None,
            compared: HashMap::new(),
        });
    }

    pub fn end(&mut self, cancelled: bool, error: Option<String>) {
        self.finished = true;
        if cancelled {
            self.phase = "Cancelled".into();
        } else if error.is_some() {
            self.phase = "Run failed".into();
        } else {
            self.phase = "Finished".into();
        }
        if error.is_some() {
            self.error = error.clone();
        }
        for scenario in self
            .scenarios
            .values_mut()
            .filter(|scenario| scenario.status.active())
        {
            scenario.status = if cancelled {
                Status::Cancelled
            } else {
                Status::Failed
            };
            scenario.infrastructure = !cancelled;
            scenario.message = error.clone().or_else(|| {
                Some(
                    if cancelled {
                        "Run cancelled"
                    } else {
                        "Worker ended before this capture finished"
                    }
                    .into(),
                )
            });
            scenario.finished_ms = Some(self.elapsed_ms);
        }
    }

    pub fn counts(&self, suite_id: Option<&str>) -> Counts {
        let mut counts = Counts::default();
        for scenario in self
            .scenarios
            .values()
            .filter(|scenario| suite_id.is_none_or(|id| scenario.job.suite_id == id))
        {
            counts.total += 1;
            match scenario.status {
                Status::Queued => counts.queued += 1,
                Status::Running | Status::Comparing => counts.running += 1,
                Status::Captured => counts.captured += 1,
                Status::Passed => counts.passed += 1,
                Status::Failed => counts.failed += 1,
                Status::Cancelled => counts.cancelled += 1,
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(event: Event) -> Record {
        Record {
            version: VERSION,
            elapsed_ms: 100,
            event,
        }
    }
    fn job(id: usize, suite: &str) -> Job {
        Job {
            id,
            suite_id: suite.into(),
            suite_name: suite.into(),
            scenario_id: format!("scenario-{id}"),
            label: format!("Scenario {id}"),
            viewport: "desktop".into(),
            captures: vec![format!("capture-{id}")],
        }
    }

    #[test]
    fn parallel_jobs_keep_independent_suite_and_scenario_statuses() {
        let mut live = LiveProgress::default();
        live.apply(record(Event::Queued {
            job: job(1, "shop"),
        }));
        live.apply(record(Event::Queued {
            job: job(2, "account"),
        }));
        live.apply(record(Event::Started { id: 2 }));
        assert_eq!(live.counts(Some("shop")).queued, 1);
        assert_eq!(live.counts(Some("account")).running, 1);
        live.apply(record(Event::Captured {
            id: 2,
            error: Some("Missing #login".into()),
            infrastructure: false,
        }));
        live.apply(record(Event::Started { id: 1 }));
        live.apply(record(Event::Captured {
            id: 1,
            error: None,
            infrastructure: false,
        }));
        assert_eq!(live.counts(None).captured, 1);
        assert_eq!(live.counts(None).failed, 1);
        assert_eq!(
            live.scenarios[&2].message.as_deref(),
            Some("Missing #login")
        );
    }

    #[test]
    fn visual_pass_waits_for_every_screenshot_and_failure_never_flips_to_pass() {
        let mut live = LiveProgress::default();
        let mut details = job(1, "shop");
        details.captures = vec!["step".into(), "final".into()];
        live.apply(record(Event::Queued { job: details }));
        live.apply(record(Event::Captured {
            id: 1,
            error: None,
            infrastructure: false,
        }));
        live.apply(record(Event::Comparing {
            suite_id: "shop".into(),
        }));
        for (capture, passed) in [("step", true), ("final", false), ("step", true)] {
            live.apply(record(Event::Compared {
                suite_id: "shop".into(),
                capture: capture.into(),
                label: "Home".into(),
                passed,
                error: (!passed).then(|| "Visual mismatch".into()),
            }));
            assert_ne!(live.scenarios[&1].status, Status::Passed);
        }
        assert_eq!(live.scenarios[&1].status, Status::Failed);
    }

    #[test]
    fn cancellation_and_infrastructure_failure_resolve_pending_jobs() {
        for cancelled in [true, false] {
            let mut live = LiveProgress::default();
            live.apply(record(Event::Queued {
                job: job(1, "shop"),
            }));
            live.apply(record(Event::Queued {
                job: job(2, "shop"),
            }));
            live.apply(record(Event::Started { id: 1 }));
            live.end(
                cancelled,
                (!cancelled).then(|| "Browser unavailable".into()),
            );
            assert_eq!(live.counts(None).complete(), 2);
            assert_eq!(live.counts(None).running, 0);
            assert_eq!(
                live.scenarios[&1].status,
                if cancelled {
                    Status::Cancelled
                } else {
                    Status::Failed
                }
            );
        }
    }

    #[test]
    fn progress_reader_retains_partial_records_and_rejects_invalid_protocol() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut reader = EventReader::open(file.path()).unwrap();
        let mut output = OpenOptions::new().append(true).open(file.path()).unwrap();
        let bytes = serde_json::to_vec(&record(Event::Started { id: 1 })).unwrap();
        output.write_all(&bytes[..10]).unwrap();
        let mut events = Vec::new();
        reader.drain(|event| events.push(event)).unwrap();
        assert!(events.is_empty());
        assert!(reader.complete().is_err());
        output.write_all(&bytes[10..]).unwrap();
        output.write_all(b"\n").unwrap();
        reader.drain(|event| events.push(event)).unwrap();
        reader.complete().unwrap();
        assert_eq!(events.len(), 1);
        output
            .write_all(
                b"{\"version\":99,\"elapsedMs\":0,\"event\":{\"event\":\"started\",\"id\":1}}\n",
            )
            .unwrap();
        assert!(reader.drain(|_| {}).is_err());
    }

    #[test]
    fn comparison_only_runs_can_populate_live_rows() {
        let mut live = LiveProgress::default();
        live.apply(record(Event::Comparing {
            suite_id: "existing".into(),
        }));
        live.apply(record(Event::Compared {
            suite_id: "existing".into(),
            capture: "home".into(),
            label: "Home".into(),
            passed: true,
            error: None,
        }));
        assert_eq!(live.counts(Some("existing")).passed, 1);
        live.apply(record(Event::Finished { error: None }));
        assert_eq!(live.scenarios[&1].status, Status::Passed);
    }
}
