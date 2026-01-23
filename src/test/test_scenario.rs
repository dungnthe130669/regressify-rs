use anyhow::Result;
use chromiumoxide::Browser;

use crate::{test::test_action::exec_test_action, types::TestScenario};

pub async fn exec_test_scenario<'a>(browser: &Browser, scenario: &'a TestScenario) -> Result<()> {
    let TestScenario { url, actions, .. } = scenario;

    let page = browser.new_page(url.as_str()).await?;

    // Execute actions
    if let Some(actions) = actions {
        for action in actions {
            exec_test_action(&page, action).await?;
        }
    }

    Ok(())
}
