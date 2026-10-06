use std::{collections::BTreeMap, fmt};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: u16 = 1;
const MAX_JSON_INTEGER: u64 = (1 << 53) - 1;

macro_rules! uuid_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl TryFrom<String> for $name {
            type Error = &'static str;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                let parsed = uuid::Uuid::parse_str(&value).map_err(|_| "Invalid diagnostic ID")?;
                if parsed.is_nil() || parsed.to_string() != value {
                    return Err("Diagnostic IDs must be non-nil canonical UUIDs");
                }
                Ok(Self(value))
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

uuid_identity!(OperationId);
uuid_identity!(ExecutionId);
uuid_identity!(SpanId);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CaptureRunId(String);

impl CaptureRunId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CaptureRunId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        crate::cli::args::run_id_parser(&value).map(Self)
    }
}

impl From<CaptureRunId> for String {
    fn from(value: CaptureRunId) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SafeIdentity(String);

impl SafeIdentity {
    pub fn fingerprint(value: impl AsRef<[u8]>) -> Self {
        Self(format!("sha256:{:x}", Sha256::digest(value.as_ref())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SafeIdentity {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() != 71
            || !value.starts_with("sha256:")
            || !value[7..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("Expected an opaque SHA-256 identity");
        }
        Ok(Self(value))
    }
}

impl From<SafeIdentity> for String {
    fn from(value: SafeIdentity) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Capture,
    Deployment,
    Command,
    Application,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureRole {
    Reference,
    Test,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    clap::ValueEnum,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::str::FromStr for Severity {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err("Log level must be trace, debug, info, warn, or error"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    QueueSetup,
    Navigation,
    StateHooks,
    ActionsFrames,
    ScreenshotSettling,
    CaptureArtifacts,
    Finish,
    Preflight,
    SnapshotStaging,
    PackagingOptimization,
    ApiUploads,
    RetryPolling,
    CompletionCleanup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Success,
    Failure,
    Cancelled,
    CancelledRemoteUnknown,
    Mismatch,
    Unknown,
}

impl Outcome {
    pub fn severity(self) -> Severity {
        match self {
            Self::Success => Severity::Info,
            Self::Failure => Severity::Error,
            Self::Cancelled | Self::CancelledRemoteUnknown | Self::Mismatch | Self::Unknown => {
                Severity::Warn
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticEvent {
    Queued,
    Started,
    StageStarted,
    StageFinished,
    Decision,
    Waiting,
    Retry,
    Heartbeat,
    Captured,
    Compared,
    Published,
    Warning,
    Cancelled,
    Finished,
}

impl DiagnosticEvent {
    pub fn message(self) -> &'static str {
        match self {
            Self::Queued => "Execution queued",
            Self::Started => "Execution started",
            Self::StageStarted => "Stage started",
            Self::StageFinished => "Stage finished",
            Self::Decision => "Execution decision",
            Self::Waiting => "Waiting for readiness",
            Self::Retry => "Retry scheduled",
            Self::Heartbeat => "Operation is still working",
            Self::Captured => "Capture completed",
            Self::Compared => "Comparison completed",
            Self::Published => "Publication confirmed",
            Self::Warning => "Diagnostics are incomplete",
            Self::Cancelled => "Execution cancelled",
            Self::Finished => "Execution finished",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    PublicationUrl,
    Command,
    ActionIndex,
    ActionType,
    Target,
    Destination,
    Artifact,
    Capture,
    TimeoutMs,
    ElementTimeoutMs,
    ActionabilityTimeoutMs,
    PollIntervalMs,
    ScenarioTimeoutMs,
    RetryLimit,
    RetryAttempt,
    BackoffMs,
    QueueMs,
    ElapsedMs,
    CompletedAnimations,
    FrozenAnimations,
    Width,
    Height,
    X,
    Y,
    Bytes,
    BytesBefore,
    BytesAfter,
    BudgetBytes,
    FileCount,
    SkippedFiles,
    BatchCount,
    SelectedSuites,
    StatusCode,
    OsErrorCode,
    RequestId,
    Provider,
    Endpoint,
    Condition,
    Reason,
    State,
    CredentialPresent,
    Compression,
    DeadlineMs,
    RedirectCount,
    FrameDepth,
    TabCount,
    Infrastructure,
    CleanupFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SafeValue {
    Click,
    Hover,
    Check,
    Uncheck,
    Focus,
    Input,
    Press,
    Select,
    Scroll,
    Navigate,
    Save,
    Hide,
    Remove,
    Wait,
    Persist,
    Restore,
    SwitchTab,
    Hook,
    Before,
    Ready,
    Exists,
    Visible,
    Stable,
    Enabled,
    Unobscured,
    Fonts,
    Images,
    LazyLoading,
    Animation,
    Caret,
    Cached,
    Download,
    Browser,
    Page,
    Handler,
    Azure,
    Netlify,
    Cloudflare,
    Manifest,
    Package,
    Upload,
    Poll,
    Publication,
    Preflight,
    Transient,
    RateLimited,
    Timeout,
    Pending,
    Complete,
    Failed,
    Cancelled,
    Init,
    Install,
    Mcp,
    Version,
    Test,
    Reference,
    Compare,
    Approve,
    Snapshot,
    Optimize,
    Empty,
    BelowMinimum,
    Unchanged,
    Cli,
    Desktop,
    Worker,
    Shell,
    Window,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum FieldValue {
    PublicUrl(PublicUrl),
    Number(u64),
    Coordinate(i32),
    Boolean(bool),
    Identity(SafeIdentity),
    Kind(SafeValue),
}

impl Field {
    fn accepts(self, value: &FieldValue) -> bool {
        match self {
            Self::PublicationUrl => matches!(value, FieldValue::PublicUrl(_)),
            Self::Target | Self::Destination | Self::Artifact | Self::Capture | Self::RequestId => {
                matches!(value, FieldValue::Identity(_))
            }
            Self::Command
            | Self::ActionType
            | Self::Provider
            | Self::Endpoint
            | Self::Condition
            | Self::Reason
            | Self::State => {
                matches!(value, FieldValue::Kind(_))
            }
            Self::CredentialPresent
            | Self::Compression
            | Self::Infrastructure
            | Self::CleanupFailed => {
                matches!(value, FieldValue::Boolean(_))
            }
            Self::X | Self::Y => matches!(value, FieldValue::Coordinate(_)),
            _ => matches!(value, FieldValue::Number(number) if *number <= MAX_JSON_INTEGER),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PublicUrl(String);

impl PublicUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PublicUrl {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let url = reqwest::Url::parse(&value).map_err(|_| "Invalid public URL")?;
        if value.len() > 2048
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !matches!(url.path(), "" | "/")
            || value.chars().any(char::is_control)
        {
            return Err("Unsafe public URL");
        }
        Ok(Self(url.to_string()))
    }
}

impl From<PublicUrl> for String {
    fn from(value: PublicUrl) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCategory {
    Timeout,
    NotFound,
    Permission,
    Navigation,
    Actionability,
    Cancelled,
    InvalidData,
    Storage,
    Provider,
    Unknown,
}

impl ErrorCategory {
    fn classify(message: &str) -> Self {
        let message = message.to_ascii_lowercase();
        if message.contains("timed out")
            || message.contains("timeout")
            || message.contains("navigation exceeded")
        {
            Self::Timeout
        } else if message.contains("cancel") {
            Self::Cancelled
        } else if message.contains("not found") || message.contains("no such file") {
            Self::NotFound
        } else if message.contains("permission") || message.contains("access is denied") {
            Self::Permission
        } else if message.contains("navigation") {
            Self::Navigation
        } else if [
            "actionability",
            "obscured",
            "not visible",
            "disabled",
            "not stable",
        ]
        .iter()
        .any(|word| message.contains(word))
        {
            Self::Actionability
        } else if message.contains("invalid") || message.contains("parse") {
            Self::InvalidData
        } else if message.contains("write") || message.contains("disk") || message.contains("flush")
        {
            Self::Storage
        } else {
            Self::Unknown
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitizedError {
    pub category: ErrorCategory,
    pub causes: Vec<ErrorCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    local_detail: Option<String>,
}

impl SanitizedError {
    pub fn summary(&self) -> String {
        self.local_detail
            .clone()
            .unwrap_or_else(|| format!("{:?} failure (causes: {:?})", self.category, self.causes))
    }

    pub fn capture(error: &anyhow::Error) -> Self {
        let causes: Vec<_> = error
            .chain()
            .take(8)
            .map(|cause| ErrorCategory::classify(&cause.to_string()))
            .collect();
        Self {
            category: causes.first().copied().unwrap_or(ErrorCategory::Unknown),
            causes,
            local_detail: None,
        }
    }

    pub fn deployment(message: &str, authorization: &reqwest::header::HeaderValue) -> Self {
        Self {
            category: ErrorCategory::Provider,
            causes: vec![ErrorCategory::Provider],
            local_detail: Some(super::redaction::sanitize(message, authorization)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportIdentity {
    pub name: SafeIdentity,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionIdentity {
    pub execution_id: ExecutionId,
    pub suite_id: SafeIdentity,
    pub scenario_id: SafeIdentity,
    pub viewport: ViewportIdentity,
    pub capture_role: CaptureRole,
    pub attempt: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRecordV1 {
    #[serde(deserialize_with = "version_one")]
    pub schema_version: u16,
    pub operation_id: OperationId,
    pub operation_kind: OperationKind,
    pub capture_run_id: Option<CaptureRunId>,
    pub execution: Option<ExecutionIdentity>,
    pub sequence: u64,
    pub timestamp_utc_ms: i64,
    pub elapsed_ms: u64,
    pub severity: Severity,
    pub stage: Stage,
    pub event_name: DiagnosticEvent,
    pub message: String,
    pub fields: BTreeMap<Field, FieldValue>,
    pub span_id: SpanId,
    pub parent_span: Option<SpanId>,
    pub duration_ms: Option<u64>,
    pub outcome: Option<Outcome>,
    pub error_cause: Option<SanitizedError>,
}

fn version_one<'de, Deserializer: serde::Deserializer<'de>>(
    deserializer: Deserializer,
) -> Result<u16, Deserializer::Error> {
    let version = u16::deserialize(deserializer)?;
    if version != SCHEMA_VERSION {
        return Err(serde::de::Error::custom(
            "Unsupported diagnostic schema version",
        ));
    }
    Ok(version)
}

impl DiagnosticRecordV1 {
    pub fn new(
        operation_id: OperationId,
        operation_kind: OperationKind,
        stage: Stage,
        event: DiagnosticEvent,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            operation_id,
            operation_kind,
            capture_run_id: None,
            execution: None,
            sequence: 0,
            timestamp_utc_ms: chrono::Utc::now().timestamp_millis(),
            elapsed_ms: 0,
            severity: Severity::Info,
            stage,
            event_name: event,
            message: event.message().into(),
            fields: BTreeMap::new(),
            span_id: SpanId::new(),
            parent_span: None,
            duration_ms: None,
            outcome: None,
            error_cause: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == SCHEMA_VERSION,
            "Unsupported diagnostic schema version"
        );
        ensure!(
            self.message == self.event_name.message(),
            "Diagnostic messages must use safe templates"
        );
        ensure!(
            self.sequence <= MAX_JSON_INTEGER
                && self.elapsed_ms <= MAX_JSON_INTEGER
                && self
                    .duration_ms
                    .is_none_or(|duration| duration <= MAX_JSON_INTEGER),
            "Diagnostic integer exceeds JSON precision"
        );
        ensure!(
            (-8_640_000_000_000_000..=8_640_000_000_000_000).contains(&self.timestamp_utc_ms),
            "Diagnostic timestamp exceeds the supported date range"
        );
        for (field, value) in &self.fields {
            ensure!(
                *field != Field::PublicationUrl || self.operation_kind == OperationKind::Deployment,
                "Publication URLs are deployment-only"
            );
            ensure!(field.accepts(value), "Invalid diagnostic field type");
        }
        if let Some(execution) = &self.execution {
            ensure!(
                self.operation_kind == OperationKind::Capture,
                "Only captures can own execution identities"
            );
            ensure!(
                execution.attempt > 0
                    && execution.viewport.width > 0
                    && execution.viewport.height > 0,
                "Invalid execution attempt or viewport"
            );
        }
        if let Some(error) = &self.error_cause {
            ensure!(
                error.local_detail.as_ref().is_none_or(
                    |detail| detail.len() <= 8195 && !detail.chars().any(char::is_control)
                ),
                "Invalid provider diagnostic detail"
            );
            ensure!(error.causes.len() <= 8, "Too many diagnostic error causes");
            ensure!(
                self.operation_kind == OperationKind::Deployment || error.local_detail.is_none(),
                "Provider details cannot enter public capture records"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn publication_urls_reject_credentials_signed_values_and_capture_export() {
        for url in [
            "http://reports.example.test/",
            "https://user:secret@reports.example.test/",
            "https://reports.example.test/?sig=private",
            "https://reports.example.test/#secret",
            "https://reports.example.test/private-token",
        ] {
            assert!(PublicUrl::try_from(url.to_owned()).is_err());
        }
        let url = PublicUrl::try_from("https://reports.example.test/".to_owned()).unwrap();
        let mut record = DiagnosticRecordV1::new(
            OperationId::new(),
            OperationKind::Capture,
            Stage::Finish,
            DiagnosticEvent::Finished,
        );
        record
            .fields
            .insert(Field::PublicationUrl, FieldValue::PublicUrl(url));
        assert!(record.validate().is_err());
        record.operation_kind = OperationKind::Deployment;
        record.validate().unwrap();
    }

    use super::*;

    #[test]
    fn diagnostic_record_v1_round_trips_through_json() {
        let mut record = DiagnosticRecordV1::new(
            OperationId::new(),
            OperationKind::Capture,
            Stage::ActionsFrames,
            DiagnosticEvent::StageFinished,
        );
        record.capture_run_id = Some(CaptureRunId::try_from("20261001_120000".to_owned()).unwrap());
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
            attempt: 2,
        });
        record
            .fields
            .insert(Field::ActionIndex, FieldValue::Number(3));
        record
            .fields
            .insert(Field::ActionType, FieldValue::Kind(SafeValue::Click));
        record.fields.insert(
            Field::Target,
            FieldValue::Identity(SafeIdentity::fingerprint("[data-secret=private]")),
        );
        record.error_cause = Some(SanitizedError::capture(&anyhow::anyhow!(
            "Timed out on private selector"
        )));
        record.outcome = Some(Outcome::Failure);
        record.validate().unwrap();
        let encoded = serde_json::to_string(&record).unwrap();
        assert!(!encoded.contains("private"));
        let decoded: DiagnosticRecordV1 = serde_json::from_str(&encoded).unwrap();
        decoded.validate().unwrap();
        assert_eq!(record, decoded);
    }

    #[test]
    fn diagnostic_ids_reject_paths_reserved_names_and_noncanonical_values() {
        for value in ["../escape", "CON", "a/b", "", "nul", "C:\\secrets"] {
            assert!(CaptureRunId::try_from(value.to_owned()).is_err());
            assert!(OperationId::try_from(value.to_owned()).is_err());
        }
        assert!(OperationId::try_from(uuid::Uuid::nil().to_string()).is_err());
        assert!(SafeIdentity::try_from("https://user:secret@example.test".to_owned()).is_err());
        assert!(serde_json::from_str::<ExecutionId>("\"../escape\"").is_err());
    }

    #[test]
    fn schema_rejects_unknown_versions_fields_and_unreviewed_payloads() {
        let mut record = DiagnosticRecordV1::new(
            OperationId::new(),
            OperationKind::Capture,
            Stage::QueueSetup,
            DiagnosticEvent::Started,
        );
        for timestamp in [
            i64::MIN,
            -8_640_000_000_000_001,
            8_640_000_000_000_001,
            i64::MAX,
        ] {
            record.timestamp_utc_ms = timestamp;
            assert!(record.validate().is_err());
        }
        for timestamp in [-8_640_000_000_000_000, 8_640_000_000_000_000, 0] {
            record.timestamp_utc_ms = timestamp;
            record.validate().unwrap();
        }
        let mut value = serde_json::to_value(&record).unwrap();
        value["schemaVersion"] = 2.into();
        assert!(serde_json::from_value::<DiagnosticRecordV1>(value.clone()).is_err());
        value["schemaVersion"] = 1.into();
        value["fields"] = serde_json::json!({"password": {"type": "kind", "value": "input"}});
        assert!(serde_json::from_value::<DiagnosticRecordV1>(value).is_err());
        record
            .fields
            .insert(Field::Destination, FieldValue::Number(1));
        assert!(record.validate().is_err());
        record.fields.clear();
        record.message = "private payload".into();
        assert!(record.validate().is_err());
    }

    #[test]
    fn capture_errors_never_export_arbitrary_payloads_or_provider_details() {
        let error =
            anyhow::anyhow!("typed-input synthetic-secret <script>alert(1)</script>\u{1b}[31m")
                .context("Navigation timeout https://user:password@example.test/?sig=private");
        let safe = SanitizedError::capture(&error);
        assert_eq!(safe.category, ErrorCategory::Timeout);
        let encoded = serde_json::to_string(&safe).unwrap();
        for secret in ["synthetic-secret", "script", "password", "private", "31m"] {
            assert!(!encoded.contains(secret));
        }
        let mut record = DiagnosticRecordV1::new(
            OperationId::new(),
            OperationKind::Capture,
            Stage::Finish,
            DiagnosticEvent::Finished,
        );
        record.error_cause = Some(SanitizedError::deployment(
            "HTTP 403; token=synthetic-secret",
            &reqwest::header::HeaderValue::from_static("Bearer synthetic-secret"),
        ));
        assert!(record.validate().is_err());
        record.operation_kind = OperationKind::Deployment;
        record.validate().unwrap();
        assert!(
            !serde_json::to_string(&record)
                .unwrap()
                .contains("synthetic-secret")
        );
    }
}
