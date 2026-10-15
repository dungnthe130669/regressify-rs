use std::{fs::File, io::Read, path::Path, time::Instant};

use anyhow::{Result, anyhow};
use lodepng::Image;
use rgb::RGBA8;
use serde::{Deserialize, Serialize};

use crate::{
    config::DEFAULT_PIXEL_MATCH_THRESHOLD,
    helpers::png::{MAX_VISUAL_IMAGE_PIXELS, read_bounded_png_dimensions_from_reader},
};

const MAX_PNG_FILE_BYTES: u64 = 128 * 1024 * 1024;

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

fn read_png_bytes(path: &Path) -> Result<(Vec<u8>, usize, usize)> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_PNG_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PNG_FILE_BYTES {
        anyhow::bail!(
            "PNG '{}' exceeds the {} MiB file size limit",
            path.display(),
            MAX_PNG_FILE_BYTES / (1024 * 1024)
        );
    }
    let (expected_width, expected_height) =
        read_bounded_png_dimensions_from_reader(bytes.as_slice(), path)?;
    Ok((bytes, expected_width, expected_height))
}

fn decode_png_bytes(
    bytes: &[u8],
    path: &Path,
    expected_width: usize,
    expected_height: usize,
) -> Result<(Vec<RGBA8>, usize, usize)> {
    let mut decoder = lodepng::Decoder::new();
    decoder.color_convert(true);
    decoder.read_text_chunks(false);
    decoder.info_raw_mut().colortype = lodepng::ColorType::RGBA;
    decoder.info_raw_mut().set_bitdepth(8);

    let decoded = decoder.decode(bytes)?;
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
            Ok((img.buffer, width, height))
        }
        _ => Err(anyhow!(
            "Failed to decode PNG as RGBA: {:?}",
            path.display()
        )),
    }
}

/// Load a PNG file as RGBA pixels.
pub(super) fn load_png(path: &Path) -> Result<(Vec<RGBA8>, usize, usize)> {
    let (bytes, expected_width, expected_height) = read_png_bytes(path)?;
    decode_png_bytes(&bytes, path, expected_width, expected_height)
}

// ─── Perceptual comparison (pixelmatch-style) ────────────────────────────────

/// Maximum possible YIQ color delta squared (pixelmatch constant).
const MAX_YIQ_DELTA: f64 = 35215.0;
const MIN_ALIGNED_ROWS: usize = 32;
const MIN_TAIL_ALIGNED_ROWS: usize = 4;

#[inline]
fn blend_channel(c: f64, a: f64) -> f64 {
    255.0 + (c - 255.0) * a
}

/// Signed perceptual color delta (YIQ) between pixel `k` in `a` and `m` in `b`.
/// When `y_only` is true, returns only the brightness delta (used for AA detection).
#[inline]
fn color_delta(a: &[RGBA8], b: &[RGBA8], k: usize, m: usize, y_only: bool) -> f64 {
    let p1 = a[k];
    let p2 = b[m];
    if p1 == p2 {
        return 0.0;
    }

    let (mut r1, mut g1, mut b1, a1) = (p1.r as f64, p1.g as f64, p1.b as f64, p1.a as f64);
    let (mut r2, mut g2, mut b2, a2) = (p2.r as f64, p2.g as f64, p2.b as f64, p2.a as f64);

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
    let center = img[y1 * w + x1];
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
            if center == img[y * w + x] {
                zeroes += 1;
                if zeroes > 2 {
                    return true;
                }
            }
        }
    }
    false
}

