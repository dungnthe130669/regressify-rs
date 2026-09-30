use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use regressify::desktop::{
    runner::{RunCommand, RunOptions, RunRequest},
    worker::{WORKER_FLAG, WorkerRequest},
};

fn invoke(executable: &Path, request: &RunRequest) -> Output {
    invoke_with_events(executable, request, None)
}

#[cfg(windows)]
#[test]
fn snapshot_worker_reports_locked_output_path() {
    use std::os::windows::fs::OpenOptionsExt;

    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join(".regressify");
    std::fs::create_dir(&output).unwrap();
    let path = output.canonicalize().unwrap().join("index.html");
    std::fs::write(&path, b"private old report").unwrap();
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let request = RunRequest {
        root: root.path().into(),
        command: RunCommand::Snapshot,
        options: RunOptions::default(),
    };
    let result = invoke(executable, &request);
    drop(locked);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{stderr}");
    assert!(
        stderr.contains("Failed to read existing snapshot artifact"),
        "{stderr}"
    );
    assert!(stderr.contains(&path.display().to_string()), "{stderr}");
    assert!(stderr.contains("os error 32"), "{stderr}");
    assert!(!stderr.contains("private old report"), "{stderr}");
    assert_eq!(std::fs::read(&path).unwrap(), b"private old report");

    let result = invoke(executable, &request);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(windows)]
#[test]
#[ignore = "requires cached managed Chromium and no running installer"]
fn installer_browser_mode_reuses_cache_without_network_during_setup() {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::CreateMutexW,
    };

    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let chromiumoxide::fetcher::BrowserVersion::Revision(revision) =
        chromiumoxide::fetcher::BrowserVersion::current(
            chromiumoxide::fetcher::BrowserKind::Chromium,
        )
    else {
        panic!("Expected a pinned Chromium revision");
    };
    let platform = if cfg!(target_arch = "x86") {
        "win32"
    } else {
        "win64"
    };
    let browser = directories::BaseDirs::new()
        .unwrap()
        .cache_dir()
        .join(format!(
            "regressify/chromium/{platform}-{revision}/chrome-win/chrome.exe"
        ));
    let before =
        std::fs::metadata(&browser).expect("Run regressify install before this opt-in test");
    let workspace = tempfile::tempdir().unwrap();
    let name: Vec<u16> = "Local\\Regressify.Desktop.Setup.ED6F0908-7E11-4608-A532-E43482AA117B"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    assert!(!mutex.is_null());
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let _setup_mutex = unsafe { OwnedHandle::from_raw_handle(mutex) };
    assert!(
        !already_running,
        "Do not run this test during an actual installation"
    );

    let mut child = Command::new(executable)
        .arg("--install-chromium")
        .current_dir(workspace.path())
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .env("NO_PROXY", "")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Installer browser preparation did not finish from cache");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    let log = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{log}");
    assert!(
        log.contains("Reusing cached Chromium; no download needed"),
        "{log}"
    );
    assert!(log.contains("Chromium ready"), "{log}");
    assert!(!log.contains("Chromium download URL:"), "{log}");
    let after = std::fs::metadata(&browser).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);

    let blocked = invoke(
        executable,
        &RunRequest {
            root: workspace.path().to_path_buf(),
            command: RunCommand::Init,
            options: RunOptions::default(),
        },
    );
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("Regressify setup is running"));
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
}

