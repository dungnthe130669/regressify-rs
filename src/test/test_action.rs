use anyhow::{Result, anyhow};
use chromiumoxide::Page;

use crate::{
    test::capture::save_screenshot,
    types::{
        ClickAction, SaveAction, ScreenshotType, TestAction, TestScenario, TestSuite, Wait,
        WaitAction,
    },
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
            save_screenshot(
                test_suite,
                scenario,
                page,
                ScreenshotType::Step(save.to_string()),
            )
            .await?
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
