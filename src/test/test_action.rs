use anyhow::{Result, anyhow};
use chromiumoxide::{
    Page, cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams,
};

use crate::types::{
    ClickAction, SaveAction, TestAction, TestScenario, TestSuite, Wait, WaitAction,
};

pub async fn exec_test_action(
    page: &Page,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    action: &TestAction,
) -> Result<()> {
    match action {
        TestAction::Click(ClickAction { click }) => {
            page.find_element(click).await?.click().await?;
        }

        TestAction::Save(SaveAction { save }) => {
            let test_suite_id = test_suite
                .id
                .as_ref()
                .ok_or_else(|| anyhow!("Test suite ID is required for Save action"))?;

            let scenario_id = scenario
                .id
                .as_ref()
                .ok_or_else(|| anyhow!("Test scenario ID is required for Save action"))?;

            page.save_screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .full_page(true)
                    .omit_background(true)
                    .build(),
                format!("{}{}{}.png", test_suite_id, scenario_id, save),
            )
            .await?;
        }

        TestAction::Wait(WaitAction { wait, url }) => {
            if let Some(url) = url {
                // Wait for navigation and ensure the URL matches
                page.wait_for_navigation().await?;
                let current_url = page
                    .url()
                    .await?
                    .ok_or_else(|| anyhow!("Failed to get current URL"))?;
                if &current_url != url {
                    anyhow::bail!("Expected URL '{}', but got '{}'", url, current_url);
                }
            }

            match wait {
                Wait::Double(seconds) => {
                    tokio::time::sleep(std::time::Duration::from_secs_f64(*seconds)).await;
                }
                Wait::String(selector) => {
                    page.find_element(selector).await?;
                }
            }
        }
    }
    Ok(())
}
