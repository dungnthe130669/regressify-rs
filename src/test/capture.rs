use std::collections::HashMap;
use std::fs::{self, create_dir_all};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use chromiumoxide::{
    Page,
    cdp::browser_protocol::page::{CaptureScreenshotFormat, Viewport},
    page::ScreenshotParams,
};
use rgb::RGBA8;
use serde::{Deserialize, Serialize};

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

/// Build the on-disk path for a scenario screenshot, creating the run directory.
/// Shared by real captures and the error-placeholder writer so both land on the
/// exact same filename (`<scenario-id>[_<viewport>]__<step|FINAL>.png`).
fn screenshot_path(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    screenshot_type: &ScreenshotType,
    viewport_label: Option<&str>,
) -> Result<String> {
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

    // When capturing at a named viewport, include it in the filename so multiple
    // viewports for the same scenario produce distinct (non-colliding) files.
    let vp_suffix = viewport_label
        .map(|l| format!("_{}", slugify(l)))
        .unwrap_or_default();

    Ok(format!(
        "{}/{}{}__{}{}.png",
        test_suite_dir,
        scenario_id,
        vp_suffix,
        match screenshot_type {
            ScreenshotType::Step(_) => "$",
            ScreenshotType::Final => "",
        },
        match screenshot_type {
            ScreenshotType::Step(step) => step.as_str(),
            ScreenshotType::Final => "FINAL",
        }
    ))
}

/// Filename of the report ordering manifest written alongside the captured
/// screenshots. It records the intended display order of screenshots (scenario
/// order, then viewport order) so the comparison report does not fall back to
/// alphabetical filename sorting.
pub const REPORT_ORDER_FILE: &str = ".report-order.json";
pub const REPORT_THRESHOLDS_FILE: &str = ".report-thresholds.json";

fn run_dir(test_suite: &TestSuite) -> Result<PathBuf> {
    let test_suite_id = test_suite
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test suite ID is required"))?;
    let test_date = test_suite
        .test_date
        .as_deref()
        .ok_or_else(|| anyhow!("Test suite date is required"))?;
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;
    Ok(regressify_dir
        .join(test_suite_id)
        .join("bitmaps_test")
        .join(test_date))
}

pub fn validate_state_name(name: &str) -> Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        anyhow::bail!(
            "Invalid state name '{}': must not contain path separators or '..'",
            name
        );
    }
    Ok(())
}

pub fn state_path(name: &str) -> Result<PathBuf> {
    validate_state_name(name)?;
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;
    Ok(regressify_dir.join("states").join(format!("{}.json", name)))
}

/// Write the report ordering manifest for a suite run. `prefixes` is the ordered
/// list of screenshot name prefixes (`<scenario-id>[_<viewport>]`, the part of
/// each filename before the `__<step|FINAL>` suffix) in the order the scenarios
/// and their viewports are defined. The comparison report uses this to list
/// screenshots in scenario-then-viewport order instead of alphabetically.
pub fn write_report_order(test_suite: &TestSuite, prefixes: &[String]) -> Result<()> {
    let run_dir = run_dir(test_suite)?;
    create_dir_all(&run_dir)?;

    let manifest = serde_json::to_vec_pretty(prefixes)?;
    fs::write(run_dir.join(REPORT_ORDER_FILE), manifest)?;
    Ok(())
}

pub fn write_report_thresholds(test_suite: &TestSuite, thresholds: &[(String, f64)]) -> Result<()> {
    let run_dir = run_dir(test_suite)?;
    create_dir_all(&run_dir)?;

    let map = thresholds.iter().cloned().collect::<HashMap<String, f64>>();
    let manifest = serde_json::to_vec_pretty(&map)?;
    fs::write(run_dir.join(REPORT_THRESHOLDS_FILE), manifest)?;
    Ok(())
}