/// Determine whether the pixel at `(x1, y1)` in `img` (aligned against
/// `(x1, other_y1)` in `other`) is likely anti-aliasing rather than a real
/// difference (pixelmatch algorithm).
fn is_antialiased(
    img: &[RGBA8],
    x1: usize,
    y1: usize,
    w: usize,
    h: usize,
    other: &[RGBA8],
    other_y1: usize,
    other_h: usize,
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

    let map_other_y = |y: usize| -> Option<usize> {
        let offset = y as isize - y1 as isize;
        let mapped = other_y1 as isize + offset;
        (0 <= mapped && (mapped as usize) < other_h).then_some(mapped as usize)
    };

    (has_many_siblings(img, min_x, min_y, w, h)
        && map_other_y(min_y)
            .is_some_and(|other_min_y| has_many_siblings(other, min_x, other_min_y, w, other_h)))
        || (has_many_siblings(img, max_x, max_y, w, h)
            && map_other_y(max_y).is_some_and(|other_max_y| {
                has_many_siblings(other, max_x, other_max_y, w, other_h)
            }))
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

impl<'a> ImageView<'a> {
    #[inline]
    fn source_row(self, row: usize) -> Option<usize> {
        let source_row = match self.gap {
            Some(gap) if row >= gap.start => {
                if row < gap.start + gap.height {
                    return None;
                }
                row - gap.height
            }
            _ => row,
        };
        (source_row < self.height).then_some(source_row)
    }

    #[inline]
    fn row_slice(self, source_row: usize) -> &'a [RGBA8] {
        let start = source_row * self.width;
        &self.pixels[start..start + self.width]
    }
}

fn row_difference(
    shorter: ImageView<'_>,
    taller: ImageView<'_>,
    row: usize,
    taller_row: usize,
    max_delta: f64,
) -> usize {
    if shorter.row_slice(row) == taller.row_slice(taller_row) {
        return 0;
    }
    let shorter_offset = row * shorter.width;
    let taller_offset = taller_row * taller.width;
    (0..shorter.width)
        .filter(|&column| {
            color_delta(
                shorter.pixels,
                taller.pixels,
                shorter_offset + column,
                taller_offset + column,
                false,
            )
            .abs()
                > max_delta
        })
        .count()
}

/// Find a single vertical insertion/deletion gap of size `taller.height - shorter.height`
/// in `shorter` that maximizes aligned row matches below the gap without letting
/// unrelated top-of-page visual changes pull the gap start across intact middle rows.
fn find_vertical_gap(
    shorter: ImageView<'_>,
    taller: ImageView<'_>,
    max_delta: f64,
) -> Option<VerticalGap> {
    if shorter.width == 0 || shorter.width != taller.width || shorter.height >= taller.height {
        return None;
    }
    let height = taller.height - shorter.height;

    // Rows in `0..top_equal` are already byte-identical unshifted (`unshifted == 0`),
    // so scanning into that prefix can never increase `gain` or `supporting_rows`.
    let top_equal = (0..shorter.height)
        .take_while(|&row| shorter.row_slice(row) == taller.row_slice(row))
        .count();
    if top_equal == shorter.height {
        return None;
    }

    let max_aligned_diff = shorter.width / 100;
    let max_plausible_diff = (shorter.width / 10).max(1);
    let mut gain = 0_i64;
    let mut supporting_rows = 0_usize;
    let mut contested_rows = 0_usize;
    let mut best_gain = 0_i64;
    let mut best_start = shorter.height;
    let mut best_support = 0_usize;
    let mut best_contested = 0_usize;

    for row in (top_equal..shorter.height).rev() {
        let unshifted = row_difference(shorter, taller, row, row, max_delta);
        let shifted = row_difference(shorter, taller, row, row + height, max_delta);
        if unshifted > 0 || shifted > 0 {
            contested_rows += 1;
        }
        let shifted_aligned = shifted <= max_aligned_diff && unshifted > shifted;
        let shifted_plausible = shifted <= max_plausible_diff
            && unshifted >= shifted.saturating_add(max_plausible_diff);
        let unshifted_aligned = unshifted <= max_aligned_diff;

        if shifted_aligned || shifted_plausible {
            supporting_rows += 1;
        }

        if shifted <= max_plausible_diff {
            gain += unshifted as i64 - shifted as i64;
        } else if unshifted_aligned {
            // Shifting this row breaks a clean unshifted match without producing
            // a plausible shifted match; penalize by the full row width so noise
            // above an intact sparse section cannot pull `best_start` upward.
            gain -= (shorter.width as i64).max(shifted as i64 - unshifted as i64);
        } else {
            // Neither unshifted nor shifted aligns this row (in-place visual change);
            // never let accidental partial overlap add positive shift gain.
            gain += (unshifted as i64 - shifted as i64).min(0);
        }

        if gain > best_gain {
            best_gain = gain;
            best_start = row;
            best_support = supporting_rows;
            best_contested = contested_rows;
        }
    }

    let has_tail_support = best_support >= MIN_TAIL_ALIGNED_ROWS
        && best_contested > 0
        && best_support * 4 >= best_contested * 3
        && best_gain >= (best_contested * shorter.width.min(8)) as i64;
    (best_gain > 0 && (best_support >= MIN_ALIGNED_ROWS || has_tail_support)).then_some(
        VerticalGap {
            start: best_start,
            height,
        },
    )
}

