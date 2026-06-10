use std::fs::{self, create_dir_all};

use anyhow::{Result, anyhow};
use chromiumoxide::{
    Page, cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams,
};

use crate::{
    common,
    helpers::{OnceLockExtensions, slugify},
    types::{ScreenshotType, TestScenario, TestSuite},
};

/// PNG chunk types that carry pixel data and are required for correct rendering.
/// All other chunks (tIME, tEXt, iTXt, zTXt, etc.) are metadata that can vary
/// between captures and cause false git diffs.
const ESSENTIAL_PNG_CHUNKS: [&[u8; 4]; 5] = [b"IHDR", b"PLTE", b"tRNS", b"IDAT", b"IEND"];

/// Strip non-essential metadata chunks from a PNG byte buffer so that
/// identical pixel content always produces byte-identical files.
fn strip_png_metadata(png: &[u8]) -> Result<Vec<u8>> {
    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

    if png.len() < 8 || png[..8] != PNG_SIGNATURE {
        return Err(anyhow!("Invalid PNG signature"));
    }

    let mut out = Vec::with_capacity(png.len());
    out.extend_from_slice(&PNG_SIGNATURE);

    let mut pos = 8;
    while pos + 8 <= png.len() {
        let data_len =
            u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]]) as usize;
        let chunk_type = &png[pos + 4..pos + 8];
        // total chunk size = 4 (length) + 4 (type) + data_len + 4 (CRC)
        let chunk_total = 12 + data_len;

        if pos + chunk_total > png.len() {
            return Err(anyhow!("Truncated PNG chunk"));
        }

        if ESSENTIAL_PNG_CHUNKS
            .iter()
            .any(|t| t.as_slice() == chunk_type)
        {
            out.extend_from_slice(&png[pos..pos + chunk_total]);
        }

        pos += chunk_total;
    }

    Ok(out)
}

pub async fn save_screenshot(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    page: &Page,
    screenshot_type: ScreenshotType,
    viewport_label: Option<&str>,
) -> Result<()> {
    let test_suite_id = test_suite
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test suite ID is required for Save action"))?;

    let scenario_id = scenario
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test scenario ID is required for Save action"))?;

    let test_date = test_suite
        .test_date
        .as_deref()
        .ok_or_else(|| anyhow!("Test suite date is required for Save action"))?;

    let test_dir = common::REGRESSIFY_DIR_NAME.get_required()?;
    let test_suite_dir = format!("{}/{}/bitmaps_test/{}", test_dir, test_suite_id, test_date);

    create_dir_all(&test_suite_dir)?;

    let params = ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .full_page(true)
        .omit_background(false)
        .build();

    // When capturing at a named viewport, include it in the filename so multiple
    // viewports for the same scenario produce distinct (non-colliding) files.
    let vp_suffix = viewport_label
        .map(|l| format!("_{}", slugify(l)))
        .unwrap_or_default();

    let path = format!(
        "{}/{}{}__{}{}.png",
        test_suite_dir,
        scenario_id,
        vp_suffix,
        match screenshot_type {
            ScreenshotType::Step(_) => "$",
            ScreenshotType::Final => "",
        },
        match &screenshot_type {
            ScreenshotType::Step(step) => step,
            ScreenshotType::Final => "FINAL",
        }
    );

    let png_bytes = page.screenshot(params).await?;
    let clean_bytes = strip_png_metadata(&png_bytes)?;
    fs::write(&path, &clean_bytes)?;

    Ok(())
}
