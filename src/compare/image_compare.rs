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

// ─── Perceptual comparison (pixelmatch-style) ────────────────────────────────

/// Matching threshold (0 = exact, 1 = very loose). 0.1 mirrors pixelmatch's
/// default and tolerates subpixel/anti-aliasing rendering noise.
const MATCH_THRESHOLD: f64 = 0.1;
/// Maximum possible YIQ color delta squared (pixelmatch constant).
const MAX_YIQ_DELTA: f64 = 35215.0;

#[inline]
fn blend_channel(c: f64, a: f64) -> f64 {
    255.0 + (c - 255.0) * a
}

/// Signed perceptual color delta (YIQ) between pixel `k` in `a` and `m` in `b`.
/// When `y_only` is true, returns only the brightness delta (used for AA detection).
fn color_delta(a: &[RGBA8], b: &[RGBA8], k: usize, m: usize, y_only: bool) -> f64 {
    let (mut r1, mut g1, mut b1, a1) = (a[k].r as f64, a[k].g as f64, a[k].b as f64, a[k].a as f64);
    let (mut r2, mut g2, mut b2, a2) = (b[m].r as f64, b[m].g as f64, b[m].b as f64, b[m].a as f64);

    if (a1 - a2).abs() < f64::EPSILON && r1 == r2 && g1 == g2 && b1 == b2 {
        return 0.0;
    }

    if a1 < 255.0 {
        let a1n = a1 / 255.0;
        r1 = blend_channel(r1, a1n);
        g1 = blend_channel(g1, a1n);
        b1 = blend_channel(b1, a1n);
    }
    if a2 < 255.0 {
        let a2n = a2 / 255.0;
        r2 = blend_channel(r2, a2n);
        g2 = blend_channel(g2, a2n);
        b2 = blend_channel(b2, a2n);
    }

    let y1 = r1 * 0.298_895_31 + g1 * 0.586_622_47 + b1 * 0.114_482_23;
    let y2 = r2 * 0.298_895_31 + g2 * 0.586_622_47 + b2 * 0.114_482_23;
    let y = y1 - y2;

    if y_only {
        return y;
    }

    let i = (r1 * 0.595_977_99 - g1 * 0.274_176_1 - b1 * 0.321_801_89)
        - (r2 * 0.595_977_99 - g2 * 0.274_176_1 - b2 * 0.321_801_89);
    let q = (r1 * 0.211_470_17 - g1 * 0.522_617_11 + b1 * 0.311_146_94)
        - (r2 * 0.211_470_17 - g2 * 0.522_617_11 + b2 * 0.311_146_94);

    let delta = 0.505_3 * y * y + 0.299 * i * i + 0.195_7 * q * q;

    // Encode the sign of the brightness change for anti-alias detection.
    if y1 > y2 { -delta } else { delta }
}

/// Count whether a pixel has at least 3 equal neighbours (used by AA detection).
fn has_many_siblings(img: &[RGBA8], x1: usize, y1: usize, w: usize, h: usize) -> bool {
    let x0 = x1.saturating_sub(1);
    let y0 = y1.saturating_sub(1);
    let x2 = (x1 + 1).min(w - 1);
    let y2 = (y1 + 1).min(h - 1);
    let pos = y1 * w + x1;
    let mut zeroes = if x1 == 0 || y1 == 0 || x1 == w - 1 || y1 == h - 1 {
        1
    } else {
        0
    };

    for y in y0..=y2 {
        for x in x0..=x2 {
            if x == x1 && y == y1 {
                continue;
            }
            let q = y * w + x;
            if img[pos].r == img[q].r
                && img[pos].g == img[q].g
                && img[pos].b == img[q].b
                && img[pos].a == img[q].a
            {
                zeroes += 1;
                if zeroes > 2 {
                    return true;
                }
            }
        }
    }
    false
}

/// Determine whether the pixel at (x1, y1) is likely anti-aliasing rather than a
/// real difference (pixelmatch algorithm).
fn is_antialiased(
    img: &[RGBA8],
    x1: usize,
    y1: usize,
    w: usize,
    h: usize,
    other: &[RGBA8],
) -> bool {
    let x0 = x1.saturating_sub(1);
    let y0 = y1.saturating_sub(1);
    let x2 = (x1 + 1).min(w - 1);
    let y2 = (y1 + 1).min(h - 1);
    let pos = y1 * w + x1;
    let mut zeroes = if x1 == 0 || y1 == 0 || x1 == w - 1 || y1 == h - 1 {
        1
    } else {
        0
    };

    let mut min = 0.0_f64;
    let mut max = 0.0_f64;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0usize, 0usize, 0usize, 0usize);

    for y in y0..=y2 {
        for x in x0..=x2 {
            if x == x1 && y == y1 {
                continue;
            }
            let delta = color_delta(img, img, pos, y * w + x, true);
            if delta == 0.0 {
                zeroes += 1;
                if zeroes > 2 {
                    return false;
                }
            } else if delta < min {
                min = delta;
                min_x = x;
                min_y = y;
            } else if delta > max {
                max = delta;
                max_x = x;
                max_y = y;
            }
        }
    }

    if min == 0.0 || max == 0.0 {
        return false;
    }

    (has_many_siblings(img, min_x, min_y, w, h) && has_many_siblings(other, min_x, min_y, w, h))
        || (has_many_siblings(img, max_x, max_y, w, h)
            && has_many_siblings(other, max_x, max_y, w, h))
}

