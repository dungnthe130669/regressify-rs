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
                    credential_id: None,
                    environment: String::new(),
                    provider: Default::default(),
                    site_id: String::new(),
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
                    credential_id: Some(credential_id),
                    environment: String::new(),
                    provider: Default::default(),
                    site_id: String::new(),
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
    assert!(summary.contains("shop/html_report/index.html"));
    assert!(!summary.contains("account/html_report/index.html"));
    for name in [
        "shop/html_report/index.html",
        "shop/html_report/config.js",
        "shop/bitmaps_test/run-one/home__FINAL.png",
        "shop/bitmaps_reference/home__FINAL.png",
        "index_bundle.js",
        "styles.css",
        "script.js",
        "divergedWorker.js",
        "lato-regular-webfont.woff2",
        "staticwebapp.config.json",
    ] {
        assert!(
            staging.join(name).is_file(),
            "missing deployment asset: {name}"
        );
    }
    for name in [
        "account",
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
