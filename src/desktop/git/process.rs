use std::{
    ffi::OsStr,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::helpers::spawn_background_group as spawn_group;
use anyhow::{Context, Result, bail, ensure};

use super::Cancellation;

const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;

pub(super) struct Output {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl Output {
    pub fn text(&self) -> Result<String> {
        String::from_utf8(self.stdout.clone()).context("Git returned non-UTF-8 output")
    }
    pub fn ensure_success(&self) -> Result<()> {
        ensure!(self.success, "Git failed: {}", self.stderr.trim());
        Ok(())
    }
}

fn drain(mut reader: impl Read, maximum: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut output = Vec::new();
    let mut truncated = false;
    let mut buffer = [0; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let retained = count.min(maximum.saturating_sub(output.len()));
        output.extend_from_slice(&buffer[..retained]);
        truncated |= retained < count;
    }
    Ok((output, truncated))
}

pub(super) fn run(
    root: &Path,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    input: Option<Vec<u8>>,
    cancellation: &Cancellation,
    timeout: Duration,
) -> Result<Output> {
    ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
    let mut command = Command::new("git");
    for key in [
        "GIT_DIR",
        "GIT_COMMON_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_ASKPASS",
        "SSH_ASKPASS",
        "GIT_SSH_COMMAND",
        "GIT_SSH",
    ] {
        command.env_remove(key);
    }
    command
        .current_dir(root)
        .args([
            "-c",
            "color.ui=false",
            "-c",
            "credential.interactive=false",
            "-c",
            "core.askPass=",
            "-c",
            "protocol.ext.allow=never",
        ])
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env("GIT_SSH_VARIANT", "ssh")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_group(&mut command)
        .context("Cannot start Git; install Git and make it available on PATH")?;
    let stdout = child
        .inner()
        .stdout
        .take()
        .context("Git output pipe unavailable")?;
    let stderr = child
        .inner()
        .stderr
        .take()
        .context("Git error pipe unavailable")?;
    let stdin = child.inner().stdin.take();
    let writer = thread::spawn(move || -> std::io::Result<()> {
        if let Some(mut stdin) = stdin
            && let Some(input) = input
        {
            stdin.write_all(&input)?;
        }
        Ok(())
    });
    let output_reader = thread::spawn(move || drain(stdout, OUTPUT_LIMIT));
    let error_reader = thread::spawn(move || drain(stderr, 64 * 1024));
    let started = Instant::now();
    let result = loop {
        if cancellation.is_cancelled() || started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!(if cancellation.is_cancelled() {
                "Git operation cancelled"
            } else {
                "Git operation timed out; check credentials or complete it in VS Code"
            }));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.into());
            }
        }
    };
    let written = writer
        .join()
        .map_err(|_| anyhow::anyhow!("Git input task failed"))?;
    let (stdout, truncated) = output_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Git output task failed"))??;
    let (stderr, _) = error_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Git error task failed"))??;
    let status = result?;
    if status.success() {
        written?;
    }
    if truncated {
        bail!("Git output exceeds the 16 MiB review limit; narrow the repository in VS Code");
    }
    Ok(Output {
        success: status.success(),
        code: status.code(),
        stdout,
        stderr: redact(&String::from_utf8_lossy(&stderr)),
    })
}

fn redact(message: &str) -> String {
    message
        .split_inclusive(char::is_whitespace)
        .map(|word| {
            if word.contains("://") || word.contains("Authorization:") {
                "[remote] ".into()
            } else {
                word.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn grouped_git_commands_do_not_create_a_console_window() {
        if std::env::var_os("REGRESSIFY_GIT_NO_CONSOLE_FIXTURE").is_some() {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn GetConsoleWindow() -> *mut std::ffi::c_void;
            }
            assert!(
                unsafe { GetConsoleWindow() }.is_null(),
                "Git process group unexpectedly created a console"
            );
            return;
        }
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "desktop::git::process::tests::grouped_git_commands_do_not_create_a_console_window",
                "--nocapture",
            ])
            .env("REGRESSIFY_GIT_NO_CONSOLE_FIXTURE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = spawn_group(&mut command)
            .unwrap()
            .wait_with_output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn output_is_bounded_but_the_pipe_is_fully_drained() {
        let mut input = std::io::Cursor::new(vec![b'a'; 100]);
        let (bytes, truncated) = drain(&mut input, 10).unwrap();
        assert_eq!(bytes.len(), 10);
        assert!(truncated);
        assert_eq!(input.position(), 100);
        let message =
            redact("fatal: https://user:private@example.test/repository?token=value failed");
        assert!(!message.contains("private") && !message.contains("token=value"));
    }

    #[test]
    fn timeout_and_cancellation_stop_git_process_groups() {
        let directory = tempfile::tempdir().unwrap();
        for cancelled in [false, true] {
            let cancellation = Cancellation::default();
            let trigger = cancellation.clone();
            let thread = cancelled.then(|| {
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(100));
                    trigger.cancel();
                })
            });
            let started = Instant::now();
            let result = run(
                directory.path(),
                ["-c", "alias.fixture-wait=!sleep 30", "fixture-wait"],
                None,
                &cancellation,
                if cancelled {
                    Duration::from_secs(5)
                } else {
                    Duration::from_millis(200)
                },
            );
            if let Some(thread) = thread {
                thread.join().unwrap();
            }
            let error = result
                .err()
                .expect("waiting Git process must be stopped")
                .to_string();
            assert!(
                error.contains(if cancelled { "cancelled" } else { "timed out" }),
                "{error}"
            );
            assert!(started.elapsed() < Duration::from_secs(5));
        }
    }

    #[test]
    fn repository_environment_cannot_redirect_commands() {
        if let Some(root) = std::env::var_os("REGRESSIFY_GIT_ENV_FIXTURE") {
            let root = Path::new(&root);
            let output = run(
                root,
                ["rev-parse", "--show-toplevel"],
                None,
                &Cancellation::default(),
                Duration::from_secs(5),
            )
            .unwrap();
            output.ensure_success().unwrap();
            assert_eq!(
                std::path::PathBuf::from(output.text().unwrap().trim())
                    .canonicalize()
                    .unwrap(),
                root.canonicalize().unwrap()
            );
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        for directory in [root.path(), outside.path()] {
            run(
                directory,
                ["init"],
                None,
                &Cancellation::default(),
                Duration::from_secs(5),
            )
            .unwrap()
            .ensure_success()
            .unwrap();
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "desktop::git::process::tests::repository_environment_cannot_redirect_commands",
                "--nocapture",
            ])
            .env("REGRESSIFY_GIT_ENV_FIXTURE", root.path())
            .env("GIT_DIR", outside.path().join(".git"))
            .env("GIT_WORK_TREE", outside.path())
            .env("GIT_INDEX_FILE", outside.path().join("foreign-index"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(!outside.path().join("foreign-index").exists());
    }
}
