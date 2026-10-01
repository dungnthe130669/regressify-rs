use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use tracing::{
    Subscriber,
    field::{Field as TraceField, Visit},
    span::Attributes,
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

use super::record::{
    CaptureRunId, DiagnosticRecordV1, ExecutionId, ExecutionIdentity, OperationId, OperationKind,
    Severity,
};
use crate::progress;

pub trait ProgressAdapter: Send + Sync {
    fn on_diagnostic(&self, record: &DiagnosticRecordV1);
}

pub struct LiveProgressAdapter {
    jobs: Mutex<HashMap<ExecutionId, usize>>,
    receive: Arc<dyn Fn(progress::Event) + Send + Sync>,
}

impl Default for LiveProgressAdapter {
    fn default() -> Self {
        Self::new(progress::emit)
    }
}

impl LiveProgressAdapter {
    pub fn new(receive: impl Fn(progress::Event) + Send + Sync + 'static) -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            receive: Arc::new(receive),
        }
    }

    pub fn bind(&self, execution: ExecutionId, job: usize) -> Result<()> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| anyhow::anyhow!("Diagnostic job mapping unavailable"))?;
        ensure!(
            jobs.get(&execution).is_none_or(|existing| *existing == job),
            "Execution already belongs to another job"
        );
        jobs.insert(execution, job);
        Ok(())
    }

    pub fn unbind(&self, execution: &ExecutionId) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(execution);
        }
    }
}

impl ProgressAdapter for LiveProgressAdapter {
    fn on_diagnostic(&self, record: &DiagnosticRecordV1) {
        if record.validate().is_err() {
            return;
        }
        let job = record
            .execution
            .as_ref()
            .and_then(|execution| self.jobs.lock().ok()?.get(&execution.execution_id).copied());
        if let Some(id) = job {
            (self.receive)(progress::Event::Log {
                id,
                message: format_record(record),
            });
        }
    }
}

pub fn format_record(record: &DiagnosticRecordV1) -> String {
    let mut message = format!(
        "[{:?}] {:?}: {}",
        record.severity, record.stage, record.message
    );
    for (field, value) in &record.fields {
        use super::record::FieldValue;
        let value = match value {
            FieldValue::Number(value) => value.to_string(),
            FieldValue::Coordinate(value) => value.to_string(),
            FieldValue::Boolean(value) => value.to_string(),
            FieldValue::Identity(value) => value.as_str().to_owned(),
            FieldValue::Kind(value) => format!("{value:?}"),
        };
        message.push_str(&format!(" {field:?}={value}"));
    }
    if let Some(outcome) = record.outcome {
        message.push_str(&format!(" outcome={outcome:?}"));
    }
    if let Some(duration) = record.duration_ms {
        message.push_str(&format!(" duration={duration}ms"));
    }
    if let Some(error) = &record.error_cause {
        message.push_str(&format!(" cause={:?}", error.causes));
    }
    message
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticContext {
    pub operation_id: OperationId,
    pub operation_kind: OperationKind,
    pub capture_run_id: Option<CaptureRunId>,
    pub execution: Option<ExecutionIdentity>,
}

impl DiagnosticContext {
    pub fn span(&self) -> tracing::Span {
        let context = serde_json::to_string(self).expect("diagnostic context is serializable");
        tracing::info_span!(target: "regressify::diagnostics", "execution", diagnostic_context = context.as_str())
    }

    fn apply(&self, record: &mut DiagnosticRecordV1) {
        record.operation_id = self.operation_id.clone();
        record.operation_kind = self.operation_kind;
        record.capture_run_id = self.capture_run_id.clone();
        record.execution = self.execution.clone();
    }
}

pub struct DiagnosticLayer {
    adapter: Arc<dyn ProgressAdapter>,
    receive: Arc<dyn Fn(&DiagnosticRecordV1) + Send + Sync>,
}

impl DiagnosticLayer {
    pub fn new(
        adapter: Arc<dyn ProgressAdapter>,
        receive: impl Fn(&DiagnosticRecordV1) + Send + Sync + 'static,
    ) -> Self {
        Self {
            adapter,
            receive: Arc::new(receive),
        }
    }
}

#[derive(Default)]
struct SafeVisitor {
    record: Option<DiagnosticRecordV1>,
    context: Option<DiagnosticContext>,
}

impl Visit for SafeVisitor {
    fn record_str(&mut self, field: &TraceField, value: &str) {
        match field.name() {
            "diagnostic_record" => self.record = serde_json::from_str(value).ok(),
            "diagnostic_context" => self.context = serde_json::from_str(value).ok(),
            _ => {}
        }
    }

    fn record_debug(&mut self, _field: &TraceField, _value: &dyn std::fmt::Debug) {}
}

impl<Sub> Layer<Sub> for DiagnosticLayer
where
    Sub: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attributes: &Attributes<'_>,
        id: &tracing::Id,
        context: Context<'_, Sub>,
    ) {
        store_context(attributes, id, context);
    }

    fn on_event(&self, event: &tracing::Event<'_>, context: Context<'_, Sub>) {
        let Some(record) = event_record(event, context) else {
            return;
        };
        (self.receive)(&record);
        self.adapter.on_diagnostic(&record);
    }
}

