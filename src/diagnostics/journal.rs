use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::record::{DiagnosticEvent, Outcome, Stage};

use super::record::{CaptureRunId, DiagnosticRecordV1, OperationId, OperationKind};

pub const DEFAULT_CHUNK_BYTES: usize = 512 * 1024;
pub const DEFAULT_DISK_BUDGET_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_RECORD_BYTES: usize = 64 * 1024;
pub const BUFFER_BYTES: usize = 64 * 1024;
pub const MAX_SEGMENTS: usize = 16_384;
pub const CHUNK_ENV: &str = "REGRESSIFY_DIAGNOSTIC_CHUNK_SIZE_BYTES";
pub const BUDGET_ENV: &str = "REGRESSIFY_DIAGNOSTIC_DISK_BUDGET_BYTES";
const JOURNAL_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveState {
    Complete,
    Partial,
    Missing,
    LegacyUnavailable,
    LoadingError,
}

impl ArchiveState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Complete => "Complete",
            Self::Partial => "Partial / interrupted",
            Self::Missing => "Expected log is missing",
            Self::LegacyUnavailable => "Legacy capture: log not available",
            Self::LoadingError => "Log could not be loaded",
        }
    }

    pub fn can_export(self) -> bool {
        self == Self::Complete
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentInfo {
    pub number: usize,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveIndex {
    pub version: u16,
    pub operation_id: OperationId,
    pub state: ArchiveState,
    pub records: u64,
    pub bytes: u64,
    pub segments: Vec<SegmentInfo>,
    pub outcome: Outcome,
    pub missing_records: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandoffNotice {
    Attached,
    RetainedSource,
}

fn segment_info(path: &Path, root: &Path, number: usize) -> Result<SegmentInfo> {
    let mut file = checked_file(path, root)?;
    ensure!(
        file.metadata()?.len() <= 8 * 1024 * 1024,
        "Diagnostic segment exceeds byte limit"
    );
    let mut digest = Sha256::new();
    let mut buffer = [0; BUFFER_BYTES];
    let mut bytes = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        ensure!(
            bytes <= 8 * 1024 * 1024,
            "Diagnostic segment grew beyond byte limit"
        );
        digest.update(&buffer[..count]);
    }
    Ok(SegmentInfo {
        number,
        bytes,
        sha256: format!("{:x}", digest.finalize()),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalOptions {
    pub chunk_bytes: usize,
    pub disk_budget_bytes: u64,
}

impl Default for JournalOptions {
    fn default() -> Self {
        Self {
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            disk_budget_bytes: DEFAULT_DISK_BUDGET_BYTES,
        }
    }
}

impl JournalOptions {
    pub fn resolve(
        chunk: Option<usize>,
        budget: Option<u64>,
        environment: impl Fn(&str) -> Option<String>,
    ) -> Result<Self> {
        let chunk_bytes = match chunk {
            Some(chunk) => chunk,
            None => environment(CHUNK_ENV)
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| anyhow!("Invalid {CHUNK_ENV}"))
                })
                .transpose()?
                .unwrap_or(DEFAULT_CHUNK_BYTES),
        };
        let disk_budget_bytes = match budget {
            Some(budget) => budget,
            None => environment(BUDGET_ENV)
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| anyhow!("Invalid {BUDGET_ENV}"))
                })
                .transpose()?
                .unwrap_or(DEFAULT_DISK_BUDGET_BYTES),
        };
        let options = Self {
            chunk_bytes,
            disk_budget_bytes,
        };
        options.validate()?;
        Ok(options)
    }

    pub fn validate(self) -> Result<()> {
        ensure!(
            (4096..=8 * 1024 * 1024).contains(&self.chunk_bytes),
            "Diagnostic chunk size must be between 4096 and 8388608 bytes"
        );
        ensure!(
            (4096..=16 * 1024 * 1024 * 1024).contains(&self.disk_budget_bytes),
            "Diagnostic disk budget must be between 4096 bytes and 16 GiB"
        );
        ensure!(
            self.disk_budget_bytes >= self.chunk_bytes as u64,
            "Diagnostic disk budget must cover at least one chunk"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildIdentity {
    pub version: String,
    pub source_commit: String,
    pub dirty: Option<bool>,
}

impl Default for BuildIdentity {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            source_commit: env!("BINARY_GIT_HASH").into(),
            dirty: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationMetadata {
    pub version: u16,
    pub operation_id: OperationId,
    pub operation_kind: OperationKind,
    pub capture_run_id: Option<CaptureRunId>,
    pub timestamp_utc_ms: i64,
    pub build: BuildIdentity,
}

impl OperationMetadata {
    pub fn new(kind: OperationKind, run_id: Option<CaptureRunId>) -> Self {
        Self {
            version: JOURNAL_VERSION,
            operation_id: OperationId::new(),
            operation_kind: kind,
            capture_run_id: run_id,
            timestamp_utc_ms: chrono::Utc::now().timestamp_millis(),
            build: BuildIdentity::default(),
        }
    }
}

pub fn archive_directory(output: &Path, operation: &OperationId) -> PathBuf {
    output.join("_diagnostics").join(operation.as_str())
}

pub fn inspect_archive(
    output: &Path,
    operation: Option<&OperationId>,
    archive_aware: bool,
) -> ArchiveState {
    if !archive_aware {
        return ArchiveState::LegacyUnavailable;
    }
    let Some(operation) = operation else {
        return ArchiveState::Missing;
    };
    inspect_present_archive(output, operation).unwrap_or(ArchiveState::LoadingError)
}

fn inspect_present_archive(output: &Path, operation: &OperationId) -> Result<ArchiveState> {
    let directory = archive_directory(output, operation);
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ArchiveState::Missing);
        }
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    let root = output.canonicalize()?;
    checked_directory(output, &root)?;
    checked_directory(&root.join("_diagnostics"), &root)?;
    let directory = checked_directory(&directory, &root)?;
    if fs::read_dir(&directory)?.next().transpose()?.is_none() {
        return Ok(ArchiveState::Missing);
    }
    if matches!(fs::symlink_metadata(directory.join("operation.json")), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        let marker: ArchiveIndex =
            read_json(&directory.join("index.json"), &directory, MAX_RECORD_BYTES)?;
        ensure!(
            marker.version == JOURNAL_VERSION
                && marker.operation_id == *operation
                && marker.state == ArchiveState::Partial
                && marker.records == 0
                && marker.segments.is_empty(),
            "Invalid diagnostic partial marker"
        );
        return Ok(ArchiveState::Partial);
    }
    Ok(JournalReader::open(&root, operation)?.archive_state())
}

fn checked_directory(path: &Path, root: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Diagnostic directory must not be a symlink"
    );
    let canonical = path.canonicalize()?;
    ensure!(
        canonical.starts_with(root),
        "Diagnostic directory escapes output root"
    );
    Ok(canonical)
}

