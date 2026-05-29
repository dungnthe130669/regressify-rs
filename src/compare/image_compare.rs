use std::path::Path;
use std::time::Instant;

use anyhow::{Result, anyhow};
use lodepng::Image;
use rgb::RGBA8;

/// Result of comparing two images.
pub struct CompareResult {
    pub is_same_dimensions: bool,
    pub dimension_difference: (i64, i64),
    pub ref_width: usize,
    pub ref_height: usize,
    pub test_width: usize,
    pub test_height: usize,
    pub raw_mis_match_percentage: f64,
    pub mis_match_percentage: String,
    pub analysis_time_ms: u128,
}

/// Load a PNG file as RGBA pixels.
fn load_png(path: &Path) -> Result<(Vec<RGBA8>, usize, usize)> {
    let mut decoder = lodepng::Decoder::new();
    decoder.color_convert(true);
    decoder.info_raw_mut().colortype = lodepng::ColorType::RGBA;
    decoder.info_raw_mut().set_bitdepth(8);

    let decoded = decoder.decode_file(path)?;
    match decoded {
        Image::RGBA(img) => {
            let width = img.width;
            let height = img.height;
            let pixels: Vec<RGBA8> = img.buffer.to_vec();
            Ok((pixels, width, height))
        }
        _ => Err(anyhow!(
            "Failed to decode PNG as RGBA: {:?}",
            path.display()
        )),
    }
}

/// Compare two images and generate a diff image.
///
/// Returns the comparison result and the diff image pixels (if images have the same dimensions).
pub fn compare_images(
    reference_path: &Path,
    test_path: &Path,
) -> Result<(CompareResult, Option<Vec<u8>>)> {
    let start = Instant::now();

    let (ref_pixels, ref_w, ref_h) = load_png(reference_path)?;
    let (test_pixels, test_w, test_h) = load_png(test_path)?;

    let is_same_dimensions = ref_w == test_w && ref_h == test_h;
    let dimension_difference = (test_w as i64 - ref_w as i64, test_h as i64 - ref_h as i64);

    if !is_same_dimensions {
        let elapsed = start.elapsed().as_millis();
        return Ok((
            CompareResult {
                is_same_dimensions: false,
                dimension_difference,
                ref_width: ref_w,
                ref_height: ref_h,
                test_width: test_w,
                test_height: test_h,
                raw_mis_match_percentage: 100.0,
                mis_match_percentage: "100.00".to_string(),
                analysis_time_ms: elapsed,
            },
            None,
        ));
    }

    let total_pixels = ref_w * ref_h;
    let mut diff_count: u64 = 0;

    // RGBA diff image: same dimensions as the images
    let mut diff_pixels: Vec<u8> = Vec::with_capacity(total_pixels * 4);

    // Per-pixel color distance threshold (0-255 per channel)
    // A small tolerance avoids false positives from anti-aliasing/subpixel rendering
    const CHANNEL_TOLERANCE: i16 = 3;

    for i in 0..total_pixels {
        let rp = &ref_pixels[i];
        let tp = &test_pixels[i];

        let dr = (rp.r as i16 - tp.r as i16).abs();
        let dg = (rp.g as i16 - tp.g as i16).abs();
        let db = (rp.b as i16 - tp.b as i16).abs();
        let da = (rp.a as i16 - tp.a as i16).abs();

        let is_different = dr > CHANNEL_TOLERANCE
            || dg > CHANNEL_TOLERANCE
            || db > CHANNEL_TOLERANCE
            || da > CHANNEL_TOLERANCE;

        if is_different {
            diff_count += 1;
            // Highlight differences in magenta/pink
            diff_pixels.push(255); // R
            diff_pixels.push(0); // G
            diff_pixels.push(255); // B
            diff_pixels.push(200); // A
        } else {
            // Dimmed original pixel for context
            diff_pixels.push(rp.r / 3);
            diff_pixels.push(rp.g / 3);
            diff_pixels.push(rp.b / 3);
            diff_pixels.push(rp.a);
        }
    }

    let raw_mis_match = if total_pixels > 0 {
        (diff_count as f64 / total_pixels as f64) * 100.0
    } else {
        0.0
    };

    let elapsed = start.elapsed().as_millis();

    // Encode diff image to PNG
    let diff_png = lodepng::encode32(&diff_pixels, ref_w, ref_h)?;

    Ok((
        CompareResult {
            is_same_dimensions: true,
            dimension_difference,
            ref_width: ref_w,
            ref_height: ref_h,
            test_width: test_w,
            test_height: test_h,
            raw_mis_match_percentage: raw_mis_match,
            mis_match_percentage: format!("{:.2}", raw_mis_match),
            analysis_time_ms: elapsed,
        },
        Some(diff_png),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_compare_identical_images() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("test.png");

        // Create a small 2x2 red image
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let png_data = lodepng::encode32(&pixels, 2, 2).unwrap();
        fs::write(&img_path, &png_data).unwrap();

        let (result, diff) = compare_images(&img_path, &img_path).unwrap();
        assert!(result.is_same_dimensions);
        assert_eq!(result.raw_mis_match_percentage, 0.0);
        assert!(diff.is_some());
    }
}
