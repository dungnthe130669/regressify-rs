use std::{path::Path, time::Duration};

use anyhow::Result;
use chromiumoxide::Page;

use crate::{
    config::{self, ResolvedConfig},
    types::TestScenario,
};

pub(crate) struct DocumentHooks {
    marker: String,
    setup: String,
    script: Option<String>,
    timeout: Duration,
}

impl DocumentHooks {
    pub fn load(root: &Path, scenario: &TestScenario, config: &ResolvedConfig) -> Result<Self> {
        let read = |path: &str| -> Result<Option<String>> {
            config::resolve_optional_workspace_file(root, path, "document hook")?
                .map(std::fs::read_to_string)
                .transpose()
                .map_err(Into::into)
        };
        let mut css = if config.use_css_override {
            read(
                config
                    .css_override_path
                    .as_deref()
                    .unwrap_or("common/_override.css"),
            )?
            .unwrap_or_default()
        } else {
            String::new()
        };
        for (selectors, declaration) in [
            (&config.hide_selectors, "visibility: hidden !important"),
            (&config.remove_selectors, "display: none !important"),
        ] {
            for selector in selectors {
                css.push_str(&format!("\n{selector} {{ {declaration}; }}"));
            }
        }
        let payload = serde_json::json!({
            "requiredLogin": config.required_login,
            "scenario": { "id": scenario.id, "label": scenario.label,
                "description": scenario.description, "url": scenario.url,
                "requiredLogin": config.required_login }
        });
        let setup = format!(
            "(() => {{ window.__REGRESSIFY__ = {payload}; window.regressify = window.__REGRESSIFY__; \
             const style = document.createElement('style'); style.textContent = {}; \
             (document.head || document.documentElement).appendChild(style); }})()",
            serde_json::to_string(&css)?
        );
        Ok(Self {
            marker: format!("__regressify_hooks_{}", uuid::Uuid::new_v4().simple()),
            setup,
            script: read(
                config
                    .js_on_ready_path
                    .as_deref()
                    .unwrap_or("common/_on-ready.js"),
            )?,
            timeout: Duration::from_millis(config.element_timeout_ms),
        })
    }

    pub async fn apply(&self, page: &Page) -> Result<()> {
        tokio::time::timeout(self.timeout, async {
            super::page_stability::wait_for_dom_ready(page, self.timeout).await?;
            let marker = serde_json::to_string(&self.marker)?;
            let completed: bool = page
                .evaluate(format!("window[{marker}] === true"))
                .await?
                .into_value()?;
            if !completed {
                page.evaluate(self.setup.as_str()).await?;
                if let Some(script) = &self.script {
                    page.evaluate(script.as_str()).await?;
                }
                page.evaluate(format!("window[{marker}] = true")).await?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timed out applying document CSS/on-ready script"))??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires managed Chromium; run regressify install"]
    async fn hooks_follow_navigation_once_and_propagate_script_errors() -> Result<()> {
        let site = super::super::browser_tests::Site::new(|request| {
            let _ = request.respond(
                tiny_http::Response::from_string("<body><div class='dynamic'>data</div></body>")
                    .with_header(
                        tiny_http::Header::from_bytes("Content-Type", "text/html").unwrap(),
                    ),
            );
        })?;
        let workspace = tempfile::tempdir()?;
        std::fs::create_dir(workspace.path().join("common"))?;
        std::fs::write(
            workspace.path().join("common/_on-ready.js"),
            "(async () => { await Promise.resolve(); window.hookCalls = (window.hookCalls || 0) + 1; })()",
        )?;
        let config = ResolvedConfig {
            hide_selectors: vec![".dynamic".into()],
            ..Default::default()
        };
        let hooks = DocumentHooks::load(workspace.path(), &TestScenario::default(), &config)?;
        let (mut browser, handler, directory) =
            super::super::browser_tests::launch_browser("hooks", Default::default()).await?;
        let result: Result<()> = async {
            let page = browser.new_page(site.origin.as_str()).await?;
            for path in ["/first", "/next"] {
                page.goto(format!("{}{path}", site.origin)).await?;
                hooks.apply(&page).await?;
                hooks.apply(&page).await?;
                let valid: bool = page.evaluate("window.hookCalls === 1 && getComputedStyle(document.querySelector('.dynamic')).visibility === 'hidden'").await?.into_value()?;
                anyhow::ensure!(valid, "document hook missing or repeated after navigation");
            }
            std::fs::write(workspace.path().join("common/_on-ready.js"), "throw new Error('hook-fixture-error')")?;
            let failing = DocumentHooks::load(workspace.path(), &TestScenario::default(), &config)?;
            anyhow::ensure!(failing.apply(&page).await.is_err(), "hook errors must propagate");
            Ok(())
        }.await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }
}