fn matching_reference_pixel(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    x: usize,
    ref_row: usize,
    test_row: usize,
    max_delta: f64,
) -> Option<usize> {
    if x >= reference.width || x >= test.width {
        return None;
    }
    let reference_index = ref_row * reference.width + x;
    let test_index = test_row * test.width + x;

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

    let same_width = reference.width == test.width;
    (same_width
        && (is_antialiased(
            reference.pixels,
            x,
            ref_row,
            reference.width,
            reference.height,
            test.pixels,
            test_row,
            test.height,
        ) || is_antialiased(
            test.pixels,
            x,
            test_row,
            test.width,
            test.height,
            reference.pixels,
            ref_row,
            reference.height,
        )))
    .then_some(reference_index)
}

#[inline]
fn append_dimmed_pixel(pixels: &mut Vec<u8>, pixel: RGBA8) {
    pixels.extend_from_slice(&[pixel.r / 3, pixel.g / 3, pixel.b / 3, pixel.a]);
}

fn append_dimmed_slice(pixels: &mut Vec<u8>, source: &[RGBA8]) {
    for &pixel in source {
        append_dimmed_pixel(pixels, pixel);
    }
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
    let same_width = reference.width == width && test.width == width;
    let mut count = 0_u64;
    let mut diff_pixels: Option<Vec<u8>> = None;

    for y in 0..height {
        let ref_row = reference.source_row(y);
        let test_row = test.source_row(y);
        if same_width
            && let (Some(r_row), Some(t_row)) = (ref_row, test_row)
            && reference.row_slice(r_row) == test.row_slice(t_row)
        {
            if let Some(pixels) = &mut diff_pixels {
                append_dimmed_slice(pixels, reference.row_slice(r_row));
            }
            continue;
        }

        for x in 0..width {
            let matched = match (ref_row, test_row) {
                (Some(r_row), Some(t_row)) => {
                    matching_reference_pixel(reference, test, x, r_row, t_row, max_delta)
                }
                _ => None,
            };
            match matched {
                Some(reference_index) => {
                    if let Some(pixels) = &mut diff_pixels {
                        append_dimmed_pixel(pixels, reference.pixels[reference_index]);
                    }
                }
                None => {
                    count += 1;
                    let pixels = diff_pixels.get_or_insert_with(|| {
                        let mut pixels = Vec::with_capacity(capacity);
                        append_dimmed_slice(&mut pixels, &reference.pixels[..y * width + x]);
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

    let (ref_bytes, ref_expected_w, ref_expected_h) = read_png_bytes(reference_path)?;
    let (test_bytes, test_expected_w, test_expected_h) = read_png_bytes(test_path)?;
    let (ref_pixels, ref_w, ref_h) =
        decode_png_bytes(&ref_bytes, reference_path, ref_expected_w, ref_expected_h)?;

    // Capture normalization strips non-essential PNG chunks, so unchanged
    // captures are byte-identical on disk. After verifying `ref_bytes` is a
    // valid PNG, skip the second decode when the raw bytes match.
    if ref_bytes == test_bytes {
        return Ok((
            CompareResult {
                is_same_dimensions: true,
                dimension_difference: (0, 0),
                ref_width: ref_w,
                ref_height: ref_h,
                test_width: ref_w,
                test_height: ref_h,
                alignment: DiffAlignment::default(),
                raw_mis_match_percentage: 0.0,
                mis_match_percentage: "0.00".to_string(),
                analysis_time_ms: start.elapsed().as_millis(),
            },
            None,
        ));
    }
    drop(ref_bytes);
    let (test_pixels, test_w, test_h) =
        decode_png_bytes(&test_bytes, test_path, test_expected_w, test_expected_h)?;
    drop(test_bytes);

    let is_same_dimensions = ref_w == test_w && ref_h == test_h;
    let dimension_difference = (test_w as i64 - ref_w as i64, test_h as i64 - ref_h as i64);

    // Decoding normalizes PNG encoding details such as palette layout and
    // metadata. Avoid allocating and encoding a full-page diff image when
    // rendered pixels are identical despite different PNG encodings.
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

    #[test]
    fn large_middle_insertions_above_512_rows_align_into_a_single_band() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 240;
        let gap_start = 90;
        let gap_height = 640;
        let reference = patterned_image(width, height);
        let mut test = reference[..gap_start * width].to_vec();
        test.extend(vec![RGBA8::new(12, 34, 56, 255); gap_height * width]);
        test.extend_from_slice(&reference[gap_start * width..]);
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
        let expected = (gap_height * width) as f64 / test.len() as f64 * 100.0;
        assert!((result.raw_mis_match_percentage - expected).abs() < 1e-10);
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        let pink = RGBA8::new(255, 0, 255, 200);
        assert!(
            diff.buffer[gap_start * width..(gap_start + gap_height) * width]
                .iter()
                .all(|pixel| *pixel == pink)
        );
        assert!(
            diff.buffer[(gap_start + gap_height) * width..]
                .iter()
                .all(|pixel| *pixel != pink)
        );
    }

    #[test]
    fn top_of_page_changes_do_not_pull_gap_start_across_intact_sparse_middle_sections() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 64;
        let height = 160;
        let gap_start = 100;
        let gap_height = 20;
        let white = RGBA8::new(255, 255, 255, 255);

        let mut reference = vec![white; width * height];
        // Top hero banner (rows 0..40): solid dark blue in reference.
        for row in 0..40 {
            for col in 0..width {
                reference[row * width + col] = RGBA8::new(20, 40, 180, 255);
            }
        }
        // Sparse middle section (rows 40..100): white background with a 2px accent column
        // that differs from row to row so shifting by `gap_height` breaks those 2 pixels.
        for row in 40..gap_start {
            let accent = RGBA8::new(((row * 67) % 200) as u8, 10, 90, 255);
            reference[row * width + 4] = accent;
            reference[row * width + 5] = accent;
        }
        // Footer section below insertion (rows 100..160): rich patterned content.
        let patterned = patterned_image(width, height - gap_start);
        reference[gap_start * width..].copy_from_slice(&patterned);

        let mut test = reference[..gap_start * width].to_vec();
        // Top hero banner changed in place in test to a bright orange gradient
        // (partially closer to white than dark blue, so `unshifted > shifted` on rows 0..40).
        for row in 0..40 {
            for col in 0..width {
                test[row * width + col] = RGBA8::new(240, 180, 120, 255);
            }
        }
        // Insert new block at `gap_start = 100`.
        test.extend(vec![RGBA8::new(0, 0, 0, 255); gap_height * width]);
        test.extend_from_slice(&reference[gap_start * width..]);

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
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        let pink = RGBA8::new(255, 0, 255, 200);
        // Intact sparse middle section (rows 40..100) must remain completely free of diffs.
        assert!(
            diff.buffer[40 * width..gap_start * width]
                .iter()
                .all(|pixel| *pixel != pink)
        );
    }

    #[test]
    fn near_bottom_insertions_with_short_tails_still_align_when_tail_rows_match() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 48;
        let gap_start = 36;
        let gap_height = 6;
        let reference = patterned_image(width, height);
        let mut test = reference[..gap_start * width].to_vec();
        test.extend(vec![RGBA8::new(0, 0, 0, 255); gap_height * width]);
        test.extend_from_slice(&reference[gap_start * width..]);
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

        let (result, _) = compare_images(&reference_path, &test_path).unwrap();

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
        let expected = (gap_height * width) as f64 / test.len() as f64 * 100.0;
        assert!((result.raw_mis_match_percentage - expected).abs() < 1e-10);
    }

    #[test]
    fn antialiased_pixels_on_vertically_shifted_rows_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 80;
        let gap_start = 20;
        let gap_height = 6;
        let mut reference = patterned_image(width, height);
        // Place a 5x5 high-contrast edge block at rows 50..55, cols 10..15 (below the gap)
        // where (12, 52) is an anti-aliased transition pixel between dark and light regions.
        let dark = RGBA8::new(0, 0, 0, 255);
        let light = RGBA8::new(255, 255, 255, 255);
        for row in 50..55 {
            for col in 10..15 {
                reference[row * width + col] = if col < 12 || (col == 12 && row < 52) {
                    dark
                } else {
                    light
                };
            }
        }
        reference[52 * width + 12] = RGBA8::new(90, 90, 90, 255);

        let mut test = reference[..gap_start * width].to_vec();
        test.extend(vec![RGBA8::new(10, 20, 30, 255); gap_height * width]);
        test.extend_from_slice(&reference[gap_start * width..]);
        // Shift the anti-aliased transition pixel slightly on the aligned row `52 + gap_height`.
        test[(52 + gap_height) * width + 12] = RGBA8::new(160, 160, 160, 255);

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

        let (result, diff) = compare_images_with_threshold(&reference_path, &test_path, 0.0)
            .expect("comparison with vertical gap and AA pixel should succeed");

        assert_eq!(
            result.alignment.reference,
            Some(VerticalGap {
                start: gap_start,
                height: gap_height,
            })
        );
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        assert_ne!(
            diff.buffer[(52 + gap_height) * width + 12],
            RGBA8::new(255, 0, 255, 200),
            "anti-aliased pixel on a vertically shifted row must not be flagged as a mismatch"
        );
    }

    #[test]
    fn middle_insertion_aligns_footer_with_whitespace_padding_and_minor_column_changes() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 40;
        let height = 120;
        let gap_start = 40;
        let gap_height = 10;
        let white = RGBA8::new(255, 255, 255, 255);

        let mut reference = patterned_image(width, height);
        // Make the bottom 68 rows (52..120) solid white padding so only 12 rows (40..52)
        // below `gap_start` have non-blank content.
        reference[52 * width..].fill(white);

        let mut test = reference[..gap_start * width].to_vec();
        test.extend(vec![RGBA8::new(0, 0, 0, 255); gap_height * width]);
        test.extend_from_slice(&reference[gap_start * width..]);
        // Also modify 2 columns (5% of width) on every content row below the insertion.
        for row in (gap_start + gap_height)..(52 + gap_height) {
            test[row * width] = RGBA8::new(250, 0, 0, 255);
            test[row * width + 1] = RGBA8::new(250, 0, 0, 255);
        }

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

        let (result, _) = compare_images(&reference_path, &test_path).unwrap();

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
    }
}
