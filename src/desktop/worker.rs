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
                    Some((
                        job,
                        super::deployment::swa::SwaClient::new(&job.target.token()?)?,
                    ))
                } else {
                    None
                };
                crate::application::execute(cli).await?;
                if let Some((job, client)) = deployment {
                    let directory = tempfile::tempdir()?;
                    let output = request.run.options.output_directory(&request.run.root);
                    let staging = directory.path().join("report");
                    super::deployment::stage_reports(&output, &job.suite_ids, &staging)?;
                    client.deploy(&staging, &job.target).await?;
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
        })
        .unwrap();
        let decoded = read_request(encoded.as_slice()).unwrap();
        assert_eq!(decoded.run.root, request.root);
        assert_eq!(decoded.run.command, request.command);
        assert!(read_request(b"not JSON".as_slice()).is_err());
        assert!(read_request(vec![b' '; MAX_REQUEST_BYTES + 1].as_slice()).is_err());
    }
}
