use std::{
    path::Path,
    sync::{Mutex, OnceLock, Weak},
};

use super::{
    health::archives_disabled,
    install::emergency_warning,
    record::*,
    runtime::{self, Operation, OperationContext},
};

static APPLICATION: OnceLock<Mutex<Weak<OperationContext>>> = OnceLock::new();

pub struct StartupLog {
    operation: Option<Operation>,
}

impl StartupLog {
    pub fn begin() -> Self {
        Self::begin_for(SafeValue::Cli)
    }

    pub fn begin_for(component: SafeValue) -> Self {
        if APPLICATION
            .get()
            .and_then(|current| current.lock().ok()?.upgrade())
            .is_some()
        {
            return Self { operation: None };
        }
        if archives_disabled(|name| std::env::var(name).ok()).unwrap_or(true) {
            return Self { operation: None };
        }
        let log = match directories::BaseDirs::new() {
            Some(directories) => Self::at(
                &directories
                    .data_local_dir()
                    .join("regressify/diagnostics/startup"),
                component,
            ),
            None => {
                emergency_warning("Startup diagnostic directory is unavailable");
                Self { operation: None }
            }
        };
        if let Some(operation) = &log.operation
            && let Ok(mut current) = APPLICATION.get_or_init(Default::default).lock()
            && current.upgrade().is_none()
        {
            *current = std::sync::Arc::downgrade(&operation.context());
        }
        log
    }

    fn at(root: &Path, component: SafeValue) -> Self {
        let operation = Operation::start(root, OperationKind::Application, None, None, None);
        let context = operation.context();
        context.set_display_level(Severity::Error);
        runtime::in_scope(Some(context.clone()), || {
            context.record(
                Stage::Preflight,
                DiagnosticEvent::Started,
                Severity::Info,
                [(Field::Reason, FieldValue::Kind(component))],
                runtime::Completion::default(),
            );
        });
        Self {
            operation: Some(operation),
        }
    }

    pub fn finish(mut self, result: &anyhow::Result<()>) {
        if let Some(operation) = self.operation.take() {
            let outcome = if result.is_ok() {
                Outcome::Success
            } else {
                Outcome::Failure
            };
            let context = operation.context();
            runtime::in_scope(Some(context.clone()), || {
                context.record(
                    Stage::Finish,
                    DiagnosticEvent::StageFinished,
                    outcome.severity(),
                    [],
                    runtime::Completion {
                        outcome: Some(outcome),
                        error: result.as_ref().err().map(SanitizedError::capture),
                        duration_ms: None,
                    },
                );
            });
            operation.finish_blocking(outcome);
        }
    }
}

pub fn warning(reason: SafeValue, error: &anyhow::Error) {
    let context = APPLICATION
        .get()
        .and_then(|current| current.lock().ok()?.upgrade());
    if let Some(context) = context {
        record_warning(context, reason, error);
    } else {
        emergency_warning("Application warning could not be archived");
    }
}

fn record_warning(
    context: std::sync::Arc<OperationContext>,
    reason: SafeValue,
    error: &anyhow::Error,
) {
    runtime::in_scope(Some(context.clone()), || {
        context.record(
            Stage::Preflight,
            DiagnosticEvent::StageFinished,
            Severity::Warn,
            [(Field::Reason, FieldValue::Kind(reason))],
            runtime::Completion {
                outcome: Some(Outcome::Failure),
                error: Some(SanitizedError::capture(error)),
                duration_ms: None,
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::journal::{ArchiveState, JournalReader};

    #[test]
    fn application_warnings_survive_background_threads_without_raw_errors() {
        let root = tempfile::tempdir().unwrap();
        let log = StartupLog::at(root.path(), SafeValue::Desktop);
        let context = log.operation.as_ref().unwrap().context();
        let operation = context.identity.operation_id.clone();
        std::thread::spawn(move || {
            record_warning(
                context,
                SafeValue::Shell,
                &anyhow::anyhow!("Permission denied password=synthetic-secret"),
            );
        })
        .join()
        .unwrap();
        log.finish(&Ok(()));
        let reader = JournalReader::open(root.path(), &operation).unwrap();
        let mut warnings = 0;
        reader
            .visit(|record| {
                assert_eq!(record.operation_kind, OperationKind::Application);
                assert!(
                    !serde_json::to_string(&record)
                        .unwrap()
                        .contains("synthetic-secret")
                );
                if record.severity == Severity::Warn {
                    warnings += 1;
                    assert_eq!(
                        record.fields.get(&Field::Reason),
                        Some(&FieldValue::Kind(SafeValue::Shell))
                    );
                    assert_eq!(
                        record.error_cause.unwrap().category,
                        ErrorCategory::Permission
                    );
                }
            })
            .unwrap();
        assert_eq!(warnings, 1);
        assert_eq!(reader.archive_state(), ArchiveState::Complete);
    }

    #[test]
    fn startup_failures_are_durable_sanitized_and_never_pruned_on_restart() {
        let root = tempfile::tempdir().unwrap();
        for result in [
            Ok(()),
            Err(anyhow::anyhow!("Invalid config password=synthetic-secret")),
        ] {
            let log = StartupLog::at(root.path(), SafeValue::Desktop);
            let operation = log
                .operation
                .as_ref()
                .unwrap()
                .context()
                .identity
                .operation_id
                .clone();
            log.finish(&result);
            let reader = JournalReader::open(root.path(), &operation).unwrap();
            assert_eq!(reader.metadata.operation_kind, OperationKind::Application);
            assert_eq!(reader.archive_state(), ArchiveState::Complete);
            reader
                .visit(|record| {
                    if record.stage == Stage::Finish {
                        assert_eq!(
                            record.severity,
                            if result.is_ok() {
                                Severity::Info
                            } else {
                                Severity::Error
                            }
                        );
                    }
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
        StartupLog::at(&blocked, SafeValue::Desktop).finish(&Ok(()));
        assert_eq!(std::fs::read(blocked).unwrap(), b"existing");
    }
}
