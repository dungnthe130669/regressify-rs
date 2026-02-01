use anyhow::{Result, anyhow};
use chromiumoxide::Page;

use crate::{
    test::capture::save_screenshot,
    types::{
        ClickAction, InputAction, SaveAction, ScreenshotType, TestAction, TestScenario, TestSuite,
        Wait, WaitAction,
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
            eprintln!("    Clicking element: {}", click);
            page.find_element(click).await?.click().await?;
        }

        TestAction::Input(InputAction { input, value }) => {
            eprintln!("    Typing into element: {}", input);
            page.find_element(input).await?.type_str(value).await?;
        }

        TestAction::Save(SaveAction { save }) => {
            eprintln!("    Saving screenshot: {}", save);

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
                eprintln!("    Waiting: {}", url);

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
                    eprintln!("    Waiting for {} seconds", seconds);
                    tokio::time::sleep(std::time::Duration::from_secs_f64(*seconds)).await;
                }
                Wait::String(selector) => {
                    eprintln!("    Waiting for element: {}", selector);
                    page.find_element(selector).await?;
                }
            }
        }
    }
    Ok(())
}