#[test]
#[ignore = "requires managed Chromium; run regressify install"]
fn capture_workspaces_share_managed_chromium() {
    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let shared_cache = directories::BaseDirs::new()
        .unwrap()
        .cache_dir()
        .join("regressify/chromium");
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut selected_browser = None;
    let page = "data:text/html,<body style='background:rgb(24,128,100)'><div id='not-select' style='height:64px'>Managed Chromium</div></body>";
    let suite = serde_json::json!({
        "id": "managed",
        "postInteractionWait": 0,
        "scenarios": [
            {"id": "passing", "label": "passing", "url": page, "delay": 0, "noScrollTop": true},
            {"id": "failing", "label": "failing", "url": page, "delay": 0, "noScrollTop": true,
             "actions": [{"select": "#not-select", "value": "missing"}]}
        ]
    });
    for (index, workspace) in [&first, &second].into_iter().enumerate() {
        let suites = workspace.path().join("visual_tests");
        std::fs::create_dir_all(&suites).unwrap();
        std::fs::write(
            suites.join("managed.tests.json"),
            serde_json::to_vec(&suite).unwrap(),
        )
        .unwrap();
        for (command, run_id) in [
            (RunCommand::Reference, "shared-reference"),
            (RunCommand::Test, "shared-test"),
        ] {
            let request = RunRequest {
                root: workspace.path().into(),
                command,
                options: RunOptions {
                    suites: vec!["managed".into()],
                    run_id: run_id.into(),
                    fetch_browser: (index == 1).then_some(false),
                    async_capture_limit: 1,
                    async_capture_ramp_ms: 0,
                    ..Default::default()
                },
            };
            let events = tempfile::NamedTempFile::new().unwrap();
            let output = invoke_with_events(executable, &request, Some(events.path()));
            let log = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "{log}");
            let mut phases = Vec::new();
            let mut reader = regressify::progress::EventReader::open(events.path()).unwrap();
            reader
                .drain(|record| {
                    if let regressify::progress::Event::Phase { name } = record.event {
                        phases.push(name);
                    }
                })
                .unwrap();
            reader.complete().unwrap();
            let ready = phases
                .iter()
                .position(|phase| phase == "Chromium ready")
                .unwrap();
            let capture = phases
                .iter()
                .position(|phase| phase == "Capturing scenarios")
                .unwrap();
            assert!(ready < capture, "{phases:?}");
            assert!(log.contains("Checking Chromium cache"), "{log}");
            assert!(log.contains("Shared Chromium cache:"), "{log}");
            let browser = log
                .lines()
                .find_map(|line| line.strip_prefix("Managed Chromium executable: "))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| panic!("Missing managed browser path: {log}"));
            assert!(browser.starts_with(&shared_cache), "{}", browser.display());
            assert!(browser.is_file());
            assert_eq!(selected_browser.get_or_insert(browser.clone()), &browser);
            assert!(!workspace.path().join("download").exists());
        }
        let artifacts = workspace.path().join(".regressify/managed");
        let references: Vec<_> = std::fs::read_dir(artifacts.join("bitmaps_reference"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
            .collect();
        assert!(!references.is_empty());
        assert!(references.iter().all(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("passing")
        }));
        assert!(
            walkdir::WalkDir::new(artifacts.join("bitmaps_test/shared-test"))
                .into_iter()
                .map(|entry| entry.unwrap())
                .any(|entry| entry.file_name().to_string_lossy().ends_with(".error.json"))
        );
    }
}

#[test]
fn deploy_missing_token_fails_before_snapshot_modifies_output() {
    use regressify::desktop::deployment::{DeployTarget, DeploymentJob};
    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join(".regressify");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("index.html"), "untouched summary").unwrap();
    let request = RunRequest {
        root: root.path().into(),
        command: RunCommand::Deploy,
        options: RunOptions {
            suites: vec!["shop".into()],
            deployment: Some(DeploymentJob {
                target: DeployTarget {
                    id: "test".into(),
                    name: "Test".into(),
                    published_url: None,
                    credential_id: None,
                    named_token_id: None,
                    environment: String::new(),
                    provider: Default::default(),
                    site_id: String::new(),
                    cloudflare: None,
                },
                suite_ids: vec!["shop".into()],
                catalog_ids: vec!["shop".into()],
                partial_confirmed: false,
            }),
            ..Default::default()
        },
    };
    let result = invoke(executable, &request);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("No saved deployment token"));
    assert_eq!(
        std::fs::read_to_string(output.join("index.html")).unwrap(),
        "untouched summary"
    );
}

