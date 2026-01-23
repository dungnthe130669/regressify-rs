use anyhow::Result;
use chromiumoxide::{
    Page, cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams,
};

use crate::types::{ClickAction, SaveAction, TestAction};

pub async fn exec_test_action(page: &Page, action: &TestAction) -> Result<()> {
    match action {
        TestAction::Click(ClickAction { click }) => {
            page.find_element(click).await?.click().await?;
        }

        TestAction::Save(SaveAction { save }) => {
            page.save_screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .full_page(true)
                    .omit_background(true)
                    .build(),
                format!("{}.png", save),
            )
            .await?;
        }
    }
    Ok(())
}
