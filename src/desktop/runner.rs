//! Bundled engine execution with bounded output and process-tree cancellation.

use std::{
    collections::VecDeque,
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use command_group::CommandGroup;
use serde::{Deserialize, Serialize};

use crate::cli::Cli;
use crate::cli::args::ScenarioFilter;
use crate::progress::{EventReader, LiveProgress};

const LOG_QUEUE_CAPACITY: usize = 256;
const MAX_LOG_CHUNK: usize = 16 * 1024;
const MAX_LOG_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunCommand {
    Test,
    Reference,
    Compare,
    Approve,
    Snapshot,
    Deploy,
    Optimize,
    Install,
    Init,
    Version,
}

impl RunCommand {
    pub const ALL: &[Self] = &[
        Self::Test,
        Self::Reference,
        Self::Compare,
        Self::Approve,
        Self::Snapshot,
        Self::Optimize,
        Self::Install,
        Self::Init,
    ];
    pub fn argument(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Reference => "ref",
            Self::Compare => "compare",
            Self::Approve => "approve",
            Self::Snapshot => "snapshot",
            Self::Deploy => "snapshot",
            Self::Optimize => "optimize",
            Self::Install => "install",
            Self::Init => "init",
            Self::Version => "version",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Test => "Test",
            Self::Reference => "Capture baseline",
            Self::Compare => "Compare",
            Self::Approve => "Approve",
            Self::Snapshot => "Snapshot",
            Self::Deploy => "Deploy",
            Self::Optimize => "Optimize",
            Self::Install => "Install Chromium",
            Self::Init => "Initialize workspace",
            Self::Version => "Version",
        }
    }
    pub fn needs_confirmation(self) -> bool {
        matches!(self, Self::Reference | Self::Approve | Self::Optimize)
    }
    pub fn action_label(self) -> &'static str {
        match self {
            Self::Test => "Run Test",
            Self::Reference => "Run Reference",
            Self::Compare => "Compare Results",
            Self::Approve => "Approve Results",
            Self::Snapshot => "Build Report",
            Self::Deploy => "Deploy",
            Self::Optimize => "Optimize Images",
            Self::Install => "Install Chromium",
            Self::Init => "Initialize Workspace",
            Self::Version => "Show Version",
        }
    }

    pub fn confirmation_message(self) -> &'static str {
        match self {
            Self::Reference => "Replace reference images with new captures?",
            Self::Approve => "Replace reference images with the selected test results?",
            Self::Optimize => "Optimize the saved image files in place?",
            _ => "",
        }
    }

    pub fn captures(self) -> bool {
        matches!(self, Self::Test | Self::Reference)
    }
    pub fn supports_suites(self) -> bool {
        matches!(
            self,
            Self::Test
                | Self::Reference
                | Self::Compare
                | Self::Approve
                | Self::Snapshot
                | Self::Deploy
        )
    }

    pub fn supports_scenarios(self) -> bool {
        self.supports_suites() && !matches!(self, Self::Snapshot | Self::Deploy)
    }

    pub fn uses_output_suite_ids(self) -> bool {
        matches!(
            self,
            Self::Compare | Self::Approve | Self::Snapshot | Self::Deploy
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct RunOptions {
    pub deployment: Option<super::deployment::DeploymentJob>,
    pub site_env: String,
    pub regressify_dir: String,
    pub run_id: String,
    pub proxy: String,
    pub async_capture_limit: u32,
    pub async_capture_ramp_ms: u64,
    pub debug: bool,
    pub verbose: bool,
    pub required_login: bool,
    pub suites: Vec<String>,
    pub scenarios: Vec<ScenarioFilter>,
    pub fetch_browser: Option<bool>,
    pub skip_compare: bool,
    pub mis_match_threshold: f64,
    pub replacement_profile: String,
    pub async_compare_limit: usize,
    pub speed: u8,
    pub min_size_kb: u64,
    pub title: String,
    pub heading: String,
    pub subheading: String,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            deployment: None,
            site_env: String::new(),
            regressify_dir: String::new(),
            run_id: String::new(),
            proxy: String::new(),
            async_capture_limit: 5,
            async_capture_ramp_ms: 1000,
            debug: false,
            verbose: false,
            required_login: false,
            suites: Vec::new(),
            scenarios: Vec::new(),
            fetch_browser: None,
            skip_compare: false,
            mis_match_threshold: 0.1,
            replacement_profile: String::new(),
            async_compare_limit: 2,
            speed: 8,
            min_size_kb: 50,
            title: "Visual Tests Summary | Regressify".into(),
            heading: "Visual Tests Summary".into(),
            subheading: "Snapshot regression test results".into(),
        }
    }
}

