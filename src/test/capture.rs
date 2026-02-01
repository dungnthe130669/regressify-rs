use std::fs::create_dir_all;

use anyhow::{Result, anyhow};
use chromiumoxide::{
    Page, cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams,
};

use crate::{
    common,
    helpers::OnceLockExtensions,
    types::{ScreenshotType, TestScenario, TestSuite},
};

pub async fn save_screenshot(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    page: &Page,
    screenshot_type: ScreenshotType,
) -> Result<()> {
    let test_suite_id = test_suite
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test suite ID is required for Save action"))?;

    let scenario_id = scenario
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test scenario ID is required for Save action"))?;

    let test_dir = common::BACKSTOP_DIR_NAME.get_required()?;
    let test_suite_dir = format!("{}/{}", test_dir, test_suite_id);

    create_dir_all(&test_suite_dir)?;

    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .full_page(true)
            .omit_background(true)
            .build(),
        format!(
            "{}/{}/tests/{}/{}__{}{}.png",
            test_dir,
            test_suite_id,
            test_suite
                .test_date
                .as_deref()
                .ok_or_else(|| anyhow!("Test suite date is required for Save action"))?,
            scenario_id,
            match screenshot_type {
                ScreenshotType::Step(_) => "$",
                ScreenshotType::Final => "",
            },
            match &screenshot_type {
                ScreenshotType::Step(step) => step,
                ScreenshotType::Final => "final",
            }
        ),
    )
    .await?;

    Ok(())
}
