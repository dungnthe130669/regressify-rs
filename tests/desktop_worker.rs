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
                events_path: None,
            })
            .unwrap(),
        )
        .unwrap();
    child.wait_with_output().unwrap()
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_regressify-desktop"))
        .arg(WORKER_FLAG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let payload = WorkerRequest {
        run: request,
        events_path: Some(events.path().into()),
    };
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&payload).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
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
