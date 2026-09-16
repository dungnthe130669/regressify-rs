//! The desktop executable doubles as its isolated, headless engine worker.

use std::io::Read;

use anyhow::{Context, Result, ensure};

use super::runner::RunRequest;

pub const WORKER_FLAG: &str = "--internal-worker";
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;

pub fn read_request(input: impl Read) -> Result<RunRequest> {
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
    let cli = request.cli()?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(crate::application::execute(cli))
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
        let encoded = serde_json::to_vec(&request).unwrap();
        let decoded = read_request(encoded.as_slice()).unwrap();
        assert_eq!(decoded.root, request.root);
        assert_eq!(decoded.command, request.command);
        assert!(read_request(b"not JSON".as_slice()).is_err());
        assert!(read_request(vec![b' '; MAX_REQUEST_BYTES + 1].as_slice()).is_err());
    }
}
