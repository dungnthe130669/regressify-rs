use anyhow::Result;
use chromiumoxide::{
    Page, cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams,
};

use crate::types::{ClickAction, TestAction};

pub async fn exec_test_action(page: &Page, action: &TestAction) -> Result<()> {
    match action {
        TestAction::Click(ClickAction { selector }) => {
            page.find_element(selector).await?.click().await?;
        }

        TestAction::Save(name) => {
            page.save_screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .full_page(true)
                    .omit_background(true)
                    .build(),
                format!("{}.png", name),
            )
            .await?;
        }
    }
    Ok(())
}