/// Compare two images and generate a diff image.
///
/// Uses a perceptual (YIQ) color metric with anti-aliasing detection to avoid
/// false positives from subpixel/font rendering. When dimensions differ, a diff
/// image sized to the union of both images is still produced: the overlapping
/// region is compared pixel-by-pixel and the non-overlapping area is flagged.
///
/// Returns the comparison result and the diff image PNG bytes.
pub fn compare_images(
    reference_path: &Path,
    test_path: &Path,
) -> Result<(CompareResult, Option<Vec<u8>>)> {
    let start = Instant::now();

    let (ref_pixels, ref_w, ref_h) = load_png(reference_path)?;
    let (test_pixels, test_w, test_h) = load_png(test_path)?;

    let is_same_dimensions = ref_w == test_w && ref_h == test_h;
    let dimension_difference = (test_w as i64 - ref_w as i64, test_h as i64 - ref_h as i64);

    // Decoding normalizes PNG encoding details such as palette layout and
    // metadata. Most repeat captures reach this path, so avoid allocating and
    // encoding a full-page diff image when their rendered pixels are identical.
    if is_same_dimensions && ref_pixels == test_pixels {
        return Ok((
            CompareResult {
                is_same_dimensions,
                dimension_difference,
                ref_width: ref_w,
                ref_height: ref_h,
                test_width: test_w,
                test_height: test_h,
                raw_mis_match_percentage: 0.0,
                mis_match_percentage: "0.00".to_string(),
                analysis_time_ms: start.elapsed().as_millis(),
            },
            None,
        ));
    }

    let max_delta = MAX_YIQ_DELTA * MATCH_THRESHOLD * MATCH_THRESHOLD;

    let out_w = ref_w.max(test_w);
    let out_h = ref_h.max(test_h);
    let total_pixels = out_w
        .checked_mul(out_h)
        .ok_or_else(|| anyhow!("Image dimensions are too large to compare safely"))?;
    let diff_capacity = total_pixels
        .checked_mul(4)
        .ok_or_else(|| anyhow!("Image dimensions are too large to build a diff safely"))?;
    let mut diff_count: u64 = 0;
    let mut diff_pixels: Vec<u8> = Vec::with_capacity(diff_capacity);

    for y in 0..out_h {
        for x in 0..out_w {
            let in_ref = x < ref_w && y < ref_h;
            let in_test = x < test_w && y < test_h;

            // Outside the overlap of the two images → always a difference.
            if !(in_ref && in_test) {
                diff_count += 1;
                diff_pixels.extend_from_slice(&[255, 0, 255, 200]);
                continue;
            }

            let rk = y * ref_w + x;
            let tk = y * test_w + x;
            let delta = color_delta(&ref_pixels, &test_pixels, rk, tk, false);

            if delta.abs() > max_delta {
                // Ignore differences that are anti-aliasing on either side. AA
                // detection requires matching dimensions to compare neighbours.
                let aa = is_same_dimensions
                    && (is_antialiased(&ref_pixels, x, y, ref_w, ref_h, &test_pixels)
                        || is_antialiased(&test_pixels, x, y, test_w, test_h, &ref_pixels));
                if aa {
                    let rp = &ref_pixels[rk];
                    diff_pixels.extend_from_slice(&[rp.r / 3, rp.g / 3, rp.b / 3, rp.a]);
                } else {
                    diff_count += 1;
                    diff_pixels.extend_from_slice(&[255, 0, 255, 200]);
                }
            } else {
                let rp = &ref_pixels[rk];
                diff_pixels.extend_from_slice(&[rp.r / 3, rp.g / 3, rp.b / 3, rp.a]);
            }
        }
    }

    let raw_mis_match = if total_pixels > 0 {
        (diff_count as f64 / total_pixels as f64) * 100.0
    } else {
        0.0
    };

    let elapsed = start.elapsed().as_millis();
    let diff_png = if diff_count == 0 {
        None
    } else {
        Some(lodepng::encode32(&diff_pixels, out_w, out_h)?)
    };

    Ok((
        CompareResult {
            is_same_dimensions,
            dimension_difference,
            ref_width: ref_w,
            ref_height: ref_h,
            test_width: test_w,
            test_height: test_h,
            raw_mis_match_percentage: raw_mis_match,
            mis_match_percentage: format!("{:.2}", raw_mis_match),
            analysis_time_ms: elapsed,
        },
        diff_png,
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
        assert!(diff.is_none());
    }

    #[test]
    fn test_compare_different_images_generates_diff() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        fs::write(
            &reference_path,
            lodepng::encode32(&[255_u8, 0, 0, 255], 1, 1).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&[0_u8, 0, 255, 255], 1, 1).unwrap(),
        )
        .unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert!(result.is_same_dimensions);
        assert_eq!(result.raw_mis_match_percentage, 100.0);
        assert!(diff.is_some());
    }

    #[test]
    fn test_compare_dimension_mismatch_generates_diff() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        fs::write(
            &reference_path,
            lodepng::encode32(&[10_u8, 20, 30, 255], 1, 1).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&[10_u8, 20, 30, 255, 10, 20, 30, 255], 2, 1).unwrap(),
        )
        .unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert!(!result.is_same_dimensions);
        assert_eq!(result.dimension_difference, (1, 0));
        assert_eq!(result.raw_mis_match_percentage, 50.0);
        assert!(diff.is_some());
    }
}
