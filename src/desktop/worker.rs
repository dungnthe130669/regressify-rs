//! The desktop executable doubles as its isolated, headless engine worker.

use std::{io::Read, path::PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::runner::RunRequest;

pub const WORKER_FLAG: &str = "--internal-worker";
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerRequest {
    pub run: RunRequest,
    pub events_path: Option<PathBuf>,
    #[serde(default)]
    pub scratch_directory: Option<PathBuf>,
}

pub fn read_request(input: impl Read) -> Result<WorkerRequest> {
    let mut bytes = Vec::new();
    input
        .take(MAX_REQUEST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_REQUEST_BYTES,
        "Run request exceeds 1 MiB"
    );
    serde_json::from_slice(&bytes).context("Invalid desktop run request")
}

pub fn run_stdio() -> Result<()> {
    let request = read_request(std::io::stdin().lock())?;
    if let Some(path) = &request.events_path {
        crate::progress::install(path)?;
    }
    let result = (|| -> Result<()> {
        let cli = request.run.cli()?;
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let deployment = if request.run.command == super::runner::RunCommand::Deploy {
                    let job = request
                        .run
                        .options
                        .deployment
                        .as_ref()
                        .context("Deployment settings are missing")?;
                    job.validate()?;
                    let directory = request
                        .scratch_directory
                        .as_ref()
                        .context("Deployment staging directory is missing")?;
                    ensure!(
                        directory.is_absolute()
                            && directory.is_dir()
                            && !std::fs::symlink_metadata(directory)?
                                .file_type()
                                .is_symlink()
                            && std::fs::read_dir(directory)?.next().is_none(),
                        "Deployment staging directory must be a new empty directory"
                    );
                    Some((
                        job,
                        super::deployment::DeploymentClient::new(
                            &job.target,
                            &job.target.token(&request.run.root)?,
                        )?,
                    ))
                } else {
                    None
                };
                if deployment.is_some() {
                    super::deployment::with_heartbeat(
                        "Generating selected report snapshots",
                        crate::application::execute(cli),
                    )
                    .await?;
                } else {
                    crate::application::execute(cli).await?;
                }
                if let Some((job, client)) = deployment {
                    let directory = request
                        .scratch_directory
                        .as_ref()
                        .context("Deployment staging directory is missing")?;
                    let output = request.run.options.output_directory(&request.run.root);
                    let staging = directory.join("report");
                    let destination = staging.clone();
                    let suites = job.suite_ids.clone();
                    super::deployment::with_heartbeat(
                        "Staging selected reports",
                        tokio::task::spawn_blocking(move || {
                            super::deployment::stage_reports(&output, &suites, &destination)
                        }),
                    )
                    .await??;
                    let url = client.deploy(&staging, &job.target).await?;
                    crate::progress::emit(crate::progress::Event::Deployed {
                        target: job.target.name.clone(),
                        url,
                    });
                }
                Ok(())
            })
    })();
    crate::progress::emit(crate::progress::Event::Finished {
        error: result.as_ref().err().map(|error| format!("{error:#}")),
    });
    crate::progress::finish()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::runner::{RunCommand, RunOptions};

    #[test]
    fn worker_request_round_trip_and_limits() {
        let request = RunRequest {
            root: "QA workspace".into(),
            command: RunCommand::Init,
            options: RunOptions::default(),
        };
        let encoded = serde_json::to_vec(&WorkerRequest {
            run: request.clone(),
            events_path: None,
            scratch_directory: None,
        })
        .unwrap();
        let decoded = read_request(encoded.as_slice()).unwrap();
        assert_eq!(decoded.run.root, request.root);
        assert_eq!(decoded.run.command, request.command);
        assert!(read_request(b"not JSON".as_slice()).is_err());
        assert!(read_request(vec![b' '; MAX_REQUEST_BYTES + 1].as_slice()).is_err());
    }
}
