use anyhow::Result;
use chromiumoxide::Page;

use crate::types::{ClickAction, TestAction};

pub async fn exec_test_action(page: &Page, action: &TestAction) -> Result<()> {
    match action {
        TestAction::Click(ClickAction { selector }) => {
            page.find_element(selector).await?.click().await?;
        } // Handle other actions here
    }
    Ok(())
}