pub async fn save_screenshot(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    page: &Page,
    screenshot_type: ScreenshotType,
    viewport_label: Option<&str>,
) -> Result<()> {
    let path = screenshot_path(test_suite, scenario, &screenshot_type, viewport_label)?;

    // Full-page capture, matching Playwright's deterministic screenshot path.
    //
    // We deliberately do NOT use chromiumoxide's `.full_page(true)` helper. That
    // helper calls `Page.setDeviceMetricsOverride` to resize the emulated
    // viewport to the full document height, which forces a relayout + repaint at
    // a new size right before the capture. That post-resize repaint is timing
    // sensitive and not deterministic — anti-aliased edges settle to slightly
    // different sub-pixel values between runs (e.g. a single white vs. cream
    // pixel). Because PNG/deflate re-aligns the entire stream on any pixel
    // change, even one drifting pixel makes the whole file differ, so `ref`
    // floods git with "changed" baselines on every run.
    //
    // Instead we replicate Playwright exactly: read the document size from
    // `Page.getLayoutMetrics`, then capture with an explicit clip and
    // `captureBeyondViewport: true`. Chromium renders the whole page in a single
    // compositor pass WITHOUT resizing the viewport (no relayout), which is
    // reproducible run-over-run. `fromSurface: true` reads from the compositor
    // surface for further stability.
    let metrics = page.layout_metrics().await?;
    let width = metrics.css_content_size.width;
    let height = metrics.css_content_size.height;

    let params = ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .clip(Viewport {
            x: 0.,
            y: 0.,
            width,
            height,
            scale: 1.,
        })
        .capture_beyond_viewport(true)
        .from_surface(true)
        .omit_background(false)
        .build();

    let png_bytes = page.screenshot(params).await?;
    let clean_bytes = strip_png_metadata(&png_bytes)?;
    fs::write(&path, &clean_bytes)?;

    Ok(())
}

/// Deterministic error-placeholder PNG (a red warning triangle with an
/// exclamation mark on a white background). Generated once and cached. The
/// image is only for display in the report; failure detection is driven by the
/// JSON error marker (see [`write_error_screenshot`] / [`read_error_marker`]).
fn error_icon_png() -> &'static [u8] {
    static ICON: OnceLock<Vec<u8>> = OnceLock::new();
    ICON.get_or_init(generate_error_icon).as_slice()
}

/// Render the error placeholder as raw PNG bytes.
fn generate_error_icon() -> Vec<u8> {
    const W: usize = 512;
    const H: usize = 512;

    let red = RGBA8::new(214, 40, 40, 255);
    let white = RGBA8::new(255, 255, 255, 255);
    let dark = RGBA8::new(30, 30, 30, 255);

    // White background.
    let mut img = vec![white; W * H];

    // Triangle vertices (outer red border) and an inwardly-scaled white interior.
    let outer = [(256.0_f64, 48.0_f64), (40.0, 452.0), (472.0, 452.0)];
    let cx = (outer[0].0 + outer[1].0 + outer[2].0) / 3.0;
    let cy = (outer[0].1 + outer[1].1 + outer[2].1) / 3.0;
    let shrink = 0.74;
    let inner = [
        (
            cx + (outer[0].0 - cx) * shrink,
            cy + (outer[0].1 - cy) * shrink,
        ),
        (
            cx + (outer[1].0 - cx) * shrink,
            cy + (outer[1].1 - cy) * shrink,
        ),
        (
            cx + (outer[2].0 - cx) * shrink,
            cy + (outer[2].1 - cy) * shrink,
        ),
    ];

    let sign = |p: (f64, f64), a: (f64, f64), b: (f64, f64)| -> f64 {
        (p.0 - b.0) * (a.1 - b.1) - (a.0 - b.0) * (p.1 - b.1)
    };
    let in_tri = |p: (f64, f64), t: &[(f64, f64); 3]| -> bool {
        let d1 = sign(p, t[0], t[1]);
        let d2 = sign(p, t[1], t[2]);
        let d3 = sign(p, t[2], t[0]);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    };

    for y in 0..H {
        for x in 0..W {
            let p = (x as f64 + 0.5, y as f64 + 0.5);
            if in_tri(p, &outer) {
                img[y * W + x] = if in_tri(p, &inner) { white } else { red };
            }
        }
    }

    // Exclamation mark (bar + dot) painted dark over the white interior.
    let bar_half = 20.0;
    let bar_top = 205.0;
    let bar_bottom = 350.0;
    let dot_cy = 400.0;
    let dot_r = 24.0;
    for y in 0..H {
        for x in 0..W {
            let fx = x as f64 + 0.5;
            let fy = y as f64 + 0.5;
            let on_bar = (fx - cx).abs() <= bar_half && (bar_top..=bar_bottom).contains(&fy);
            let on_dot = (fx - cx).powi(2) + (fy - dot_cy).powi(2) <= dot_r * dot_r;
            if on_bar || on_dot {
                img[y * W + x] = dark;
            }
        }
    }

    lodepng::encode32(&img, W, H).expect("failed to encode error placeholder PNG")
}