#[cfg(windows)]
#[test]
fn deploy_worker_reads_machine_local_credentials_without_exposing_tokens() {
    use regressify::desktop::deployment::{
        DeployTarget, DeployTargets, DeploymentJob, credentials::DeploymentCredential,
    };
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
    };

    struct Cleanup(DeploymentCredential);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.delete().expect("Remove synthetic test credential");
        }
    }

    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let root = tempfile::tempdir().unwrap();
    let credential_id = uuid::Uuid::new_v4().to_string();
    let credential = Cleanup(DeploymentCredential::new(root.path(), &credential_id).unwrap());
    let token = format!("synthetic-invalid-token-{}", uuid::Uuid::new_v4());
    credential.0.write(&token).unwrap();
    assert_eq!(credential.0.read().unwrap().as_str(), token);

    let settings_path = DeployTargets::path(root.path()).unwrap();
    let workspace_key = settings_path.file_stem().unwrap().to_str().unwrap();
    let target_name = format!("{workspace_key}:{credential_id}.Regressify Desktop Deployment")
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut stored: *mut CREDENTIALW = std::ptr::null_mut();
    assert_ne!(
        unsafe { CredReadW(target_name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut stored) },
        0
    );
    let persistence = unsafe { (*stored).Persist };
    unsafe { CredFree(stored.cast()) };
    assert_eq!(persistence, CRED_PERSIST_LOCAL_MACHINE);

    let output = root.path().join(".regressify");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("index.html"), "untouched summary").unwrap();
    let events = root.path().join("events.jsonl");
    std::fs::write(&events, []).unwrap();
    let request = RunRequest {
        root: root.path().into(),
        command: RunCommand::Deploy,
        options: RunOptions {
            suites: vec!["shop".into()],
            deployment: Some(DeploymentJob {
                target: DeployTarget {
                    id: "test".into(),
                    name: "Test".into(),
                    published_url: None,
                    credential_id: Some(credential_id),
                    named_token_id: None,
                    environment: String::new(),
                    provider: Default::default(),
                    site_id: String::new(),
                    cloudflare: None,
                },
                suite_ids: vec!["shop".into()],
                catalog_ids: vec!["shop".into()],
                partial_confirmed: false,
            }),
            ..Default::default()
        },
    };
    assert!(!serde_json::to_string(&request).unwrap().contains(&token));
    let result = invoke_with_events(executable, &request, Some(&events));
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("Invalid Azure deployment token format"),
        "Unexpected worker error: {}",
        String::from_utf8_lossy(&result.stderr).replace(&token, "[REDACTED]")
    );
    assert!(!String::from_utf8_lossy(&result.stderr).contains(&token));
    assert!(!String::from_utf8_lossy(&result.stdout).contains(&token));
    assert!(!std::fs::read_to_string(events).unwrap().contains(&token));
    assert_eq!(
        std::fs::read_to_string(output.join("index.html")).unwrap(),
        "untouched summary"
    );
    let netlify_token = format!("synthetic invalid Netlify token {}", uuid::Uuid::new_v4());
    credential.0.write(&netlify_token).unwrap();
    let mut netlify_request = request;
    let target = &mut netlify_request.options.deployment.as_mut().unwrap().target;
    target.provider = regressify::desktop::deployment::DeploymentProvider::Netlify;
    target.site_id = uuid::Uuid::new_v4().to_string();
    let result = invoke(executable, &netlify_request);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("valid Netlify personal access token")
    );
    assert!(!String::from_utf8_lossy(&result.stderr).contains(&netlify_token));
    assert!(!String::from_utf8_lossy(&result.stdout).contains(&netlify_token));
    assert!(
        !serde_json::to_string(&netlify_request)
            .unwrap()
            .contains(&netlify_token)
    );
    assert_eq!(
        std::fs::read_to_string(output.join("index.html")).unwrap(),
        "untouched summary"
    );
    credential.0.delete().unwrap();
    assert!(credential.0.read_optional().unwrap().is_none());
}

fn invoke_with_events(
    executable: &Path,
    request: &RunRequest,
    events_path: Option<&Path>,
) -> Output {
    let scratch = (request.command == RunCommand::Deploy).then(|| tempfile::tempdir().unwrap());
    let mut child = Command::new(executable)
        .arg(WORKER_FLAG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            &serde_json::to_vec(&WorkerRequest {
                run: request.clone(),
                events_path: events_path.map(Path::to_path_buf),
                scratch_directory: scratch
                    .as_ref()
                    .map(|directory| directory.path().to_path_buf()),
            })
            .unwrap(),
        )
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn duplicate_workspace_launches_activate_the_owner_without_starting_another_gui() {
    use regressify::desktop::{
        instance::{DesktopInstance, OpenInstance},
        workspace::Preferences,
    };
    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let workspace = tempfile::tempdir().unwrap();
    let locks = Preferences::path().unwrap().with_file_name("instances");
    let owner = match DesktopInstance::open(&locks, workspace.path()).unwrap() {
        OpenInstance::Owned(instance) => instance,
        OpenInstance::Activated => panic!("Expected isolated workspace ownership"),
    };
    for path in [workspace.path().to_path_buf(), workspace.path().join(".")] {
        let mut child = Command::new(executable)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("Duplicate launcher did not hand off the workspace");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(owner.take_activation());
    }
    let request = RunRequest {
        root: workspace.path().into(),
        command: RunCommand::Init,
        options: RunOptions::default(),
    };
    let output = invoke(executable, &request);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!owner.take_activation());
    assert!(workspace.path().join("visual_tests").is_dir());
}

