use std::{path::Path, process::Stdio, time::Duration};

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) fn validate(root: &Path, script: &str) -> Result<()> {
    crate::config::resolve_workspace_file(root, script, "Node hook")?;
    let status = std::process::Command::new("node")
        .current_dir(root)
        .args([
            "-e",
            "try { require.resolve('playwright'); } catch { require.resolve('playwright-core'); }",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    anyhow::ensure!(
        status.is_ok_and(|status| status.success()),
        "Node hooks require Node.js and a workspace installation of playwright or playwright-core (npm install --save-dev playwright-core)"
    );
    Ok(())
}

pub(crate) async fn run(root: &Path, payload: serde_json::Value, timeout: Duration) -> Result<()> {
    tokio::time::timeout(timeout, async {
        let mut process = tokio::process::Command::new("node")
            .current_dir(root)
            .args(["-e", include_str!("node_hook.cjs")])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("Cannot launch Node lifecycle hook")?;
        let mut stdin = process
            .stdin
            .take()
            .context("Node hook stdin unavailable")?;
        let mut stderr = process
            .stderr
            .take()
            .context("Node hook stderr unavailable")?;
        let read = async {
            let mut output = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let count = stderr.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                let retained = count.min(16_384usize.saturating_sub(output.len()));
                output.extend_from_slice(&buffer[..retained]);
            }
            Ok::<_, std::io::Error>(output)
        };
        let execute = async {
            stdin.write_all(&serde_json::to_vec(&payload)?).await?;
            drop(stdin);
            Ok::<_, anyhow::Error>(process.wait().await?)
        };
        let (status, stderr) =
            tokio::try_join!(execute, async { read.await.map_err(anyhow::Error::from) })?;
        anyhow::ensure!(
            status.success(),
            "Node lifecycle hook failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("Node lifecycle hook exceeded its timeout")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires regressify install and npm install --prefix target/node-hooks playwright-core@1.59.1"]
    async fn node_hook_attaches_without_closing_browser_and_is_bounded() -> Result<()> {
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/node-hooks");
        let workspace = tempfile::tempdir_in(packages)?;
        let hook = workspace.path().join("hook.cjs");
        std::fs::write(
            &hook,
            "module.exports = async (page, scenario, viewport, isReference) => { await page.evaluate(label => document.body.textContent = label, scenario.label); };",
        )?;
        validate(workspace.path(), "hook.cjs")?;
        let (mut browser, handler, directory) =
            super::super::browser_tests::launch_browser("node-hook", Default::default()).await?;
        let result: Result<()> = async {
            let page = browser.new_page("about:blank").await?;
            let payload = serde_json::json!({"script": hook, "endpoint": browser.websocket_address(),
                "target": page.target_id(), "scenario": {"label": "hook completed"}, "isReference": true});
            run(workspace.path(), payload.clone(), Duration::from_secs(10)).await?;
            let text: String = page.evaluate("document.body.textContent").await?.into_value()?;
            anyhow::ensure!(text == "hook completed", "Node hook did not execute on the scenario page");
            std::fs::write(&hook, "module.exports = async () => await new Promise(() => {});")?;
            anyhow::ensure!(run(workspace.path(), payload, Duration::from_secs(2)).await.is_err(), "unbounded hook");
            page.evaluate("document.body.textContent = 'browser remains available'").await?;
            Ok(())
        }.await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }
}
