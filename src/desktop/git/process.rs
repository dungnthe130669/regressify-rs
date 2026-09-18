use std::{
    ffi::OsStr,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use command_group::CommandGroup;

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
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .group_spawn()
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