#[test]
fn standalone_desktop_runs_shared_engine_without_a_cli_binary() {
    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let installation = tempfile::tempdir_in(executable.parent().unwrap()).unwrap();
    let standalone = installation.path().join(executable.file_name().unwrap());
    std::fs::hard_link(executable, &standalone).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let request = RunRequest {
        root: workspace.path().into(),
        command: RunCommand::Init,
        options: RunOptions::default(),
    };
    let output = invoke(&standalone, &request);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(workspace.path().join("common").is_dir());
    let sentinel = workspace.path().join("visual_tests/basic.tests.yaml");
    std::fs::write(&sentinel, "# keep QA content\nscenarios: []\n").unwrap();
    assert!(invoke(&standalone, &request).status.success());
    assert_eq!(
        std::fs::read_to_string(sentinel).unwrap(),
        "# keep QA content\nscenarios: []\n"
    );
    assert_eq!(std::fs::read_dir(installation.path()).unwrap().count(), 1);
    let mut invalid = request;
    invalid.options.async_capture_limit = 0;
    assert!(!invoke(&standalone, &invalid).status.success());
}

#[test]
fn bundled_worker_reports_live_outcomes_for_multiple_suites() {
    let workspace = tempfile::tempdir().unwrap();
    for (suite, changed) in [("shop", false), ("account", true)] {
        let root = workspace.path().join(".regressify").join(suite);
        let reference = root.join("bitmaps_reference");
        let captures = root.join("bitmaps_test/run-1");
        std::fs::create_dir_all(&reference).unwrap();
        std::fs::create_dir_all(&captures).unwrap();
        lodepng::encode32_file(
            reference.join("home__FINAL.png"),
            &[255_u8, 255, 255, 255],
            1,
            1,
        )
        .unwrap();
        let rgba = if changed {
            [0_u8, 0, 0, 255]
        } else {
            [255_u8; 4]
        };
        lodepng::encode32_file(captures.join("home__FINAL.png"), &rgba, 1, 1).unwrap();
    }
    let events = tempfile::NamedTempFile::new().unwrap();
    let request = RunRequest {
        root: workspace.path().into(),
        command: RunCommand::Compare,
        options: RunOptions {
            suites: vec!["shop".into(), "account".into()],
            ..Default::default()
        },
    };
    let output = invoke_with_events(
        Path::new(env!("CARGO_BIN_EXE_regressify-desktop")),
        &request,
        Some(events.path()),
    );
    assert!(
        output.status.success(),
        "Visual mismatch must not fail the worker: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut reader = regressify::progress::EventReader::open(events.path()).unwrap();
    let mut live = regressify::progress::LiveProgress::default();
    reader.drain(|record| live.apply(record)).unwrap();
    reader.complete().unwrap();
    assert!(live.finished);
    assert!(live.error.is_none());
    assert_eq!(live.suites.len(), 2);
    assert_eq!(live.counts(Some("shop")).passed, 1);
    assert_eq!(live.counts(Some("account")).failed, 1);
}

#[test]
fn mixed_report_outcomes_match_html_saved_results_and_live_totals() {
    let workspace = tempfile::tempdir().unwrap();
    let output = workspace.path().join(".regressify");
    let suite = output.join("alloy");
    let reference = suite.join("bitmaps_reference");
    let captures = suite.join("bitmaps_test/mixed-run");
    std::fs::create_dir_all(&reference).unwrap();
    std::fs::create_dir_all(&captures).unwrap();
    let image = lodepng::encode32(&[255_u8; 4], 1, 1).unwrap();
    let mut metadata = serde_json::Map::new();
    for scenario in 0..4 {
        for viewport in ["phone", "tablet", "desktop"] {
            let scenario_id = format!("scenario-{scenario}");
            let key = format!("{scenario_id}_{viewport}__FINAL");
            let filename = format!("{key}.png");
            std::fs::write(reference.join(&filename), &image).unwrap();
            std::fs::write(captures.join(&filename), &image).unwrap();
            if scenario == 3 {
                std::fs::write(captures.join(format!("{filename}.error.json")),
                    serde_json::to_vec(&serde_json::json!({"error": true, "message": "Timed out waiting for missing selector"})).unwrap()).unwrap();
            }
            metadata.insert(
                key,
                serde_json::json!({
                    "scenarioId": scenario_id, "scenarioLabel": format!("Scenario {scenario}"),
                    "referenceUrl": "https://example.com", "url": "https://example.com"
                }),
            );
        }
    }
    std::fs::write(
        captures.join(".report-urls.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 1, "captures": metadata
        }))
        .unwrap(),
    )
    .unwrap();
    let events = tempfile::NamedTempFile::new().unwrap();
    let request = RunRequest {
        root: workspace.path().into(),
        command: RunCommand::Compare,
        options: RunOptions {
            suites: vec!["alloy".into()],
            run_id: "mixed-run".into(),
            ..Default::default()
        },
    };
    let result = invoke_with_events(
        Path::new(env!("CARGO_BIN_EXE_regressify-desktop")),
        &request,
        Some(events.path()),
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(captures.join("report.json")).unwrap()).unwrap();
    let html_config = std::fs::read_to_string(suite.join("html_report/config.js")).unwrap();
    let html: serde_json::Value = serde_json::from_str(
        html_config
            .strip_prefix("report(")
            .unwrap()
            .strip_suffix(");")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(html, saved);
    let catalog = regressify::desktop::results::Catalog::load(&output).unwrap();
    assert!(catalog.warnings.is_empty());
    assert_eq!(catalog.reports[0].counts(), (9, 0, 3));
    let mut reader = regressify::progress::EventReader::open(events.path()).unwrap();
    let mut live = regressify::progress::LiveProgress::default();
    reader.drain(|record| live.apply(record)).unwrap();
    reader.complete().unwrap();
    assert!(live.finished && live.error.is_none());
    assert_eq!(live.counts(None).total, 4);
    let comparisons = live.comparison_counts(None);
    assert_eq!(
        (comparisons.total, comparisons.passed, comparisons.failed),
        (12, 9, 3)
    );
}

