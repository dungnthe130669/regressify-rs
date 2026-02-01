use anyhow::Result;
use chromiumoxide::{
    Browser, cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams,
};

use crate::{
    test::test_action::exec_test_action,
    types::{TestScenario, TestSuite},
};

pub async fn exec_test_scenario(
    browser: &Browser,
    test_suite: &TestSuite,
    scenario: &TestScenario,
) -> Result<()> {
    let TestScenario { url, actions, .. } = scenario;

    eprint!("  Executing scenario for URL '{}': ", url);

    let page = browser.new_page(url.as_str()).await?;

    // Execute actions
    if let Some(actions) = actions {
        for action in actions {
            exec_test_action(&page, test_suite, scenario, action).await?;
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

    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        "final.png",
    )
    .await?;

    Ok(())
}
