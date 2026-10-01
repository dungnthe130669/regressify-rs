use std::{
    io::{self, Write},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

use tracing_subscriber::prelude::*;

use super::{
    adapter::{DiagnosticLayer, LiveProgressAdapter, format_record},
    record::{DiagnosticRecordV1, Severity},
};

static INSTALLATION: OnceLock<Installation> = OnceLock::new();

pub struct Installation {
    pub adapter: Arc<LiveProgressAdapter>,
    console: Arc<Console>,
    installed: bool,
}

impl Installation {
    pub fn set_console_level(&self, level: Severity) {
        self.console.level.store(level as u8, Ordering::Relaxed);
    }

    pub fn installed(&self) -> bool {
        self.installed
    }
}

struct Console {
    output: Mutex<Box<dyn Write + Send>>,
    level: AtomicU8,
    failed: AtomicBool,
    silent: AtomicBool,
}

impl Console {
    fn new(output: impl Write + Send + 'static, level: Severity) -> Self {
        Self {
            output: Mutex::new(Box::new(output)),
            level: AtomicU8::new(level as u8),
            failed: AtomicBool::new(false),
            silent: AtomicBool::new(false),
        }
    }

    fn receive(&self, record: &DiagnosticRecordV1) {
        let level = super::runtime::current()
            .and_then(|context| context.display_level())
            .map(|level| level as u8)
            .unwrap_or_else(|| self.level.load(Ordering::Relaxed));
        if self.silent.load(Ordering::Relaxed)
            || crate::common::NO_PRINT.get().copied().unwrap_or(false)
            || (record.severity as u8) < level
            || self.failed.load(Ordering::Relaxed)
        {
            return;
        }
        let execution = record
            .execution
            .as_ref()
            .map(|execution| execution.execution_id.as_str())
            .unwrap_or(record.operation_id.as_str());
        let result = self
            .output
            .lock()
            .map_err(|_| io::Error::other("Diagnostic console unavailable"))
            .and_then(|mut output| {
                writeln!(
                    output,
                    "[{}ms {execution}] {}",
                    record.elapsed_ms,
                    format_record(record)
                )
            });
        if result.is_err() {
            self.failed.store(true, Ordering::Relaxed);
        }
    }

    fn flush(&self) -> io::Result<()> {
        self.output
            .lock()
            .map_err(|_| io::Error::other("Diagnostic console unavailable"))?
            .flush()
    }
}

pub fn install_subscriber() -> &'static Installation {
    INSTALLATION.get_or_init(|| {
        let adapter = Arc::new(LiveProgressAdapter::default());
        let console = Arc::new(Console::new(io::stderr(), Severity::Info));
        let output = console.clone();
        let subscriber = tracing_subscriber::registry()
            .with(super::runtime::JournalLayer)
            .with(DiagnosticLayer::new(adapter.clone(), move |record| {
                output.receive(record)
            }));
        let installed = tracing::subscriber::set_global_default(subscriber).is_ok();
        if !installed {
            emergency_warning(
                "Diagnostic subscriber unavailable; an existing subscriber was retained",
            );
        }
        Installation {
            adapter,
            console,
            installed,
        }
    })
}

pub fn current() -> Option<&'static Installation> {
    INSTALLATION.get()
}

pub fn configure_console(
    level: Option<Severity>,
    verbose: bool,
    silent: bool,
) -> anyhow::Result<()> {
    let flag = level.or_else(|| verbose.then_some(Severity::Debug));
    let environment = std::env::var("REGRESSIFY_LOG_LEVEL").ok();
    let resolved =
        crate::config::resolve_log_level(flag, environment.as_deref(), None, None, None)?;
    if let Some(installation) = current() {
        installation.set_console_level(resolved);
        installation.console.silent.store(silent, Ordering::Relaxed);
    }
    Ok(())
}

pub fn flush() {
    if let Some(installation) = INSTALLATION.get()
        && (installation.console.flush().is_err()
            || installation.console.failed.load(Ordering::Relaxed))
    {
        emergency_warning("Diagnostic console could not be fully written");
    }
}

pub(crate) fn emergency_warning(message: &'static str) {
    let _ = writeln!(io::stderr().lock(), "Warning: {message}");
}

#[cfg(test)]
mod tests {
    use super::super::{adapter::emit, record::*};
    use super::*;

    #[derive(Clone, Default)]
    struct Output(Arc<Mutex<Vec<u8>>>);

    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn quiet_console_does_not_disable_standard_debug_delivery() {
        let output = Output::default();
        let console = Arc::new(Console::new(output.clone(), Severity::Error));
        let captured = Arc::new(Mutex::new(Vec::new()));
        let received = captured.clone();
        let subscriber = tracing_subscriber::registry().with(DiagnosticLayer::new(
            Arc::new(LiveProgressAdapter::default()),
            move |record| {
                received.lock().unwrap().push(record.clone());
                console.receive(record);
            },
        ));
        tracing::subscriber::with_default(subscriber, || {
            let mut record = DiagnosticRecordV1::new(
                OperationId::new(),
                OperationKind::Capture,
                Stage::QueueSetup,
                DiagnosticEvent::Decision,
            );
            record.severity = Severity::Debug;
            emit(&record).unwrap();
        });
        assert_eq!(captured.lock().unwrap().len(), 1);
        assert!(output.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn per_execution_console_levels_do_not_filter_archives() {
        use crate::diagnostics::{journal::JournalReader, runtime};
        let root = tempfile::tempdir().unwrap();
        let output = Output::default();
        let console = Console::new(output.clone(), Severity::Info);
        for level in [Severity::Trace, Severity::Error] {
            let operation =
                runtime::Operation::start(root.path(), OperationKind::Capture, None, None, None);
            operation.context().set_display_level(level);
            runtime::scope(Some(operation.context()), async {
                let mut record = DiagnosticRecordV1::new(
                    operation.context().identity.operation_id.clone(),
                    OperationKind::Capture,
                    Stage::QueueSetup,
                    DiagnosticEvent::Decision,
                );
                record.severity = Severity::Debug;
                console.receive(&record);
                runtime::event(
                    Stage::QueueSetup,
                    DiagnosticEvent::Decision,
                    Severity::Debug,
                    [],
                );
            })
            .await;
            let reference = operation.finish(Outcome::Success).await;
            assert_eq!(
                JournalReader::open(root.path(), &reference.operation_id)
                    .unwrap()
                    .visit(|_| {})
                    .unwrap()
                    .records,
                3
            );
        }
        assert_eq!(
            String::from_utf8(output.0.lock().unwrap().clone())
                .unwrap()
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn repeated_installation_returns_the_same_owner_without_panicking() {
        let first = install_subscriber();
        let second = install_subscriber();
        assert!(std::ptr::eq(first, second));
        flush();
    }

    #[test]
    fn console_writes_one_line_and_reports_flush_failure() {
        let output = Output::default();
        let console = Console::new(output.clone(), Severity::Info);
        let record = DiagnosticRecordV1::new(
            OperationId::new(),
            OperationKind::Deployment,
            Stage::Preflight,
            DiagnosticEvent::Started,
        );
        console.receive(&record);
        console.flush().unwrap();
        assert_eq!(
            String::from_utf8(output.0.lock().unwrap().clone())
                .unwrap()
                .lines()
                .count(),
            1
        );
        struct FailedFlush;
        impl Write for FailedFlush {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("synthetic flush failure"))
            }
        }
        assert!(Console::new(FailedFlush, Severity::Info).flush().is_err());
    }
}