#[test]
fn filtered_snapshot_stages_only_selected_suites_and_their_public_dependencies() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("regressify.yaml"),
        "snapshotTitle: Shop Visual Test Report\nsnapshotHeading: Shop Visual Tests Summary\nsnapshotSubheading: Compare [Integration](https://integration.example.com/en) and [Preproduction](https://preproduction.example.com/en)\n",
    )
    .unwrap();
    let output = workspace.path().join(".regressify");
    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let image = lodepng::encode32(&[20_u8, 40, 60, 255], 1, 1).unwrap();
    for suite in ["shop", "account"] {
        let baseline = output.join(suite).join("bitmaps_reference");
        let capture = output.join(suite).join("bitmaps_test/run-one");
        std::fs::create_dir_all(&baseline).unwrap();
        std::fs::create_dir_all(&capture).unwrap();
        std::fs::write(baseline.join("home__FINAL.png"), &image).unwrap();
        std::fs::write(capture.join("home__FINAL.png"), &image).unwrap();
    }
    let mut request = RunRequest {
        root: workspace.path().into(),
        command: RunCommand::Compare,
        options: RunOptions {
            suites: vec!["shop".into(), "account".into()],
            ..Default::default()
        },
    };
    let compared = invoke(executable, &request);
    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    std::fs::create_dir_all(output.join("states")).unwrap();
    std::fs::write(output.join("states/private.json"), "private").unwrap();
    std::fs::write(
        output.join("shop/bitmaps_test/run-one/unreferenced.png"),
        &image,
    )
    .unwrap();
    let sibling = std::fs::read(output.join("account/html_report/config.js")).unwrap();
    request.command = RunCommand::Snapshot;
    request.options.suites = vec!["shop".into()];
    let snapshot = invoke(executable, &request);
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    assert_eq!(
        std::fs::read(output.join("account/html_report/config.js")).unwrap(),
        sibling
    );
    let staging = workspace.path().join("staging");
    regressify::desktop::deployment::stage_reports(&output, &["shop".into()], &staging).unwrap();
    let summary = std::fs::read_to_string(staging.join("index.html")).unwrap();
    assert!(summary.contains("<title>Shop Visual Test Report</title>"));
    let manifest_path = summary
        .split_once("name=\"regressify-manifest\" content=\"")
        .unwrap()
        .1
        .split_once('"')
        .unwrap()
        .0;
    let read_resource = |url: &str| {
        let (path, version) = url
            .split_once("?v=")
            .expect("Every report resource needs a version");
        let path = percent_encoding::percent_decode_str(path)
            .decode_utf8()
            .unwrap();
        let bytes = std::fs::read(staging.join(path.as_ref())).unwrap();
        assert_eq!(version, regressify::snapshot::get_asset_version(&bytes, 20));
        bytes
    };
    let manifest: serde_json::Value =
        serde_json::from_slice(&read_resource(manifest_path)).unwrap();
    assert_eq!(manifest["heading"], "Shop Visual Tests Summary");
    assert_eq!(
        manifest["subheading"],
        "Compare [Integration](https://integration.example.com/en) and [Preproduction](https://preproduction.example.com/en)"
    );
    assert_eq!(manifest["suites"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["suites"][0]["id"], "shop");
    let report: serde_json::Value = serde_json::from_slice(&read_resource(
        manifest["suites"][0]["report"].as_str().unwrap(),
    ))
    .unwrap();
    let capture = &report["tests"][0];
    assert_ne!(capture["reference"], capture["test"]);
    assert_eq!(read_resource(capture["test"].as_str().unwrap()), image);
    assert_eq!(read_resource(capture["reference"].as_str().unwrap()), image);
    assert_eq!(
        walkdir::WalkDir::new(staging.join("_snapshot/images"))
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .count(),
        2
    );
    assert!(!summary.contains("html_report"));
    for name in [
        "index.html",
        "_headers",
        "_redirects",
        "staticwebapp.config.json",
    ] {
        assert!(
            staging.join(name).is_file(),
            "missing deployment asset: {name}"
        );
    }
    for name in [
        "account",
        "shop",
        "states",
        "shop/bitmaps_test/run-one/unreferenced.png",
        "shop/bitmaps_test/run-one/report.json",
    ] {
        assert!(
            !staging.join(name).exists(),
            "private or unselected artifact was staged: {name}"
        );
    }
}

