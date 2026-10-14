use std::{path::Path, time::Instant};

use anyhow::{Result, anyhow};
use lodepng::Image;
use rgb::RGBA8;
use serde::{Deserialize, Serialize};

use crate::{
    config::DEFAULT_PIXEL_MATCH_THRESHOLD,
    helpers::png::{MAX_VISUAL_IMAGE_PIXELS, read_bounded_png_dimensions},
};

/// Result of comparing two images.
pub struct CompareResult {
    pub is_same_dimensions: bool,
    pub dimension_difference: (i64, i64),
    pub ref_width: usize,
    pub ref_height: usize,
    pub test_width: usize,
    pub test_height: usize,
    pub alignment: DiffAlignment,
    pub raw_mis_match_percentage: f64,
    pub mis_match_percentage: String,
    pub analysis_time_ms: u128,
}

/// Load a PNG file as RGBA pixels.
pub(super) fn load_png(path: &Path) -> Result<(Vec<RGBA8>, usize, usize)> {
    let (expected_width, expected_height) = read_bounded_png_dimensions(path)?;
    let mut decoder = lodepng::Decoder::new();
    decoder.color_convert(true);
    decoder.info_raw_mut().colortype = lodepng::ColorType::RGBA;
    decoder.info_raw_mut().set_bitdepth(8);

    let decoded = decoder.decode_file(path)?;
    match decoded {
        Image::RGBA(img) => {
            let width = img.width;
            let height = img.height;
            if (width, height) != (expected_width, expected_height) {
                return Err(anyhow!(
                    "PNG '{}' decoded to dimensions {}x{} instead of its {}x{} IHDR dimensions",
                    path.display(),
                    width,
                    height,
                    expected_width,
                    expected_height
                ));
            }
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

/// Maximum possible YIQ color delta squared (pixelmatch constant).
const MAX_YIQ_DELTA: f64 = 35215.0;
const MAX_VERTICAL_SHIFT: usize = 512;
const MIN_ALIGNED_ROWS: usize = 32;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct VerticalGap {
    pub start: usize,
    pub height: usize,
}

impl VerticalGap {
    pub fn for_image(self, image_height: usize, diff_height: usize) -> Option<Self> {
        (self.height > 0
            && self.start <= image_height
            && image_height.checked_add(self.height) == Some(diff_height))
        .then_some(self)
    }

    pub fn to_aligned_row(self, row: f32) -> f32 {
        if row >= self.start as f32 {
            row + self.height as f32
        } else {
            row
        }
    }

    pub fn to_image_row(self, row: f32) -> Option<f32> {
        if row < self.start as f32 {
            Some(row)
        } else if row < self.start as f32 + self.height as f32 {
            None
        } else {
            Some(row - self.height as f32)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct DiffAlignment {
    pub reference: Option<VerticalGap>,
    pub test: Option<VerticalGap>,
}

#[derive(Clone, Copy)]
struct ImageView<'a> {
    pixels: &'a [RGBA8],
    width: usize,
    height: usize,
    gap: Option<VerticalGap>,
}

impl ImageView<'_> {
    fn pixel_index(self, column: usize, row: usize) -> Option<usize> {
        let source_row = match self.gap {
            Some(gap) if row >= gap.start => {
                if row < gap.start + gap.height {
                    return None;
                }
                row - gap.height
            }
            _ => row,
        };
        (column < self.width && source_row < self.height)
            .then_some(source_row * self.width + column)
    }
}

fn row_difference(
    shorter: ImageView<'_>,
    taller: ImageView<'_>,
    row: usize,
    taller_row: usize,
    max_delta: f64,
) -> usize {
    (0..shorter.width)
        .filter(|column| {
            color_delta(
                shorter.pixels,
                taller.pixels,
                row * shorter.width + column,
                taller_row * taller.width + column,
                false,
            )
            .abs()
                > max_delta
        })
        .count()
}

fn find_vertical_gap(
    shorter: ImageView<'_>,
    taller: ImageView<'_>,
    max_delta: f64,
) -> Option<VerticalGap> {
    if shorter.width != taller.width || shorter.height >= taller.height {
        return None;
    }
    let height = taller.height - shorter.height;
    if height > MAX_VERTICAL_SHIFT {
        return None;
    }

    let mut gain = 0_i64;
    let mut supporting_rows = 0;
    let mut best_gain = 0;
    let mut best_start = shorter.height;
    let mut best_support = 0;

    for row in (0..shorter.height).rev() {
        let unshifted = row_difference(shorter, taller, row, row, max_delta);
        let shifted = row_difference(shorter, taller, row, row + height, max_delta);
        gain += unshifted as i64 - shifted as i64;
        if shifted * 100 <= shorter.width && unshifted > shifted {
            supporting_rows += 1;
        }
        if gain > best_gain {
            best_gain = gain;
            best_start = row;
            best_support = supporting_rows;
        }
    }

    (best_support >= MIN_ALIGNED_ROWS).then_some(VerticalGap {
        start: best_start,
        height,
    })
}

fn matching_reference_pixel(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    x: usize,
    y: usize,
    max_delta: f64,
) -> Option<usize> {
    let (Some(reference_index), Some(test_index)) =
        (reference.pixel_index(x, y), test.pixel_index(x, y))
    else {
        return None;
    };

    let delta = color_delta(
        reference.pixels,
        test.pixels,
        reference_index,
        test_index,
        false,
    );
    if delta.abs() <= max_delta {
        return Some(reference_index);
    }

    let same_dimensions = reference.width == test.width && reference.height == test.height;
    (same_dimensions
        && (is_antialiased(
            reference.pixels,
            x,
            y,
            reference.width,
            reference.height,
            test.pixels,
        ) || is_antialiased(test.pixels, x, y, test.width, test.height, reference.pixels)))
    .then_some(reference_index)
}

/// Compare pixels and build the diff buffer in a single pass, allocating the
/// diff buffer lazily only when the first differing pixel is encountered.
fn compare_and_render_diff(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    max_delta: f64,
    capacity: usize,
) -> (u64, Option<Vec<u8>>) {
    let width = reference.width.max(test.width);
    let height = reference.height.max(test.height);
    let mut count = 0_u64;
    let mut diff_pixels: Option<Vec<u8>> = None;

    for y in 0..height {
        for x in 0..width {
            match matching_reference_pixel(reference, test, x, y, max_delta) {
                Some(reference_index) => {
                    if let Some(pixels) = &mut diff_pixels {
                        let pixel = &reference.pixels[reference_index];
                        pixels.extend_from_slice(&[pixel.r / 3, pixel.g / 3, pixel.b / 3, pixel.a]);
                    }
                }
                None => {
                    count += 1;
                    let pixels = diff_pixels.get_or_insert_with(|| {
                        let mut pixels = Vec::with_capacity(capacity);
                        for prev_y in 0..=y {
                            let end_x = if prev_y == y { x } else { width };
                            for prev_x in 0..end_x {
                                if let Some(index) = reference.pixel_index(prev_x, prev_y) {
                                    let pixel = &reference.pixels[index];
                                    pixels.extend_from_slice(&[
                                        pixel.r / 3,
                                        pixel.g / 3,
                                        pixel.b / 3,
                                        pixel.a,
                                    ]);
                                }
                            }
                        }
                        pixels
                    });
                    pixels.extend_from_slice(&[255, 0, 255, 200]);
                }
            }
        }
    }
    (count, diff_pixels)
}

/// Compare two images and generate a diff image only for visual mismatches.
///
/// Uses a perceptual (YIQ) color metric with anti-aliasing detection to avoid
/// false positives from subpixel/font rendering. When dimensions differ, a diff
/// image sized to the union of both images is still produced. A single vertical
/// insertion/deletion is aligned when matching rows support it; the missing band
/// and all remaining pixel differences are flagged.
///
/// Returns the comparison result and the diff image PNG bytes.
pub fn compare_images(
    reference_path: &Path,
    test_path: &Path,
) -> Result<(CompareResult, Option<Vec<u8>>)> {
    compare_images_with_threshold(reference_path, test_path, DEFAULT_PIXEL_MATCH_THRESHOLD)
}

/// Compare images with a per-pixel perceptual tolerance between 0 and 100.
pub fn compare_images_with_threshold(
    reference_path: &Path,
    test_path: &Path,
    pixel_match_threshold: f64,
) -> Result<(CompareResult, Option<Vec<u8>>)> {
    if !pixel_match_threshold.is_finite() || !(0.0..=100.0).contains(&pixel_match_threshold) {
        anyhow::bail!("pixelMatchThreshold must be a finite value between 0 and 100");
    }
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
                alignment: DiffAlignment::default(),
                raw_mis_match_percentage: 0.0,
                mis_match_percentage: "0.00".to_string(),
                analysis_time_ms: start.elapsed().as_millis(),
            },
            None,
        ));
    }

    let normalized_threshold = pixel_match_threshold / 100.0;
    let max_delta = MAX_YIQ_DELTA * normalized_threshold * normalized_threshold;
    let mut reference = ImageView {
        pixels: &ref_pixels,
        width: ref_w,
        height: ref_h,
        gap: None,
    };
    let mut test = ImageView {
        pixels: &test_pixels,
        width: test_w,
        height: test_h,
        gap: None,
    };

    let out_w = ref_w.max(test_w);
    let out_h = ref_h.max(test_h);
    let total_pixels = out_w
        .checked_mul(out_h)
        .ok_or_else(|| anyhow!("Image dimensions are too large to compare safely"))?;
    if total_pixels > MAX_VISUAL_IMAGE_PIXELS as usize {
        anyhow::bail!(
            "Comparison canvas {}x{} exceeds the {} pixel limit",
            out_w,
            out_h,
            MAX_VISUAL_IMAGE_PIXELS
        );
    }
    if ref_h < test_h {
        reference.gap = find_vertical_gap(reference, test, max_delta);
    } else if test_h < ref_h {
        test.gap = find_vertical_gap(test, reference, max_delta);
    }
    let diff_capacity = total_pixels
        .checked_mul(4)
        .ok_or_else(|| anyhow!("Image dimensions are too large to build a diff safely"))?;
    let (diff_count, diff_pixels) =
        compare_and_render_diff(reference, test, max_delta, diff_capacity);

    let raw_mis_match = if total_pixels > 0 {
        (diff_count as f64 / total_pixels as f64) * 100.0
    } else {
        0.0
    };

    let diff_png = diff_pixels
        .map(|diff_pixels| lodepng::encode32(&diff_pixels, out_w, out_h))
        .transpose()?;
    let elapsed = start.elapsed().as_millis();

    Ok((
        CompareResult {
            is_same_dimensions,
            dimension_difference,
            ref_width: ref_w,
            ref_height: ref_h,
            test_width: test_w,
            test_height: test_h,
            alignment: DiffAlignment {
                reference: reference.gap,
                test: test.gap,
            },
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

    use crate::helpers::png::MAX_VISUAL_IMAGE_DIMENSION;

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut header = vec![137, 80, 78, 71, 13, 10, 26, 10];
        header.extend_from_slice(&13_u32.to_be_bytes());
        header.extend_from_slice(b"IHDR");
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header
    }

    fn patterned_image(width: usize, height: usize) -> Vec<RGBA8> {
        (0..width * height)
            .map(|index| {
                let row = index / width;
                let column = index % width;
                RGBA8::new(
                    ((row * 73 + column * 17) % 256) as u8,
                    ((row * 151 + column * 43) % 256) as u8,
                    ((row * 199 + column * 97) % 256) as u8,
                    255,
                )
            })
            .collect()
    }

    #[test]
    fn diff_alignment_rows_preserve_boundaries_and_reject_inconsistent_geometry() {
        let gap = VerticalGap {
            start: 24,
            height: 5,
        };
        assert_eq!(gap.for_image(128, 133), Some(gap));
        for (image_height, diff_height) in [(128, 128), (20, 25), (usize::MAX, 4)] {
            assert_eq!(gap.for_image(image_height, diff_height), None);
        }
        assert!(
            VerticalGap {
                start: 0,
                height: 0
            }
            .for_image(128, 128)
            .is_none()
        );
        for row in [0., 23.5, 24., 24.5, 127.5] {
            let aligned = gap.to_aligned_row(row);
            assert_eq!(aligned, if row < 24. { row } else { row + 5. });
            assert_eq!(gap.to_image_row(aligned), Some(row));
        }
        for row in [24., 24.5, 28.999] {
            assert_eq!(gap.to_image_row(row), None);
        }
        assert_eq!(gap.to_image_row(29.), Some(24.));
    }

    #[test]
    fn inserted_rows_are_a_band_without_hiding_other_changes() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 128;
        let gap_start = 24;
        let gap_height = 5;
        let mut reference = patterned_image(width, height);
        reference[5 * width + 3] = RGBA8::new(255, 255, 255, 255);
        reference[80 * width + 3] = RGBA8::new(255, 255, 255, 255);
        let mut test = reference[..gap_start * width].to_vec();
        test.extend(vec![RGBA8::new(0, 0, 0, 255); gap_height * width]);
        test.extend_from_slice(&reference[gap_start * width..]);
        test[5 * width + 3] = RGBA8::new(0, 0, 0, 255);
        test[(80 + gap_height) * width + 3] = RGBA8::new(0, 0, 0, 255);
        fs::write(
            &reference_path,
            lodepng::encode32(&reference, width, height).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&test, width, height + gap_height).unwrap(),
        )
        .unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert!(!result.is_same_dimensions);
        assert_eq!(result.dimension_difference, (0, gap_height as i64));
        assert_eq!(
            result.alignment,
            DiffAlignment {
                reference: Some(VerticalGap {
                    start: gap_start,
                    height: gap_height,
                }),
                test: None,
            }
        );
        let expected = (gap_height * width + 2) as f64 / test.len() as f64 * 100.0;
        assert!((result.raw_mis_match_percentage - expected).abs() < 1e-10);
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        assert_eq!((diff.width, diff.height), (width, height + gap_height));
        let pink = RGBA8::new(255, 0, 255, 200);
        assert!(
            diff.buffer[gap_start * width..(gap_start + gap_height) * width]
                .iter()
                .all(|pixel| *pixel == pink)
        );
        assert_eq!(diff.buffer[5 * width + 3], pink);
        assert_eq!(diff.buffer[(80 + gap_height) * width + 3], pink);
        let original = reference[100 * width];
        assert_eq!(
            diff.buffer[(100 + gap_height) * width],
            RGBA8::new(original.r / 3, original.g / 3, original.b / 3, original.a)
        );
    }

    #[test]
    fn removed_rows_are_a_band_without_hiding_other_changes() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 128;
        let gap_start = 24;
        let gap_height = 5;
        let mut reference = patterned_image(width, height);
        reference[5 * width + 3] = RGBA8::new(255, 255, 255, 255);
        reference[80 * width + 3] = RGBA8::new(255, 255, 255, 255);
        let mut test = reference[..gap_start * width].to_vec();
        test.extend_from_slice(&reference[(gap_start + gap_height) * width..]);
        test[5 * width + 3] = RGBA8::new(0, 0, 0, 255);
        test[(80 - gap_height) * width + 3] = RGBA8::new(0, 0, 0, 255);
        fs::write(
            &reference_path,
            lodepng::encode32(&reference, width, height).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&test, width, height - gap_height).unwrap(),
        )
        .unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert!(!result.is_same_dimensions);
        assert_eq!(result.dimension_difference, (0, -(gap_height as i64)));
        assert_eq!(
            result.alignment,
            DiffAlignment {
                reference: None,
                test: Some(VerticalGap {
                    start: gap_start,
                    height: gap_height,
                }),
            }
        );
        let expected = (gap_height * width + 2) as f64 / reference.len() as f64 * 100.0;
        assert!((result.raw_mis_match_percentage - expected).abs() < 1e-10);
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        assert_eq!((diff.width, diff.height), (width, height));
        let pink = RGBA8::new(255, 0, 255, 200);
        assert!(
            diff.buffer[gap_start * width..(gap_start + gap_height) * width]
                .iter()
                .all(|pixel| *pixel == pink)
        );
        assert_eq!(diff.buffer[5 * width + 3], pink);
        assert_eq!(diff.buffer[80 * width + 3], pink);
        let original = reference[100 * width];
        assert_eq!(
            diff.buffer[100 * width],
            RGBA8::new(original.r / 3, original.g / 3, original.b / 3, original.a)
        );
    }

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
        assert_eq!(result.alignment, DiffAlignment::default());
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

    #[test]
    fn default_threshold_highlights_subtle_background_changes() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        fs::write(
            &reference_path,
            lodepng::encode32(&[RGBA8::new(255, 255, 255, 255); 9], 3, 3).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&[RGBA8::new(230, 240, 237, 255); 9], 3, 3).unwrap(),
        )
        .unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert_eq!(result.raw_mis_match_percentage, 100.0);
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        assert!(
            diff.buffer
                .iter()
                .all(|pixel| *pixel == RGBA8::new(255, 0, 255, 200))
        );

        for (threshold, expected_mismatch) in
            [(0.0, 100.0), (5.0, 100.0), (10.0, 0.0), (100.0, 0.0)]
        {
            let (result, diff) =
                compare_images_with_threshold(&reference_path, &test_path, threshold).unwrap();
            assert_eq!(result.raw_mis_match_percentage, expected_mismatch);
            assert_eq!(diff.is_some(), expected_mismatch > 0.0);
        }
    }

    #[test]
    fn pixel_match_threshold_rejects_invalid_values_before_reading_images() {
        for threshold in [
            -1.0,
            -0.01,
            100.01,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let error = compare_images_with_threshold(
                Path::new("missing-reference.png"),
                Path::new("missing-test.png"),
                threshold,
            )
            .err()
            .expect("invalid pixel tolerance must fail");
            assert!(
                error
                    .to_string()
                    .contains("pixelMatchThreshold must be a finite value between 0 and 100")
            );
        }
    }

    #[test]
    fn test_compare_ignores_sub_threshold_pixel_noise() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        fs::write(
            &reference_path,
            lodepng::encode32(&[100_u8, 100, 100, 255], 1, 1).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&[101_u8, 101, 101, 255], 1, 1).unwrap(),
        )
        .unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert_eq!(result.raw_mis_match_percentage, 0.0);
        assert!(diff.is_none());
    }

    #[test]
    fn oversized_pngs_are_rejected_before_decode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized.png");
        fs::write(
            &path,
            png_header(MAX_VISUAL_IMAGE_DIMENSION, MAX_VISUAL_IMAGE_DIMENSION),
        )
        .unwrap();

        let error = load_png(&path).unwrap_err();

        assert!(error.to_string().contains("pixel limit"));
    }

    #[test]
    fn comparison_rejects_an_oversized_union_canvas() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let side = MAX_VISUAL_IMAGE_DIMENSION as usize;
        let pixels = vec![0_u8; side * 4];
        fs::write(
            &reference_path,
            lodepng::encode32(&pixels, side, 1).unwrap(),
        )
        .unwrap();
        fs::write(&test_path, lodepng::encode32(&pixels, 1, side).unwrap()).unwrap();

        let error = match compare_images(&reference_path, &test_path) {
            Ok(_) => panic!("comparison unexpectedly accepted an oversized union canvas"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("Comparison canvas"));
    }
}