impl RunOptions {
    pub fn output_directory(&self, root: &Path) -> PathBuf {
        if !self.regressify_dir.is_empty() {
            root.join(&self.regressify_dir)
        } else if !self.site_env.is_empty() {
            root.join(format!(".regressify__{}", self.site_env))
        } else {
            root.join(".regressify")
        }
    }

    pub fn applies(key: &str, command: RunCommand) -> bool {
        match key {
            "deployment" => false,
            "suites" => command.supports_suites(),
            "scenarios" => command.supports_scenarios(),
            "fetchBrowser" | "asyncCaptureLimit" | "asyncCaptureRampMs" | "debug"
            | "requiredLogin" => command.captures(),
            "skipCompare" => command == RunCommand::Test,
            "replacementProfile" => command == RunCommand::Test,
            "misMatchThreshold" => matches!(command, RunCommand::Test | RunCommand::Compare),
            "asyncCompareLimit" => command == RunCommand::Compare,
            "speed" | "minSizeKb" => command == RunCommand::Optimize,
            "title" | "heading" | "subheading" => {
                matches!(command, RunCommand::Snapshot | RunCommand::Deploy)
            }
            _ => true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRequest {
    pub root: PathBuf,
    pub command: RunCommand,
    pub options: RunOptions,
}

impl RunRequest {
    pub fn cli(&self) -> Result<Cli> {
        Ok(Cli::try_parse_from(
            std::iter::once(OsString::from("regressify")).chain(self.arguments()?),
        )?)
    }

    pub fn arguments(&self) -> Result<Vec<OsString>> {
        if self.command == RunCommand::Deploy {
            let deployment = self
                .options
                .deployment
                .as_ref()
                .context("Choose a deployment target")?;
            deployment.validate()?;
            ensure!(
                self.options.suites
                    == deployment
                        .suite_ids
                        .iter()
                        .map(|id| glob::Pattern::escape(id))
                        .collect::<Vec<_>>(),
                "Deployment suite filters changed; review the selection"
            );
        }
        let mut arguments = vec![
            OsString::from("--root"),
            self.root.as_os_str().to_os_string(),
        ];
        let options = &self.options;
        for (flag, value) in [
            ("--site-env", &options.site_env),
            ("--regressify-dir", &options.regressify_dir),
            ("--run-id", &options.run_id),
            ("--proxy", &options.proxy),
        ] {
            if !value.is_empty() {
                arguments.extend([flag.into(), value.into()]);
            }
        }
        arguments.extend([
            "--async-capture-limit".into(),
            options.async_capture_limit.to_string().into(),
            "--async-capture-ramp-ms".into(),
            options.async_capture_ramp_ms.to_string().into(),
        ]);
        for (flag, enabled) in [
            ("--debug", options.debug),
            ("--verbose", options.verbose),
            ("--required-login", options.required_login),
        ] {
            if enabled {
                arguments.push(flag.into());
            }
        }
        arguments.push(self.command.argument().into());
        if self.command.supports_suites() {
            for suite in &options.suites {
                arguments.extend(["--test-suite".into(), suite.into()]);
            }
        }
        if self.command.supports_scenarios() {
            for scenario in &options.scenarios {
                arguments.extend(["--scenario".into(), scenario.to_string().into()]);
            }
        }
        if self.command.captures()
            && let Some(fetch) = options.fetch_browser
        {
            arguments.extend(["--fetch-browser".into(), fetch.to_string().into()]);
        }
        if self.command == RunCommand::Test {
            if options.skip_compare {
                arguments.push("--skip-compare".into());
            }
            if !options.replacement_profile.is_empty() {
                arguments.extend([
                    "--replacement-profile".into(),
                    options.replacement_profile.clone().into(),
                ]);
            }
        }
        if matches!(self.command, RunCommand::Test | RunCommand::Compare) {
            arguments.extend([
                "--mis-match-threshold".into(),
                options.mis_match_threshold.to_string().into(),
            ]);
        }
        if self.command == RunCommand::Compare {
            arguments.extend([
                "--async-compare-limit".into(),
                options.async_compare_limit.to_string().into(),
            ]);
        }
        if self.command == RunCommand::Optimize {
            arguments.extend([
                "--speed".into(),
                options.speed.to_string().into(),
                "--min-size-kb".into(),
                options.min_size_kb.to_string().into(),
            ]);
        }
        if matches!(self.command, RunCommand::Snapshot | RunCommand::Deploy) {
            for (flag, value) in [
                ("--title", &options.title),
                ("--heading", &options.heading),
                ("--subheading", &options.subheading),
            ] {
                arguments.extend([flag.into(), value.into()]);
            }
        }
        // Validate with the real clap parser before starting a child process.
        Cli::try_parse_from(
            std::iter::once(OsString::from("regressify")).chain(arguments.clone()),
        )?;
        Ok(arguments)
    }

    fn process(&self) -> Result<Command> {
        ensure!(self.root.is_dir(), "Workspace directory is unavailable");
        self.arguments()?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg(super::worker::WORKER_FLAG)
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        Ok(command)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    /// Exit zero describes command execution, not whether visual comparisons passed.
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub stderr: bool,
    pub text: String,
}

struct RunHandle {
    logs: Receiver<LogLine>,
    cancel: SyncSender<()>,
    worker: Option<JoinHandle<RunOutcome>>,
    omitted: Arc<AtomicUsize>,
    progress: Arc<Mutex<LiveProgress>>,
    _events_file: Option<tempfile::NamedTempFile>,
}

impl RunHandle {
    fn spawn(
        mut command: Command,
        payload: Option<Vec<u8>>,
        events_file: Option<tempfile::NamedTempFile>,
        scratch: Option<tempfile::TempDir>,
    ) -> Result<Self> {
        let (sender, logs) = mpsc::sync_channel(LOG_QUEUE_CAPACITY);
        let (cancel, cancellation) = mpsc::sync_channel(1);
        let omitted = Arc::new(AtomicUsize::new(0));
        let lost = omitted.clone();
        let mut events = events_file
            .as_ref()
            .map(|file| EventReader::open(file.path()))
            .transpose()?;
        let progress = Arc::new(Mutex::new(LiveProgress::default()));
        let live = progress.clone();
        let worker = thread::Builder::new()
            .name("regressify-run".into())
            .spawn(move || {
                let _scratch = scratch;
                let result = (|| -> Result<RunOutcome> {
                    let mut child = command
                        .group_spawn()
                        .context("Cannot launch the bundled engine worker")?;
                    let stdout = child.inner().stdout.take();
                    let stderr = child.inner().stderr.take();
                    let mut readers = Vec::new();
                    if let Some(stdout) = stdout {
                        readers.push(read_stream(stdout, false, sender.clone(), lost.clone()));
                    }
                    if let Some(stderr) = stderr {
                        readers.push(read_stream(stderr, true, sender, lost));
                    }
                    if let Some(payload) = payload {
                        let sent = child
                            .inner()
                            .stdin
                            .take()
                            .ok_or_else(|| anyhow!("Worker input is unavailable"))
                            .and_then(|mut input| {
                                input
                                    .write_all(&payload)
                                    .context("Cannot send run settings to worker")
                            });
                        if let Err(error) = sent {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(error);
                        }
                    }
                    let outcome = (|| -> Result<RunOutcome> {
                        loop {
                            if let Some(events) = &mut events {
                                let mut state = live
                                    .lock()
                                    .map_err(|_| anyhow!("Run progress is unavailable"))?;
                                events.drain(|record| state.apply(record))?;
                            }
                            if let Some(status) = child.try_wait()? {
                                if let Some(events) = &mut events {
                                    let mut state = live
                                        .lock()
                                        .map_err(|_| anyhow!("Run progress is unavailable"))?;
                                    events.drain(|record| state.apply(record))?;
                                    events.complete()?;
                                }
                                break Ok(if status.success() {
                                    RunOutcome::Completed
                                } else {
                                    RunOutcome::Failed(format!(
                                        "Engine worker exited with {}",
                                        status
                                            .code()
                                            .map(|code| code.to_string())
                                            .unwrap_or_else(|| "a signal".into())
                                    ))
                                });
                            }
                            match cancellation.recv_timeout(Duration::from_millis(50)) {
                                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    child
                                        .kill()
                                        .context("Cannot terminate the engine process group")?;
                                    child.wait()?;
                                    break Ok(RunOutcome::Cancelled);
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                            }
                        }
                    })();
                    if outcome.is_err() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    for reader in readers {
                        reader
                            .join()
                            .map_err(|_| anyhow!("Log reader stopped unexpectedly"))??;
                    }
                    outcome
                })();
                let outcome =
                    result.unwrap_or_else(|error| RunOutcome::Failed(format!("{error:#}")));
                if let Ok(mut state) = live.lock() {
                    match &outcome {
                        RunOutcome::Cancelled => state.end(true, None),
                        RunOutcome::Failed(error) => {
                            let message = state.error.clone().unwrap_or_else(|| error.clone());
                            state.end(false, Some(message));
                        }
                        RunOutcome::Completed if !state.finished => state.end(false, None),
                        _ => {}
                    }
                }
                outcome
            })?;
        Ok(Self {
            logs,
            cancel,
            worker: Some(worker),
            omitted,
            progress,
            _events_file: events_file,
        })
    }

    fn cancel(&self) {
        let _ = self.cancel.try_send(());
    }
}

impl Drop for RunHandle {
    fn drop(&mut self) {
        self.cancel();
        // Close waits for process-group cleanup so Chromium is not orphaned on quit.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_stream(
    stream: impl Read + Send + 'static,
    stderr: bool,
    sender: SyncSender<LogLine>,
    omitted: Arc<AtomicUsize>,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        loop {
            let mut bytes = Vec::new();
            let count = (&mut reader)
                .take(MAX_LOG_CHUNK as u64)
                .read_until(b'\n', &mut bytes)?;
            if count == 0 {
                break;
            }
            let line = LogLine {
                stderr,
                text: String::from_utf8_lossy(&bytes).into_owned(),
            };
            if sender.try_send(line).is_err() {
                omitted.fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(())
    })
}

#[derive(Default)]
pub struct RunManager {
    active: Option<RunHandle>,
    pub logs: VecDeque<LogLine>,
    pub outcome: Option<RunOutcome>,
    pub request: Option<RunRequest>,
    pub omitted_chunks: usize,
    log_bytes: usize,
    pub progress: LiveProgress,
}

impl RunManager {
    pub fn running(&self) -> bool {
        self.active.is_some()
    }

    pub fn start(&mut self, mut request: RunRequest) -> Result<()> {
        ensure!(!self.running(), "A command is already running");
        if request.command.captures() && request.options.run_id.is_empty() {
            request.options.run_id = format!("desktop-{}", uuid::Uuid::new_v4().simple());
        }
        let command = request.process()?;
        let events = tempfile::NamedTempFile::new()?;
        let scratch = (request.command == RunCommand::Deploy)
            .then(|| {
                tempfile::Builder::new()
                    .prefix("regressify-deploy-")
                    .tempdir()
            })
            .transpose()?;
        let payload = serde_json::to_vec(&super::worker::WorkerRequest {
            run: request.clone(),
            events_path: Some(events.path().into()),
            scratch_directory: scratch
                .as_ref()
                .map(|directory| directory.path().to_path_buf()),
        })?;
        ensure!(
            payload.len() <= super::worker::MAX_REQUEST_BYTES,
            "Run request exceeds 1 MiB"
        );
        self.active = Some(RunHandle::spawn(
            command,
            Some(payload),
            Some(events),
            scratch,
        )?);
        self.progress = LiveProgress::default();
        self.logs.clear();
        self.log_bytes = 0;
        self.omitted_chunks = 0;
        self.outcome = None;
        self.request = Some(request);
        Ok(())
    }

    pub fn cancel(&self) {
        if let Some(active) = &self.active {
            active.cancel();
        }
    }

    /// Non-blocking UI tick; completion is separate from the bounded log queue.
    pub fn poll(&mut self) -> bool {
        let Some(active) = &self.active else {
            return false;
        };
        let lines = active.logs.try_iter().collect::<Vec<_>>();
        self.omitted_chunks = active.omitted.load(Ordering::Relaxed);
        let finished = active.worker.as_ref().is_some_and(JoinHandle::is_finished);
        if let Ok(state) = active.progress.lock() {
            self.progress = state.clone();
        }
        for line in lines {
            self.push_log(line);
        }
        if finished && let Some(mut active) = self.active.take() {
            for line in active.logs.try_iter() {
                self.push_log(line);
            }
            self.outcome = active.worker.take().map(|worker| {
                worker.join().unwrap_or_else(|_| {
                    RunOutcome::Failed("Execution worker stopped unexpectedly".into())
                })
            });
        }
        finished
    }

    fn push_log(&mut self, line: LogLine) {
        self.log_bytes += line.text.len();
        self.logs.push_back(line);
        while self.log_bytes > MAX_LOG_BYTES {
            if let Some(line) = self.logs.pop_front() {
                self.log_bytes -= line.text.len();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_arguments_include_suites_but_not_scenario_filters() {
        let mut request = request(RunCommand::Snapshot);
        request.options.suites = vec!["shop-output".into()];
        request.options.scenarios = vec!["shop:home".parse().unwrap()];
        let arguments = request.arguments().unwrap();
        assert!(arguments.contains(&OsString::from("--test-suite")));
        assert!(arguments.contains(&OsString::from("shop-output")));
        assert!(!arguments.contains(&OsString::from("--scenario")));
        assert!(RunOptions::applies("suites", RunCommand::Snapshot));
        assert!(!RunOptions::applies("scenarios", RunCommand::Snapshot));
    }

    fn request(command: RunCommand) -> RunRequest {
        RunRequest {
            root: PathBuf::from("C:/QA projects/Shop & checkout"),
            command,
            options: RunOptions::default(),
        }
    }

    #[test]
    fn all_command_arguments_match_clap() {
        for command in RunCommand::ALL {
            let request = request(*command);
            assert!(request.arguments().is_ok(), "{command:?}");
        }
        let mut request = request(RunCommand::Test);
        request.options.suites = vec!["shop & checkout".into(), "mobile".into()];
        request.options.site_env = "staging".into();
        request.options.regressify_dir = "output with spaces".into();
        request.options.fetch_browser = Some(false);
        request.options.required_login = true;
        request.options.replacement_profile = "QA ; profile".into();
        let arguments = request.arguments().unwrap();
        assert!(arguments.contains(&OsString::from("shop & checkout")));
        assert!(arguments.contains(&OsString::from("QA ; profile")));
        assert_eq!(arguments[1], request.root.as_os_str());
    }

    #[test]
    fn invalid_options_fail_before_process_launch() {
        let mut request = request(RunCommand::Test);
        request.options.async_capture_limit = 0;
        assert!(request.arguments().is_err());
        request.options.async_capture_limit = 1;
        request.options.site_env = "../escape".into();
        assert!(request.arguments().is_err());
        request.options.site_env.clear();
        request.options.mis_match_threshold = f64::NAN;
        assert!(request.arguments().is_err());
    }

    #[test]
    fn output_directory_matches_cli_precedence() {
        let mut options = RunOptions::default();
        let root = Path::new("workspace");
        assert_eq!(options.output_directory(root), root.join(".regressify"));
        options.site_env = "staging".into();
        assert_eq!(
            options.output_directory(root),
            root.join(".regressify__staging")
        );
        options.regressify_dir = "artifacts".into();
        assert_eq!(options.output_directory(root), root.join("artifacts"));
    }

    fn fixture(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "desktop::runner::tests::process_fixture",
                "--nocapture",
            ])
            .env("REGRESSIFY_DESKTOP_FIXTURE", mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    #[test]
    fn process_fixture() {
        let Ok(mode) = std::env::var("REGRESSIFY_DESKTOP_FIXTURE") else {
            return;
        };
        println!("fixture stdout");
        eprintln!("fixture stderr");
        if mode == "progress" {
            let path = std::env::var_os("REGRESSIFY_PROGRESS_FIXTURE").unwrap();
            crate::progress::install(Path::new(&path)).unwrap();
            for index in 0..2000 {
                println!("log record {index}");
            }
            crate::progress::emit(crate::progress::Event::Queued {
                job: crate::progress::Job {
                    id: 1,
                    suite_id: "shop".into(),
                    suite_name: "shop".into(),
                    scenario_id: "home".into(),
                    label: "Home".into(),
                    viewport: "desktop".into(),
                    captures: vec!["home__FINAL".into()],
                },
            });
            crate::progress::emit(crate::progress::Event::Started { id: 1 });
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(crate::progress::scope_scenario(1, async {
                    crate::progress::scenario_log!("Scoped fixture action");
                }));
            crate::progress::emit(crate::progress::Event::Captured {
                id: 1,
                error: None,
                infrastructure: false,
            });
            crate::progress::emit(crate::progress::Event::Finished { error: None });
            crate::progress::finish().unwrap();
        }
        if mode == "wait" {
            thread::park();
        }
        if mode == "fail" {
            std::process::exit(7);
        }
    }

    #[test]
    fn subprocess_reads_both_streams_and_distinguishes_failure() {
        for (mode, success) in [("complete", true), ("fail", false)] {
            let mut handle = RunHandle::spawn(fixture(mode), None, None, None).unwrap();
            let outcome = handle.worker.take().unwrap().join().unwrap();
            assert_eq!(outcome == RunOutcome::Completed, success);
            let logs = handle.logs.try_iter().collect::<Vec<_>>();
            assert!(
                logs.iter()
                    .any(|line| !line.stderr && line.text.contains("fixture stdout"))
            );
            assert!(
                logs.iter()
                    .any(|line| line.stderr && line.text.contains("fixture stderr"))
            );
        }
    }

    #[test]
    fn cancellation_stops_a_running_process_group() {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().to_path_buf();
        std::fs::write(path.join("package.zip"), b"synthetic package").unwrap();
        let mut handle = RunHandle::spawn(fixture("wait"), None, None, Some(scratch)).unwrap();
        handle.logs.recv_timeout(Duration::from_secs(10)).unwrap();
        handle.cancel();
        assert_eq!(
            handle.worker.take().unwrap().join().unwrap(),
            RunOutcome::Cancelled
        );
        assert!(
            !path.exists(),
            "cancelled worker left its staging files behind"
        );
    }

    #[test]
    fn scenario_selection_reaches_each_supported_worker_command() {
        for command in [
            RunCommand::Test,
            RunCommand::Reference,
            RunCommand::Compare,
            RunCommand::Approve,
        ] {
            let request = RunRequest {
                root: PathBuf::from("."),
                command,
                options: RunOptions {
                    suites: vec!["shop".into()],
                    scenarios: vec!["shop:checkout".parse().unwrap()],
                    ..Default::default()
                },
            };
            request.cli().unwrap();
            assert!(request.arguments().unwrap().windows(2).any(|pair| {
                pair == [
                    OsString::from("--scenario"),
                    OsString::from("shop:checkout"),
                ]
            }));
            let restored: RunRequest =
                serde_json::from_slice(&serde_json::to_vec(&request).unwrap()).unwrap();
            assert_eq!(restored.options.scenarios, request.options.scenarios);
        }
    }

    #[test]
    fn log_memory_is_bounded() {
        let mut manager = RunManager::default();
        for _ in 0..300 {
            manager.push_log(LogLine {
                stderr: false,
                text: "x".repeat(MAX_LOG_CHUNK),
            });
        }
        assert!(manager.log_bytes <= MAX_LOG_BYTES);
        assert!(!manager.logs.is_empty());
    }

    #[test]
    fn progress_survives_a_full_console_log_queue() {
        let events = tempfile::NamedTempFile::new().unwrap();
        let mut command = fixture("progress");
        command.env("REGRESSIFY_PROGRESS_FIXTURE", events.path());
        let mut handle = RunHandle::spawn(command, None, Some(events), None).unwrap();
        let outcome = handle.worker.take().unwrap().join().unwrap();
        assert_eq!(outcome, RunOutcome::Completed);
        assert!(handle.omitted.load(Ordering::Relaxed) > 0);
        let progress = handle.progress.lock().unwrap();
        assert!(progress.finished);
        assert_eq!(progress.counts(None).captured, 1);
        assert!(progress.logs.iter().any(|entry| entry.job_id == 1 && entry.message.as_ref() == "Scoped fixture action"));
    }
}