pub(crate) fn store_context<Sub>(
    attributes: &Attributes<'_>,
    id: &tracing::Id,
    context: Context<'_, Sub>,
) where
    Sub: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    if attributes.metadata().target() != "regressify::diagnostics" {
        return;
    }
    let mut visitor = SafeVisitor::default();
    attributes.record(&mut visitor);
    if let Some(safe) = visitor.context
        && let Some(span) = context.span(id)
    {
        span.extensions_mut().replace(safe);
    }
}

pub(crate) fn event_record<Sub>(
    event: &tracing::Event<'_>,
    context: Context<'_, Sub>,
) -> Option<DiagnosticRecordV1>
where
    Sub: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    if event.metadata().target() != "regressify::diagnostics" {
        return None;
    }
    let mut visitor = SafeVisitor::default();
    event.record(&mut visitor);
    let mut record = visitor.record?;
    if let Some(scope) = context.event_scope(event) {
        for span in scope.from_root() {
            if let Some(safe) = span.extensions().get::<DiagnosticContext>() {
                safe.apply(&mut record);
            }
        }
    }
    record.validate().ok()?;
    Some(record)
}

pub fn emit(record: &DiagnosticRecordV1) -> Result<()> {
    record.validate()?;
    let encoded = serde_json::to_string(record)?;
    match record.severity {
        Severity::Trace => {
            tracing::trace!(target: "regressify::diagnostics", diagnostic_record = encoded.as_str())
        }
        Severity::Debug => {
            tracing::debug!(target: "regressify::diagnostics", diagnostic_record = encoded.as_str())
        }
        Severity::Info => {
            tracing::info!(target: "regressify::diagnostics", diagnostic_record = encoded.as_str())
        }
        Severity::Warn => {
            tracing::warn!(target: "regressify::diagnostics", diagnostic_record = encoded.as_str())
        }
        Severity::Error => {
            tracing::error!(target: "regressify::diagnostics", diagnostic_record = encoded.as_str())
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::record::*;
    use super::*;
    use tracing_subscriber::prelude::*;

    fn execution() -> ExecutionIdentity {
        ExecutionIdentity {
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
        }
    }

    #[test]
    fn tracing_adapter_matches_legacy_live_progress_without_result_transitions() {
        let actual = Arc::new(Mutex::new(progress::LiveProgress::default()));
        let received = actual.clone();
        let adapter = Arc::new(LiveProgressAdapter::new(move |event| {
            received.lock().unwrap().apply(progress::Record {
                version: 1,
                elapsed_ms: 0,
                event,
            })
        }));
        let mut expected = progress::LiveProgress::default();
        let mut records = Vec::new();
        for job_id in [12, 43] {
            let queued = progress::Record {
                version: 1,
                elapsed_ms: 0,
                event: progress::Event::Queued {
                    job: progress::Job {
                        id: job_id,
                        suite_id: "suite".into(),
                        suite_name: "Suite".into(),
                        scenario_id: format!("scenario-{job_id}"),
                        label: "Scenario".into(),
                        viewport: "desktop".into(),
                        captures: Vec::new(),
                    },
                },
            };
            expected.apply(queued.clone());
            actual.lock().unwrap().apply(queued);
            let identity = execution();
            adapter.bind(identity.execution_id.clone(), job_id).unwrap();
            let mut record = DiagnosticRecordV1::new(
                OperationId::new(),
                OperationKind::Capture,
                Stage::ActionsFrames,
                DiagnosticEvent::StageStarted,
            );
            record.execution = Some(identity);
            records.push(record);
        }
        let subscriber = tracing_subscriber::registry().with(DiagnosticLayer::new(adapter, |_| {}));
        tracing::subscriber::with_default(subscriber, || {
            for index in [0, 1, 0, 1] {
                emit(&records[index]).unwrap();
                expected.apply(progress::Record {
                    version: 1,
                    elapsed_ms: 0,
                    event: progress::Event::Log {
                        id: [12, 43][index],
                        message: format_record(&records[index]),
                    },
                });
            }
        });
        assert_eq!(
            serde_json::to_value(&expected).unwrap(),
            serde_json::to_value(&*actual.lock().unwrap()).unwrap()
        );
        assert!(
            actual
                .lock()
                .unwrap()
                .scenarios
                .values()
                .all(|scenario| scenario.status == progress::Status::Queued)
        );
        assert_eq!(actual.lock().unwrap().logs.len(), 6);
    }

    #[test]
    fn span_context_is_inherited_and_arbitrary_tracing_fields_are_not_forwarded() {
        let records = Arc::new(Mutex::new(Vec::new()));
        let received = records.clone();
        let subscriber = tracing_subscriber::registry().with(DiagnosticLayer::new(
            Arc::new(LiveProgressAdapter::default()),
            move |record| received.lock().unwrap().push(record.clone()),
        ));
        let context = DiagnosticContext {
            operation_id: OperationId::new(),
            operation_kind: OperationKind::Capture,
            capture_run_id: None,
            execution: Some(execution()),
        };
        tracing::subscriber::with_default(subscriber, || {
            context.span().in_scope(|| {
                tracing::info!(password = "synthetic-secret", "private body");
                let mut record = DiagnosticRecordV1::new(
                    OperationId::new(),
                    OperationKind::Capture,
                    Stage::Navigation,
                    DiagnosticEvent::Decision,
                );
                record.severity = Severity::Debug;
                emit(&record).unwrap();
            });
        });
        let records = records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].operation_id, context.operation_id);
        assert_eq!(records[0].execution, context.execution);
        assert_eq!(records[0].severity, Severity::Debug);
        assert!(
            !serde_json::to_string(&*records)
                .unwrap()
                .contains("synthetic-secret")
        );
    }

    #[test]
    fn missing_or_rebound_executions_cannot_guess_live_job_ownership() {
        let adapter = LiveProgressAdapter::new(|_| panic!("Unbound record must not be delivered"));
        let identity = execution();
        adapter.bind(identity.execution_id.clone(), 1).unwrap();
        assert!(adapter.bind(identity.execution_id.clone(), 2).is_err());
        adapter.unbind(&identity.execution_id);
        let mut record = DiagnosticRecordV1::new(
            OperationId::new(),
            OperationKind::Capture,
            Stage::Finish,
            DiagnosticEvent::Finished,
        );
        record.execution = Some(identity);
        adapter.on_diagnostic(&record);
        record.message = "unreviewed".into();
        adapter.on_diagnostic(&record);
    }
}
