use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use regressify::desktop::{
    runner::{RunCommand, RunOptions, RunRequest},
    worker::WORKER_FLAG,
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
        .write_all(&serde_json::to_vec(request).unwrap())
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