#[test]
fn bundled_worker_scopes_scenarios_and_preflights_all_approvals() {
    let workspace = tempfile::tempdir().unwrap();
    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let reference = lodepng::encode32(&[10_u8, 20, 30, 255], 1, 1).unwrap();
    let current = lodepng::encode32(&[200_u8, 20, 30, 255], 1, 1).unwrap();
    for suite in ["shop", "account"] {
        let output = workspace.path().join(".regressify").join(suite);
        let baseline = output.join("bitmaps_reference");
        let run = output.join("bitmaps_test/run-1");
        std::fs::create_dir_all(&baseline).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        let mut metadata = serde_json::Map::new();
        for scenario in ["home", "home-extra"] {
            let key = format!("{scenario}__FINAL");
            std::fs::write(baseline.join(format!("{key}.png")), &reference).unwrap();
            std::fs::write(run.join(format!("{key}.png")), &current).unwrap();
            metadata.insert(
                key,
                serde_json::json!({
                    "scenarioId": scenario, "scenarioLabel": scenario,
                    "referenceUrl": "https://example.com", "url": "https://example.com"
                }),
            );
        }
        std::fs::write(
            run.join(".report-urls.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1, "captures": metadata
            }))
            .unwrap(),
        )
        .unwrap();
    }
    let mut request = RunRequest {
        root: workspace.path().into(),
        command: RunCommand::Approve,
        options: RunOptions {
            suites: vec!["shop".into(), "account".into()],
            scenarios: vec![
                "shop:home".parse().unwrap(),
                "account:missing".parse().unwrap(),
            ],
            run_id: "run-1".into(),
            ..Default::default()
        },
    };
    let result = invoke(executable, &request);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing"));
    let shop = workspace.path().join(".regressify/shop");
    assert_eq!(
        std::fs::read(shop.join("bitmaps_reference/home__FINAL.png")).unwrap(),
        reference
    );
    request.options.scenarios.pop();
    let result = invoke(executable, &request);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        std::fs::read(shop.join("bitmaps_reference/home__FINAL.png")).unwrap(),
        current
    );
    assert_eq!(
        std::fs::read(shop.join("bitmaps_reference/home-extra__FINAL.png")).unwrap(),
        reference
    );
    assert_eq!(
        std::fs::read(
            workspace
                .path()
                .join(".regressify/account/bitmaps_reference/home__FINAL.png")
        )
        .unwrap(),
        reference
    );
    request.command = RunCommand::Compare;
    let result = invoke(executable, &request);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(shop.join("bitmaps_test/run-1/report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["tests"].as_array().unwrap().len(), 1);
    assert_eq!(report["tests"][0]["pair"]["scenarioId"], "home");
}

#[test]
fn bundled_worker_approves_one_report_viewport_and_recalculates_full_scope() {
    let workspace = tempfile::tempdir().unwrap();
    let executable = Path::new(env!("CARGO_BIN_EXE_regressify-desktop"));
    let output = workspace.path().join("review-output/shop");
    let baseline = output.join("bitmaps_reference");
    std::fs::create_dir_all(&baseline).unwrap();
    let reference = lodepng::encode32(&[10_u8, 20, 30, 255], 1, 1).unwrap();
    let current = lodepng::encode32(&[200_u8, 20, 30, 255], 1, 1).unwrap();
    let newer = lodepng::encode32(&[70_u8, 20, 30, 255], 1, 1).unwrap();
    for (run_id, png) in [("reviewed-run", &current), ("zz-newer-run", &newer)] {
        let run = output.join("bitmaps_test").join(run_id);
        std::fs::create_dir_all(&run).unwrap();
        let mut metadata = serde_json::Map::new();
        for (key, scenario) in [
            ("home_desktop__FINAL", "home"),
            ("home_desktop__$saved", "home"),
            ("home_mobile__FINAL", "home"),
            ("other_desktop__FINAL", "other"),
            ("hidden_desktop__FINAL", "hidden"),
        ] {
            std::fs::write(baseline.join(format!("{key}.png")), &reference).unwrap();
            std::fs::write(run.join(format!("{key}.png")), png).unwrap();
            metadata.insert(key.into(), serde_json::json!({
                "scenarioId": scenario, "scenarioLabel": scenario,
                "referenceUrl": "https://production.example.test", "url": "https://staging.example.test",
            }));
        }
        std::fs::write(
            run.join(".report-urls.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1, "captures": metadata,
            }))
            .unwrap(),
        )
        .unwrap();
    }
    let mut request = RunRequest {
        root: workspace.path().into(),
        command: RunCommand::Compare,
        options: RunOptions {
            regressify_dir: "review-output".into(),
            run_id: "reviewed-run".into(),
            suites: vec!["shop".into()],
            scenarios: vec!["shop:home".parse().unwrap(), "shop:other".parse().unwrap()],
            ..Default::default()
        },
    };
    let result = invoke(executable, &request);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    request.command = RunCommand::Approve;
    request.options.scenarios.truncate(1);
    request.options.report_capture = Some("home_desktop__FINAL".into());
    let result = invoke(executable, &request);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for key in ["home_desktop__FINAL", "home_desktop__$saved"] {
        assert_eq!(
            std::fs::read(baseline.join(format!("{key}.png"))).unwrap(),
            current
        );
    }
    for key in [
        "home_mobile__FINAL",
        "other_desktop__FINAL",
        "hidden_desktop__FINAL",
    ] {
        assert_eq!(
            std::fs::read(baseline.join(format!("{key}.png"))).unwrap(),
            reference
        );
    }
    let catalog =
        regressify::desktop::results::Catalog::load(&workspace.path().join("review-output"))
            .unwrap();
    assert_eq!(catalog.reports.len(), 1);
    assert_eq!(catalog.reports[0].run_id, "reviewed-run");
    assert_eq!(catalog.reports[0].counts(), (2, 2, 0));
    assert_eq!(catalog.reports[0].report.tests.len(), 4);
    assert!(
        !output
            .join("bitmaps_test/zz-newer-run/report.json")
            .exists()
    );
    assert!(!workspace.path().join(".regressify").exists());
}