/// Sidecar JSON marker recording that a screenshot is a capture failure.
#[derive(Debug, Serialize, Deserialize)]
struct ErrorMarker {
    error: bool,
    #[serde(default)]
    message: String,
}

/// Path of the JSON error-marker sidecar for a screenshot file.
/// e.g. `scenario__FINAL.png` -> `scenario__FINAL.png.error.json`.
pub fn error_marker_path(png_path: &Path) -> PathBuf {
    let mut s = png_path.as_os_str().to_owned();
    s.push(".error.json");
    PathBuf::from(s)
}

/// Write the error placeholder image AND its JSON marker to the scenario's
/// final screenshot path. Used when a scenario fails before producing a real
/// screenshot, so the report has an image to show and the comparison can detect
/// the failure via the marker flag instead of inspecting pixels.
pub fn write_error_screenshot(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    viewport_label: Option<&str>,
    message: &str,
) -> Result<()> {
    let path = screenshot_path(test_suite, scenario, &ScreenshotType::Final, viewport_label)?;
    fs::write(&path, error_icon_png())?;

    let marker = ErrorMarker {
        error: true,
        message: message.to_string(),
    };
    let marker_json = serde_json::to_vec_pretty(&marker)?;
    fs::write(error_marker_path(Path::new(&path)), marker_json)?;
    Ok(())
}

/// Read the error marker for a screenshot. Returns the recorded failure message
/// when the screenshot is flagged as a capture failure (`error: true`), or
/// `None` when there is no marker or the flag is false.
pub fn read_error_marker(png_path: &Path) -> Option<String> {
    let bytes = fs::read(error_marker_path(png_path)).ok()?;
    let marker: ErrorMarker = serde_json::from_slice(&bytes).ok()?;
    marker.error.then_some(marker.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_icon_is_deterministic_and_valid_png() {
        const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
        let a = error_icon_png();
        let b = generate_error_icon();
        assert_eq!(a, b.as_slice(), "error icon must be byte-deterministic");
        assert_eq!(&a[..8], &PNG_SIGNATURE, "error icon must be a valid PNG");
    }

    #[test]
    fn error_marker_round_trips_and_reflects_flag() {
        let dir = std::env::temp_dir().join(format!("regressify_err_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let png = dir.join("scenario__FINAL.png");
        fs::write(&png, error_icon_png()).unwrap();

        // No marker yet.
        assert!(read_error_marker(&png).is_none());

        // error: true -> returns the message.
        let marker = ErrorMarker {
            error: true,
            message: "boom".to_string(),
        };
        fs::write(
            error_marker_path(&png),
            serde_json::to_vec(&marker).unwrap(),
        )
        .unwrap();
        assert_eq!(read_error_marker(&png).as_deref(), Some("boom"));

        // error: false -> treated as no failure.
        let ok = ErrorMarker {
            error: false,
            message: String::new(),
        };
        fs::write(error_marker_path(&png), serde_json::to_vec(&ok).unwrap()).unwrap();
        assert!(read_error_marker(&png).is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
