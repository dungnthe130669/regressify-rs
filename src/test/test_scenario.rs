use anyhow::Result;
use chromiumoxide::Browser;
use colored::Colorize;

use crate::{
    test::{capture::save_screenshot, test_action::exec_test_action},
    types::{ScreenshotType, TestScenario, TestSuite},
};

pub async fn exec_test_scenario(
    browser: &Browser,
    test_suite: &TestSuite,
    scenario: &TestScenario,
) -> Result<()> {
    let TestScenario { url, actions, .. } = scenario;

    eprintln!("  Executing scenario for URL {}: ", url.cyan());

    let page = browser.new_page(url.as_str()).await?;

    // Execute actions
    if let Some(actions) = actions {
        for action in actions {
            exec_test_action(&page, test_suite, scenario, action)
                .await
                .ok();
        }
    }

    if let Some(post_interaction_wait) = test_suite.post_interaction_wait
        && post_interaction_wait > 0.0
    {
        tokio::time::sleep(std::time::Duration::from_millis(
            post_interaction_wait as u64,
        ))
        .await;
    }

    save_screenshot(test_suite, scenario, &page, ScreenshotType::Final).await?;

    Ok(())
}