pub(crate) fn checked_file(path: &Path, root: &Path) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Diagnostic asset must be a regular file"
    );
    ensure!(
        path.canonicalize()?.starts_with(root),
        "Diagnostic asset escapes output root"
    );
    Ok(File::open(path)?)
}

pub(crate) fn read_json<Output: serde::de::DeserializeOwned>(
    path: &Path,
    root: &Path,
    limit: usize,
) -> Result<Output> {
    let mut bytes = Vec::new();
    checked_file(path, root)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "Diagnostic metadata exceeds size limit"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len() + bytes.len() > self.limit {
            return Err(std::io::Error::other(
                "Diagnostic record exceeds byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct JournalWriter {
    directory: PathBuf,
    metadata: OperationMetadata,
    options: JournalOptions,
    output: Option<BufWriter<File>>,
    segment_count: usize,
    segment_bytes: usize,
    bytes: u64,
    records: u64,
    peak_buffer_bytes: usize,
    last_flush: Instant,
    failed: bool,
    finalized: bool,
}

impl JournalWriter {
    pub fn create(
        output: &Path,
        metadata: OperationMetadata,
        options: JournalOptions,
    ) -> Result<Self> {
        options.validate()?;
        ensure!(
            metadata.version == JOURNAL_VERSION,
            "Unsupported diagnostic journal version"
        );
        fs::create_dir_all(output)?;
        let root = output.canonicalize()?;
        checked_directory(output, &root)?;
        let diagnostics = root.join("_diagnostics");
        match fs::create_dir(&diagnostics) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        checked_directory(&diagnostics, &root)?;
        let directory = diagnostics.join(metadata.operation_id.as_str());
        fs::create_dir(&directory)
            .context("Diagnostic operation already exists or cannot be created")?;
        let directory = checked_directory(&directory, &root)?;
        let bytes = serde_json::to_vec(&metadata)?;
        ensure!(
            bytes.len() as u64 <= options.disk_budget_bytes,
            "Diagnostic metadata exceeds disk budget"
        );
        crate::helpers::write_file_atomically(&directory.join("operation.json"), &bytes)?;
        Ok(Self {
            directory,
            metadata,
            options,
            output: None,
            segment_count: 0,
            segment_bytes: 0,
            bytes: bytes.len() as u64,
            records: 0,
            peak_buffer_bytes: 0,
            last_flush: Instant::now(),
            failed: false,
            finalized: false,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn metadata(&self) -> &OperationMetadata {
        &self.metadata
    }
    pub fn record_count(&self) -> u64 {
        self.records
    }
    pub fn segment_count(&self) -> usize {
        self.segment_count
    }
    pub fn byte_count(&self) -> u64 {
        self.bytes
    }
    pub fn peak_buffer_bytes(&self) -> usize {
        self.peak_buffer_bytes
    }

    pub fn write(&mut self, record: &DiagnosticRecordV1) -> Result<()> {
        ensure!(
            !self.failed && !self.finalized,
            "Diagnostic writer is degraded or finalized"
        );
        let result = self.append(record);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn append(&mut self, record: &DiagnosticRecordV1) -> Result<()> {
        record.validate()?;
        ensure!(
            record.operation_id == self.metadata.operation_id
                && record.operation_kind == self.metadata.operation_kind,
            "Record belongs to another diagnostic operation"
        );
        ensure!(
            record.capture_run_id.is_none()
                || record.capture_run_id == self.metadata.capture_run_id,
            "Record belongs to another capture run"
        );
        let mut record = record.clone();
        record.sequence = self.records;
        record.capture_run_id = self.metadata.capture_run_id.clone();
        let mut encoded = BoundedBytes {
            bytes: Vec::new(),
            limit: MAX_RECORD_BYTES.min(self.options.chunk_bytes) - 1,
        };
        serde_json::to_writer(&mut encoded, &record)?;
        encoded.bytes.push(b'\n');
        ensure!(
            self.bytes + encoded.bytes.len() as u64 <= self.options.disk_budget_bytes,
            "Diagnostic disk budget exhausted"
        );
        self.peak_buffer_bytes = self
            .peak_buffer_bytes
            .max(BUFFER_BYTES + encoded.bytes.capacity());
        if self.output.is_none()
            || self.segment_bytes + encoded.bytes.len() > self.options.chunk_bytes
        {
            self.rollover()?;
        }
        self.output
            .as_mut()
            .context("Diagnostic segment is unavailable")?
            .write_all(&encoded.bytes)?;
        self.segment_bytes += encoded.bytes.len();
        self.bytes += encoded.bytes.len() as u64;
        self.records += 1;
        if self.records.is_multiple_of(64)
            || self.last_flush.elapsed() >= Duration::from_millis(250)
        {
            self.flush()?;
        }
        Ok(())
    }

    fn rollover(&mut self) -> Result<()> {
        self.flush()?;
        if let Some(output) = &self.output {
            output.get_ref().sync_all()?;
        }
        self.output = None;
        ensure!(
            self.segment_count < MAX_SEGMENTS,
            "Diagnostic segment limit reached"
        );
        let path = self
            .directory
            .join(format!("segment-{:06}.jsonl", self.segment_count));
        let output = OpenOptions::new().create_new(true).write(true).open(path)?;
        self.output = Some(BufWriter::with_capacity(BUFFER_BYTES, output));
        self.segment_count += 1;
        self.segment_bytes = 0;
        Ok(())
    }

    pub fn handoff_bootstrap(
        mut self,
        destination: &Path,
        mut notify: impl FnMut(HandoffNotice),
    ) -> Self {
        let expected = archive_directory(destination, &self.metadata.operation_id);
        if expected.canonicalize().ok().as_ref() == Some(&self.directory) {
            return self;
        }
        match self.handoff_inner(destination) {
            Ok(()) => notify(HandoffNotice::Attached),
            Err(_) => notify(HandoffNotice::RetainedSource),
        }
        self
    }

    fn handoff_inner(&mut self, destination: &Path) -> Result<()> {
        ensure!(
            !self.failed && !self.finalized,
            "Cannot hand off a degraded or finalized bootstrap journal"
        );
        self.flush()?;
        if let Some(output) = &self.output {
            output.get_ref().sync_all()?;
        }
        fs::create_dir_all(destination)?;
        let root = destination.canonicalize()?;
        checked_directory(destination, &root)?;
        let diagnostics = root.join("_diagnostics");
        match fs::create_dir(&diagnostics) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        checked_directory(&diagnostics, &root)?;
        let directory = archive_directory(&root, &self.metadata.operation_id);
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(anyhow!(
                    "Bootstrap destination already exists or is inaccessible"
                ));
            }
        }
        let temporary = tempfile::Builder::new()
            .prefix(".bootstrap-")
            .tempdir_in(&diagnostics)?;
        let mut names = vec!["operation.json".to_owned()];
        names.extend((0..self.segment_count).map(|number| format!("segment-{number:06}.jsonl")));
        for name in &names {
            let source_path = self.directory.join(name);
            let before = segment_info(&source_path, &self.directory, 0)?;
            let source = checked_file(&source_path, &self.directory)?;
            let destination_path = temporary.path().join(name);
            let mut target = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&destination_path)?;
            let count = std::io::copy(&mut source.take(before.bytes + 1), &mut target)?;
            target.sync_all()?;
            drop(target);
            let after = segment_info(&destination_path, temporary.path(), 0)?;
            ensure!(
                before == after && count == before.bytes,
                "Bootstrap copy verification failed"
            );
        }
        fs::rename(temporary.path(), &directory)?;
        let directory = checked_directory(&directory, &root)?;
        let reader = JournalReader::open(&root, &self.metadata.operation_id)?;
        let summary = reader.visit(|_| {})?;
        ensure!(
            summary.records == self.records && !summary.interrupted_tail,
            "Bootstrap record verification failed"
        );
        let output = if self.segment_count == 0 {
            None
        } else {
            let path = directory.join(format!("segment-{:06}.jsonl", self.segment_count - 1));
            checked_file(&path, &directory)?;
            Some(BufWriter::with_capacity(
                BUFFER_BYTES,
                OpenOptions::new().append(true).open(path)?,
            ))
        };
        crate::helpers::write_file_atomically(
            &self.directory.join("handoff.json"),
            &serde_json::to_vec(
                &serde_json::json!({"operationId": self.metadata.operation_id, "records": self.records}),
            )?,
        )?;
        self.directory = directory;
        self.output = output;
        Ok(())
    }

    pub fn finalize(&mut self, outcome: Outcome) -> Result<ArchiveIndex> {
        ensure!(
            !self.failed && !self.finalized,
            "Cannot complete a degraded or finalized diagnostic archive"
        );
        let result = self.finalize_inner(outcome);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn finalize_inner(&mut self, outcome: Outcome) -> Result<ArchiveIndex> {
        let mut terminal = DiagnosticRecordV1::new(
            self.metadata.operation_id.clone(),
            self.metadata.operation_kind,
            Stage::Finish,
            DiagnosticEvent::Finished,
        );
        terminal.severity = outcome.severity();
        terminal.outcome = Some(outcome);
        self.write(&terminal)?;
        self.flush()?;
        if let Some(output) = &self.output {
            output.get_ref().sync_all()?;
        }
        self.persist_index(ArchiveState::Complete, outcome, Some(0))
    }

    pub fn finalize_partial(
        &mut self,
        outcome: Outcome,
        missing_records: Option<u64>,
    ) -> Result<ArchiveIndex> {
        ensure!(!self.finalized, "Diagnostic archive already finalized");
        self.flush()?;
        if let Some(output) = &self.output {
            output.get_ref().sync_all()?;
        }
        self.persist_index(ArchiveState::Partial, outcome, missing_records)
    }

    fn persist_index(
        &mut self,
        state: ArchiveState,
        outcome: Outcome,
        missing_records: Option<u64>,
    ) -> Result<ArchiveIndex> {
        let root = self
            .directory
            .parent()
            .and_then(Path::parent)
            .context("Diagnostic output root unavailable")?;
        let reader = JournalReader::open(root, &self.metadata.operation_id)?;
        let summary = reader.visit(|_| {})?;
        ensure!(
            state != ArchiveState::Complete
                || (summary.records == self.records && !summary.interrupted_tail),
            "Diagnostic journal is not fully written"
        );
        let segments = reader
            .segments
            .iter()
            .enumerate()
            .map(|(number, path)| segment_info(path, &self.directory, number))
            .collect::<Result<Vec<_>>>()?;
        let index = ArchiveIndex {
            version: JOURNAL_VERSION,
            operation_id: self.metadata.operation_id.clone(),
            state,
            records: summary.records,
            bytes: self.bytes,
            segments,
            outcome,
            missing_records,
        };
        let bytes = serde_json::to_vec(&index)?;
        ensure!(
            self.bytes + bytes.len() as u64 <= self.options.disk_budget_bytes,
            "Diagnostic index exceeds disk budget"
        );
        crate::helpers::write_file_atomically(&self.directory.join("index.json"), &bytes)?;
        self.finalized = true;
        self.output = None;
        Ok(index)
    }

    pub fn flush(&mut self) -> Result<()> {
        let result = self
            .output
            .as_mut()
            .map(|output| output.flush())
            .transpose();
        self.last_flush = Instant::now();
        if result.is_err() {
            self.failed = true;
        }
        result?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadSummary {
    pub records: u64,
    pub bytes: u64,
    pub interrupted_tail: bool,
}

pub struct JournalReader {
    pub(crate) directory: PathBuf,
    pub metadata: OperationMetadata,
    pub(crate) segments: Vec<PathBuf>,
}

impl JournalReader {
    pub fn open(output: &Path, operation: &OperationId) -> Result<Self> {
        let root = output.canonicalize()?;
        checked_directory(&root.join("_diagnostics"), &root)?;
        let directory = checked_directory(&archive_directory(&root, operation), &root)?;
        let metadata: OperationMetadata = read_json(
            &directory.join("operation.json"),
            &directory,
            MAX_RECORD_BYTES,
        )?;
        ensure!(
            metadata.version == JOURNAL_VERSION && metadata.operation_id == *operation,
            "Invalid diagnostic operation metadata"
        );
        let mut segments = Vec::new();
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().context("Invalid diagnostic filename")?;
            if name.starts_with("segment-") {
                let number = name
                    .strip_prefix("segment-")
                    .and_then(|name| name.strip_suffix(".jsonl"))
                    .filter(|number| {
                        number.len() == 6 && number.bytes().all(|byte| byte.is_ascii_digit())
                    })
                    .context("Invalid diagnostic segment name")?;
                ensure!(
                    segments.len() < MAX_SEGMENTS,
                    "Diagnostic segment limit exceeded"
                );
                segments.push((number.parse::<usize>()?, entry.path()));
            }
        }
        segments.sort_by_key(|(number, _)| *number);
        for (expected, (actual, _)) in segments.iter().enumerate() {
            ensure!(expected == *actual, "Missing diagnostic segment");
        }
        Ok(Self {
            directory,
            metadata,
            segments: segments.into_iter().map(|(_, path)| path).collect(),
        })
    }

    pub fn archive_state(&self) -> ArchiveState {
        self.archive_state_and_missing().0
    }

    pub(crate) fn archive_state_and_missing(&self) -> (ArchiveState, Option<u64>) {
        match fs::symlink_metadata(self.directory.join("cancellation.json")) {
            Ok(_) => {
                return self
                    .visit(|_| {})
                    .map(|_| (ArchiveState::Partial, None))
                    .unwrap_or((ArchiveState::LoadingError, None));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return (ArchiveState::LoadingError, None),
        }
        let path = self.directory.join("index.json");
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (ArchiveState::Partial, None)
            }
            Err(_) => (ArchiveState::LoadingError, None),
            Ok(_) => self
                .verified_index()
                .map(|index| (index.state, index.missing_records))
                .unwrap_or((ArchiveState::LoadingError, None)),
        }
    }

    pub fn verified_index(&self) -> Result<ArchiveIndex> {
        let index: ArchiveIndex = read_json(
            &self.directory.join("index.json"),
            &self.directory,
            4 * 1024 * 1024,
        )?;
        ensure!(
            index.version == JOURNAL_VERSION && index.operation_id == self.metadata.operation_id,
            "Diagnostic index identity mismatch"
        );
        ensure!(
            matches!(index.state, ArchiveState::Complete | ArchiveState::Partial),
            "Invalid stored archive state"
        );
        ensure!(
            index.segments.len() == self.segments.len(),
            "Diagnostic index segment count mismatch"
        );
        for (number, (path, expected)) in self.segments.iter().zip(&index.segments).enumerate() {
            ensure!(
                segment_info(path, &self.directory, number)? == *expected,
                "Diagnostic segment checksum mismatch"
            );
        }
        let header_bytes = checked_file(&self.directory.join("operation.json"), &self.directory)?
            .metadata()?
            .len();
        let segment_bytes = index.segments.iter().try_fold(0_u64, |total, segment| {
            total
                .checked_add(segment.bytes)
                .ok_or_else(|| anyhow!("Diagnostic index byte count overflow"))
        })?;
        ensure!(
            header_bytes.checked_add(segment_bytes) == Some(index.bytes),
            "Diagnostic index byte count mismatch"
        );
        let mut terminal = None;
        let summary = self.visit(|record| {
            terminal = Some((record.event_name, record.outcome));
        })?;
        ensure!(
            summary.records == index.records
                && (index.state == ArchiveState::Partial || !summary.interrupted_tail),
            "Diagnostic index record count mismatch"
        );
        if index.state == ArchiveState::Complete {
            ensure!(
                index.missing_records == Some(0)
                    && terminal == Some((DiagnosticEvent::Finished, Some(index.outcome))),
                "Complete archive lacks a verified terminal outcome"
            );
        }
        Ok(index)
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn visit(&self, mut receive: impl FnMut(DiagnosticRecordV1)) -> Result<ReadSummary> {
        let mut summary = ReadSummary::default();
        let mut pending = Vec::new();
        for (index, path) in self.segments.iter().enumerate() {
            let mut input =
                BufReader::with_capacity(BUFFER_BYTES, checked_file(path, &self.directory)?);
            loop {
                pending.clear();
                let count = (&mut input)
                    .take(MAX_RECORD_BYTES as u64 + 1)
                    .read_until(b'\n', &mut pending)?;
                if count == 0 {
                    break;
                }
                ensure!(
                    count <= MAX_RECORD_BYTES,
                    "Diagnostic record exceeds size limit"
                );
                if pending.last() != Some(&b'\n') {
                    ensure!(
                        index + 1 == self.segments.len(),
                        "Interrupted non-terminal diagnostic segment"
                    );
                    summary.interrupted_tail = true;
                    break;
                }
                let record: DiagnosticRecordV1 = serde_json::from_slice(&pending)?;
                record.validate()?;
                ensure!(
                    record.operation_id == self.metadata.operation_id
                        && record.operation_kind == self.metadata.operation_kind
                        && record.capture_run_id == self.metadata.capture_run_id
                        && record.sequence == summary.records,
                    "Diagnostic identity or sequence mismatch"
                );
                summary.records += 1;
                summary.bytes += count as u64;
                receive(record);
            }
        }
        let cancellation = self.directory.join("cancellation.json");
        match fs::symlink_metadata(&cancellation) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => {
                let record: DiagnosticRecordV1 =
                    read_json(&cancellation, &self.directory, MAX_RECORD_BYTES)?;
                record.validate()?;
                ensure!(
                    record.operation_id == self.metadata.operation_id
                        && record.operation_kind == OperationKind::Deployment
                        && record.event_name == DiagnosticEvent::Cancelled
                        && record.sequence == summary.records,
                    "Invalid deployment cancellation record"
                );
                summary.records += 1;
                receive(record);
            }
        }
        Ok(summary)
    }
}

#[cfg(feature = "desktop")]
pub(crate) fn record_worker_cancellation(output: &Path, operation: &OperationId) -> Result<()> {
    use super::record::{Field, FieldValue, SafeValue, Severity};
    if super::health::archives_disabled(|name| std::env::var(name).ok()).unwrap_or(true) {
        return Ok(());
    }
    let directory = archive_directory(output, operation);
    if !directory.exists() {
        let mut metadata = OperationMetadata::new(OperationKind::Deployment, None);
        metadata.operation_id = operation.clone();
        let writer = JournalWriter::create(output, metadata, JournalOptions::default())?;
        drop(writer);
    }
    let reader = JournalReader::open(output, operation)?;
    ensure!(
        reader.metadata.operation_kind == OperationKind::Deployment,
        "Only deployment workers have remote cancellation state"
    );
    let path = reader.directory.join("cancellation.json");
    if fs::symlink_metadata(&path).is_ok() {
        reader.visit(|_| {})?;
        return Ok(());
    }
    let mut published = false;
    let summary = reader.visit(|record| {
        published |= record.event_name == DiagnosticEvent::Published;
    })?;
    let mut record = DiagnosticRecordV1::new(
        operation.clone(),
        OperationKind::Deployment,
        Stage::CompletionCleanup,
        DiagnosticEvent::Cancelled,
    );
    record.sequence = summary.records;
    record.elapsed_ms = chrono::Utc::now()
        .timestamp_millis()
        .saturating_sub(reader.metadata.timestamp_utc_ms)
        .max(0) as u64;
    record.severity = Severity::Warn;
    record.outcome = Some(if published {
        Outcome::Cancelled
    } else {
        Outcome::CancelledRemoteUnknown
    });
    record.fields.insert(
        Field::State,
        FieldValue::Kind(if published {
            SafeValue::Complete
        } else {
            SafeValue::Unknown
        }),
    );
    record.validate()?;
    crate::helpers::write_file_atomically(&path, &serde_json::to_vec(&record)?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::record::{DiagnosticEvent, Severity, Stage};
    use super::*;

    fn write_records(
        root: &Path,
        options: JournalOptions,
        count: usize,
    ) -> (JournalWriter, OperationMetadata) {
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let mut writer = JournalWriter::create(root, metadata.clone(), options).unwrap();
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Decision,
        );
        for _ in 0..count {
            writer.write(&record).unwrap();
        }
        writer.flush().unwrap();
        (writer, metadata)
    }

    #[test]
    fn forced_flush_failure_retains_prefix_and_refuses_completion() {
        let root = tempfile::tempdir().unwrap();
        let (mut writer, metadata) = write_records(root.path(), JournalOptions::default(), 1);
        writer.output = Some(BufWriter::with_capacity(
            BUFFER_BYTES,
            File::open(writer.directory().join("segment-000000.jsonl")).unwrap(),
        ));
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::Finish,
            DiagnosticEvent::Decision,
        );
        writer.write(&record).unwrap();
        assert!(writer.flush().is_err());
        assert!(writer.finalize(Outcome::Success).is_err());
        let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
        assert_eq!(reader.archive_state(), ArchiveState::Partial);
        assert_eq!(reader.visit(|_| {}).unwrap().records, 1);
    }

    #[test]
    fn oversized_records_and_tampered_indexes_never_look_complete() {
        let root = tempfile::tempdir().unwrap();
        let (mut writer, metadata) = write_records(root.path(), JournalOptions::default(), 1);
        let mut record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::Finish,
            DiagnosticEvent::Finished,
        );
        record.message = "private\u{1b}<script>\u{1f600}".repeat(MAX_RECORD_BYTES);
        assert!(writer.write(&record).is_err());
        assert!(writer.finalize(Outcome::Success).is_err());
        writer.flush().unwrap();
        assert_eq!(
            JournalReader::open(root.path(), &metadata.operation_id)
                .unwrap()
                .archive_state(),
            ArchiveState::Partial
        );
        let (mut writer, metadata) = write_records(root.path(), JournalOptions::default(), 1);
        writer.finalize(Outcome::Success).unwrap();
        let path = writer.directory().join("index.json");
        let mut index: ArchiveIndex = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        index.bytes += 1;
        fs::write(path, serde_json::to_vec(&index).unwrap()).unwrap();
        assert_eq!(
            JournalReader::open(root.path(), &metadata.operation_id)
                .unwrap()
                .archive_state(),
            ArchiveState::LoadingError
        );
    }

    #[test]
    fn abrupt_process_exit_retains_flushed_records_and_never_infers_success() {
        const FIXTURE: &str = "REGRESSIFY_JOURNAL_ABORT_FIXTURE";
        if let Some(root) = std::env::var_os(FIXTURE) {
            let (mut writer, _) = write_records(Path::new(&root), JournalOptions::default(), 500);
            writer.flush().unwrap();
            std::process::exit(23);
        }
        let root = tempfile::tempdir().unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "diagnostics::journal::tests::abrupt_process_exit_retains_flushed_records_and_never_infers_success"])
            .env(FIXTURE, root.path()).output().unwrap();
        assert_eq!(result.status.code(), Some(23));
        let operation = fs::read_dir(root.path().join("_diagnostics"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let operation =
            OperationId::try_from(operation.file_name().into_string().unwrap()).unwrap();
        let reader = JournalReader::open(root.path(), &operation).unwrap();
        assert_eq!(reader.archive_state(), ArchiveState::Partial);
        assert_eq!(reader.visit(|_| {}).unwrap().records, 500);
        let path = archive_directory(root.path(), &operation).join("segment-000000.jsonl");
        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"{\"schemaVersion\":")
            .unwrap();
        let summary = reader.visit(|_| {}).unwrap();
        assert!(summary.interrupted_tail);
        assert_eq!(summary.records, 500);
        assert_eq!(reader.archive_state(), ArchiveState::Partial);
    }

    #[test]
    fn bootstrap_handoff_retains_source_on_destination_failure() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let blocked = destination.path().join("blocked");
        fs::write(&blocked, b"not a directory").unwrap();
        let (writer, metadata) = write_records(source.path(), JournalOptions::default(), 3);
        let source_directory = writer.directory().to_owned();
        let original = fs::read(source_directory.join("segment-000000.jsonl")).unwrap();
        let mut notices = Vec::new();
        let mut writer = writer.handoff_bootstrap(&blocked, |notice| notices.push(notice));
        assert_eq!(notices, vec![HandoffNotice::RetainedSource]);
        assert_eq!(writer.directory(), source_directory);
        assert_eq!(
            fs::read(source_directory.join("segment-000000.jsonl")).unwrap(),
            original
        );
        assert!(!source_directory.join("handoff.json").exists());
        writer.finalize(Outcome::Failure).unwrap();
        assert_eq!(
            JournalReader::open(source.path(), &metadata.operation_id)
                .unwrap()
                .archive_state(),
            ArchiveState::Complete
        );
    }

    #[test]
    fn bootstrap_handoff_marks_source_consumed_only_after_verified_append_open() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let (writer, metadata) = write_records(source.path(), JournalOptions::default(), 3);
        let source_directory = writer.directory().to_owned();
        let original = fs::read(source_directory.join("segment-000000.jsonl")).unwrap();
        let mut notices = Vec::new();
        let mut writer =
            writer.handoff_bootstrap(destination.path(), |notice| notices.push(notice));
        assert_eq!(notices, vec![HandoffNotice::Attached]);
        assert!(source_directory.join("handoff.json").is_file());
        assert_eq!(
            fs::read(source_directory.join("segment-000000.jsonl")).unwrap(),
            original
        );
        assert_eq!(writer.metadata.operation_id, metadata.operation_id);
        writer = writer.handoff_bootstrap(destination.path(), |_| {
            panic!("Repeated handoff must not copy or notify")
        });
        writer.finalize(Outcome::Success).unwrap();
        let reader = JournalReader::open(destination.path(), &metadata.operation_id).unwrap();
        assert_eq!(reader.archive_state(), ArchiveState::Complete);
        assert_eq!(reader.visit(|_| {}).unwrap().records, 4);
        assert_eq!(
            JournalReader::open(source.path(), &metadata.operation_id)
                .unwrap()
                .visit(|_| {})
                .unwrap()
                .records,
            3
        );
    }

    #[test]
    fn legacy_report_without_archive_is_explicit_not_an_error() {
        let root = tempfile::tempdir().unwrap();
        let operation = OperationId::new();
        assert_eq!(
            inspect_archive(root.path(), Some(&operation), false),
            ArchiveState::LegacyUnavailable
        );
        assert_eq!(
            inspect_archive(root.path(), None, false),
            ArchiveState::LegacyUnavailable
        );
        fs::create_dir_all(archive_directory(root.path(), &operation)).unwrap();
        fs::write(
            archive_directory(root.path(), &operation).join("operation.json"),
            b"corrupt",
        )
        .unwrap();
        assert_eq!(
            inspect_archive(root.path(), Some(&operation), false),
            ArchiveState::LegacyUnavailable
        );
    }

    #[test]
    fn archive_missing_when_expected_is_distinct_from_legacy() {
        let root = tempfile::tempdir().unwrap();
        let operation = OperationId::new();
        assert_eq!(
            inspect_archive(root.path(), Some(&operation), true),
            ArchiveState::Missing
        );
        fs::create_dir_all(archive_directory(root.path(), &operation)).unwrap();
        assert_eq!(
            inspect_archive(root.path(), Some(&operation), true),
            ArchiveState::Missing
        );
        assert_ne!(
            inspect_archive(root.path(), Some(&operation), false),
            ArchiveState::Missing
        );
        assert_eq!(
            inspect_archive(root.path(), None, true),
            ArchiveState::Missing
        );
    }

    #[test]
    fn partial_marker_is_distinct_from_legacy_and_empty_storage() {
        let root = tempfile::tempdir().unwrap();
        let operation = OperationId::new();
        let directory = archive_directory(root.path(), &operation);
        fs::create_dir_all(&directory).unwrap();
        let marker = ArchiveIndex {
            version: JOURNAL_VERSION,
            operation_id: operation.clone(),
            state: ArchiveState::Partial,
            records: 0,
            bytes: 0,
            segments: Vec::new(),
            outcome: Outcome::Unknown,
            missing_records: None,
        };
        fs::write(
            directory.join("index.json"),
            serde_json::to_vec(&marker).unwrap(),
        )
        .unwrap();
        assert_eq!(
            inspect_archive(root.path(), Some(&operation), true),
            ArchiveState::Partial
        );
        let (writer, metadata) = write_records(root.path(), JournalOptions::default(), 3);
        drop(writer);
        assert_eq!(
            inspect_archive(root.path(), Some(&metadata.operation_id), true),
            ArchiveState::Partial
        );
    }

    #[test]
    fn loading_error_when_segment_corrupt_despite_finalized_index() {
        let root = tempfile::tempdir().unwrap();
        let (mut writer, metadata) = write_records(root.path(), JournalOptions::default(), 3);
        writer.finalize(Outcome::Success).unwrap();
        assert_eq!(
            inspect_archive(root.path(), Some(&metadata.operation_id), true),
            ArchiveState::Complete
        );
        let segment = writer.directory().join("segment-000000.jsonl");
        let mut contents = fs::read(&segment).unwrap();
        contents[0] = b'!';
        fs::write(&segment, &contents).unwrap();
        assert_eq!(
            inspect_archive(root.path(), Some(&metadata.operation_id), true),
            ArchiveState::LoadingError
        );
        fs::remove_file(&segment).unwrap();
        assert_eq!(
            inspect_archive(root.path(), Some(&metadata.operation_id), true),
            ArchiveState::LoadingError
        );
    }

    #[test]
    fn only_complete_archives_allow_complete_exports() {
        for state in [
            ArchiveState::Complete,
            ArchiveState::Partial,
            ArchiveState::Missing,
            ArchiveState::LegacyUnavailable,
            ArchiveState::LoadingError,
        ] {
            assert_eq!(state.can_export(), state == ArchiveState::Complete);
            assert!(!state.label().is_empty());
        }
    }

    #[test]
    fn operation_kind_linkage_round_trips_both_kinds() {
        for kind in [OperationKind::Capture, OperationKind::Deployment] {
            let run = (kind == OperationKind::Capture)
                .then(|| CaptureRunId::try_from("run-one".to_owned()).unwrap());
            let metadata = OperationMetadata::new(kind, run);
            let record = DiagnosticRecordV1::new(
                metadata.operation_id.clone(),
                kind,
                Stage::QueueSetup,
                DiagnosticEvent::Started,
            );
            let records = exercise_journal_round_trip(metadata, vec![record]);
            assert_eq!(
                records[0].capture_run_id.is_some(),
                kind == OperationKind::Capture
            );
        }
    }

    pub(crate) fn exercise_journal_round_trip(
        metadata: OperationMetadata,
        records: Vec<DiagnosticRecordV1>,
    ) -> Vec<DiagnosticRecordV1> {
        let root = tempfile::tempdir().unwrap();
        let mut writer =
            JournalWriter::create(root.path(), metadata.clone(), JournalOptions::default())
                .unwrap();
        for record in &records {
            writer.write(record).unwrap();
        }
        writer.finalize(Outcome::Success).unwrap();
        let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
        assert_eq!(reader.archive_state(), ArchiveState::Complete);
        let mut loaded = Vec::new();
        reader
            .visit(|record| {
                assert_eq!(record.sequence, loaded.len() as u64);
                assert_eq!(record.operation_kind, metadata.operation_kind);
                assert_eq!(record.capture_run_id, metadata.capture_run_id);
                loaded.push(record);
            })
            .unwrap();
        assert_eq!(loaded.len(), records.len() + 1);
        loaded
    }

    #[test]
    fn operation_kind_linkage_rejects_conflicting_run_identity() {
        let root = tempfile::tempdir().unwrap();
        let metadata = OperationMetadata::new(
            OperationKind::Capture,
            Some(CaptureRunId::try_from("run-one".to_owned()).unwrap()),
        );
        let mut writer =
            JournalWriter::create(root.path(), metadata.clone(), JournalOptions::default())
                .unwrap();
        let mut record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Started,
        );
        record.capture_run_id = Some(CaptureRunId::try_from("run-two".to_owned()).unwrap());
        assert!(writer.write(&record).is_err());
        assert_eq!(writer.record_count(), 0);
    }

    #[test]
    fn journal_writer_reconstructs_ten_thousand_records_across_segments() {
        let root = tempfile::tempdir().unwrap();
        let (writer, metadata) = write_records(root.path(), JournalOptions::default(), 10_001);
        assert!(writer.segment_count() > 1);
        assert!(writer.peak_buffer_bytes() <= BUFFER_BYTES + 2 * MAX_RECORD_BYTES);
        let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
        let mut expected = 0;
        let summary = reader
            .visit(|record| {
                assert_eq!(record.sequence, expected);
                expected += 1;
            })
            .unwrap();
        assert_eq!(summary.records, 10_001);
        assert!(!summary.interrupted_tail);
    }

    #[test]
    fn finalize_marks_interrupted_write_as_partial_not_complete() {
        let root = tempfile::tempdir().unwrap();
        let (writer, metadata) = write_records(root.path(), JournalOptions::default(), 500);
        drop(writer);
        let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
        assert_eq!(reader.archive_state(), ArchiveState::Partial);
        assert_eq!(reader.visit(|_| {}).unwrap().records, 500);
    }

    #[test]
    fn normal_finalization_verifies_every_record_and_terminal_outcome() {
        let root = tempfile::tempdir().unwrap();
        for (outcome, severity) in [
            (Outcome::Success, Severity::Info),
            (Outcome::Failure, Severity::Error),
            (Outcome::Cancelled, Severity::Warn),
            (Outcome::CancelledRemoteUnknown, Severity::Warn),
            (Outcome::Mismatch, Severity::Warn),
            (Outcome::Unknown, Severity::Warn),
        ] {
            let (mut writer, metadata) = write_records(root.path(), JournalOptions::default(), 999);
            let index = writer.finalize(outcome).unwrap();
            assert_eq!(index.records, 1000);
            assert!(writer.finalize(outcome).is_err());
            let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
            assert_eq!(reader.archive_state(), ArchiveState::Complete);
            assert_eq!(reader.verified_index().unwrap(), index);
            let mut terminal = None;
            let summary = reader.visit(|record| terminal = Some(record)).unwrap();
            assert_eq!(summary.records, 1000);
            let terminal = terminal.unwrap();
            assert_eq!(terminal.event_name, DiagnosticEvent::Finished);
            assert_eq!(terminal.outcome, Some(outcome));
            assert_eq!(terminal.severity, severity);
        }
    }

    #[test]
    fn failed_index_write_cannot_advertise_a_complete_archive() {
        let root = tempfile::tempdir().unwrap();
        let (mut writer, metadata) = write_records(root.path(), JournalOptions::default(), 5);
        fs::create_dir(writer.directory().join("index.json")).unwrap();
        assert!(writer.finalize(Outcome::Success).is_err());
        assert!(writer.finalize(Outcome::Success).is_err());
        let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
        assert_ne!(reader.archive_state(), ArchiveState::Complete);
        assert_eq!(reader.visit(|_| {}).unwrap().records, 6);
    }

    #[test]
    fn chunk_size_flag_and_env_var_override_default() {
        use clap::Parser;
        let root = tempfile::tempdir().unwrap();
        let defaults = JournalOptions::resolve(None, None, |_| None).unwrap();
        let (default_writer, _) = write_records(root.path(), defaults, 10_001);
        let cli = crate::cli::Cli::try_parse_from([
            "regressify",
            "--diagnostic-chunk-size-bytes",
            "65536",
            "test",
        ])
        .unwrap();
        let overridden = JournalOptions::resolve(
            cli.diagnostic_chunk_size_bytes,
            cli.diagnostic_disk_budget_bytes,
            |name| (name == CHUNK_ENV).then(|| "131072".into()),
        )
        .unwrap();
        let (overridden_writer, _) = write_records(root.path(), overridden, 10_001);
        assert!(overridden_writer.segment_count() > default_writer.segment_count());
        assert_eq!(overridden.chunk_bytes, 65536);
        assert_eq!(
            JournalOptions::resolve(None, None, |name| (name == CHUNK_ENV)
                .then(|| "131072".into()))
            .unwrap()
            .chunk_bytes,
            131072
        );
        assert!(JournalOptions::resolve(None, None, |_| Some("invalid".into())).is_err());
    }

    #[test]
    fn budgets_fail_without_overwriting_existing_history_or_escaping_paths() {
        let root = tempfile::tempdir().unwrap();
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let options = JournalOptions {
            chunk_bytes: 4096,
            disk_budget_bytes: 4096,
        };
        let mut writer = JournalWriter::create(root.path(), metadata.clone(), options).unwrap();
        assert!(JournalWriter::create(root.path(), metadata.clone(), options).is_err());
        let record = DiagnosticRecordV1::new(
            metadata.operation_id.clone(),
            OperationKind::Capture,
            Stage::Finish,
            DiagnosticEvent::Decision,
        );
        let mut written = 0;
        while writer.write(&record).is_ok() {
            written += 1;
        }
        writer.flush().unwrap();
        assert!(written > 0);
        assert!(writer.byte_count() <= 4096);
        assert!(writer.write(&record).is_err());
        let reader = JournalReader::open(root.path(), &metadata.operation_id).unwrap();
        assert_eq!(reader.visit(|_| {}).unwrap().records, written);
        let other = DiagnosticRecordV1::new(
            OperationId::new(),
            OperationKind::Capture,
            Stage::Finish,
            DiagnosticEvent::Decision,
        );
        let (mut writer, _) = write_records(root.path(), JournalOptions::default(), 0);
        assert!(writer.write(&other).is_err());
    }

    #[test]
    fn journal_rejects_invalid_numeric_limits_and_segment_substitution() {
        for options in [
            JournalOptions {
                chunk_bytes: 0,
                disk_budget_bytes: 4096,
            },
            JournalOptions {
                chunk_bytes: 8192,
                disk_budget_bytes: 4096,
            },
            JournalOptions {
                chunk_bytes: usize::MAX,
                disk_budget_bytes: u64::MAX,
            },
        ] {
            assert!(options.validate().is_err());
        }
        let root = tempfile::tempdir().unwrap();
        let (writer, metadata) = write_records(root.path(), JournalOptions::default(), 1);
        let directory = writer.directory().to_owned();
        drop(writer);
        fs::rename(
            directory.join("segment-000000.jsonl"),
            directory.join("segment-000001.jsonl"),
        )
        .unwrap();
        assert!(JournalReader::open(root.path(), &metadata.operation_id).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn journal_rejects_symlinked_storage_and_segments() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("_diagnostics")).unwrap();
        assert!(
            JournalWriter::create(
                root.path(),
                OperationMetadata::new(OperationKind::Capture, None),
                JournalOptions::default()
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }
}
