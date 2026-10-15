use std::{fs::File, io::Read, path::Path, time::Instant};

use anyhow::{Result, anyhow};
use lodepng::Image;
use rgb::RGBA8;
use serde::{Deserialize, Serialize};

use crate::{
    config::DEFAULT_PIXEL_MATCH_THRESHOLD,
    helpers::png::{
        MAX_VISUAL_IMAGE_DIMENSION, MAX_VISUAL_IMAGE_PIXELS,
        read_bounded_png_dimensions_from_reader,
    },
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
const MAX_ALIGNMENT_DEPTH: usize = 4;
const MAX_SIGNATURE_SAMPLES: usize = 32;
const MAX_HASH_FREQUENCY: usize = 64;

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

pub fn validate_vertical_gaps(
    gaps: &[VerticalGap],
    image_height: usize,
    diff_height: usize,
) -> Option<Vec<VerticalGap>> {
    if gaps.is_empty() {
        return None;
    }
    let mut total_gap_height = 0_usize;
    let mut previous_start = None;
    for &gap in gaps {
        if gap.height == 0 || gap.start > image_height {
            return None;
        }
        if previous_start.is_some_and(|prev| gap.start <= prev) {
            return None;
        }
        previous_start = Some(gap.start);
        total_gap_height = total_gap_height.checked_add(gap.height)?;
    }
    (image_height.checked_add(total_gap_height) == Some(diff_height)).then(|| gaps.to_vec())
}

pub fn gaps_to_aligned_row(gaps: &[VerticalGap], row: f32) -> f32 {
    let shift: usize = gaps
        .iter()
        .filter(|gap| row >= gap.start as f32)
        .map(|gap| gap.height)
        .sum();
    row + shift as f32
}

pub fn gaps_to_image_row(gaps: &[VerticalGap], row: f32) -> Option<f32> {
    let mut shift = 0.0_f32;
    for &gap in gaps {
        let aligned_start = gap.start as f32 + shift;
        if row < aligned_start {
            return Some(row - shift);
        }
        let gap_height = gap.height as f32;
        if row < aligned_start + gap_height {
            return None;
        }
        shift += gap_height;
    }
    Some(row - shift)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffAlignment {
    pub reference: Option<VerticalGap>,
    pub test: Option<VerticalGap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_gaps: Vec<VerticalGap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_gaps: Vec<VerticalGap>,
}

impl DiffAlignment {
    pub fn from_gaps(reference_gaps: Vec<VerticalGap>, test_gaps: Vec<VerticalGap>) -> Self {
        let single_side_at_most_one = reference_gaps.len() <= 1
            && test_gaps.len() <= 1
            && (reference_gaps.is_empty() || test_gaps.is_empty());
        Self {
            reference: reference_gaps.first().copied(),
            test: test_gaps.first().copied(),
            reference_gaps: if single_side_at_most_one {
                Vec::new()
            } else {
                reference_gaps
            },
            test_gaps: if single_side_at_most_one {
                Vec::new()
            } else {
                test_gaps
            },
        }
    }

    pub fn reference_gaps(&self) -> &[VerticalGap] {
        if !self.reference_gaps.is_empty() {
            &self.reference_gaps
        } else if let Some(gap) = &self.reference {
            std::slice::from_ref(gap)
        } else {
            &[]
        }
    }

    pub fn test_gaps(&self) -> &[VerticalGap] {
        if !self.test_gaps.is_empty() {
            &self.test_gaps
        } else if let Some(gap) = &self.test {
            std::slice::from_ref(gap)
        } else {
            &[]
        }
    }

    pub fn reference_for_image(
        &self,
        image_height: usize,
        diff_height: usize,
    ) -> Option<Vec<VerticalGap>> {
        validate_vertical_gaps(self.reference_gaps(), image_height, diff_height)
    }

    pub fn test_for_image(
        &self,
        image_height: usize,
        diff_height: usize,
    ) -> Option<Vec<VerticalGap>> {
        validate_vertical_gaps(self.test_gaps(), image_height, diff_height)
    }

    pub fn is_valid(&self) -> bool {
        if self.reference_gaps.is_empty()
            && self.test_gaps.is_empty()
            && self.reference.is_some()
            && self.test.is_some()
        {
            return false;
        }
        if !self.reference_gaps.is_empty() && self.reference != self.reference_gaps.first().copied()
        {
            return false;
        }
        if !self.test_gaps.is_empty() && self.test != self.test_gaps.first().copied() {
            return false;
        }
        let aligned_intervals = |gaps: &[VerticalGap]| -> Option<Vec<(u64, u64)>> {
            let mut intervals = Vec::with_capacity(gaps.len());
            let mut shift = 0_u64;
            let mut previous_start = None;
            for &gap in gaps {
                let start = u64::from(u32::try_from(gap.start).ok()?);
                let height = u64::from(u32::try_from(gap.height).ok()?);
                if height == 0 || previous_start.is_some_and(|prev| start <= prev) {
                    return None;
                }
                previous_start = Some(start);
                let aligned_start = start.checked_add(shift)?;
                let aligned_end = aligned_start.checked_add(height)?;
                intervals.push((aligned_start, aligned_end));
                shift = shift.checked_add(height)?;
            }
            Some(intervals)
        };
        let Some(ref_intervals) = aligned_intervals(self.reference_gaps()) else {
            return false;
        };
        let Some(test_intervals) = aligned_intervals(self.test_gaps()) else {
            return false;
        };
        !ref_intervals.iter().any(|&(r_start, r_end)| {
            test_intervals
                .iter()
                .any(|&(t_start, t_end)| r_start < t_end && t_start < r_end)
        })
    }
}

#[derive(Clone, Copy)]
struct ImageView<'a> {
    pixels: &'a [RGBA8],
    width: usize,
    height: usize,
    gaps: &'a [VerticalGap],
}

impl<'a> ImageView<'a> {
    #[inline]
    fn source_row(self, row: usize) -> Option<usize> {
        let mut shift = 0_usize;
        for &gap in self.gaps {
            let aligned_start = gap.start + shift;
            if row < aligned_start {
                let source_row = row - shift;
                return (source_row < self.height).then_some(source_row);
            }
            if row < aligned_start + gap.height {
                return None;
            }
            shift += gap.height;
        }
        let source_row = row - shift;
        (source_row < self.height).then_some(source_row)
    }

    #[inline]
    fn aligned_height(self) -> Option<usize> {
        self.gaps
            .iter()
            .try_fold(self.height, |acc, gap| acc.checked_add(gap.height))
    }

    #[inline]
    fn row_slice(self, source_row: usize) -> &'a [RGBA8] {
        let start = source_row * self.width;
        &self.pixels[start..start + self.width]
    }

    #[inline]
    fn sub_rows(self, start_row: usize, end_row: usize) -> Self {
        let start = start_row * self.width;
        let end = end_row * self.width;
        Self {
            pixels: &self.pixels[start..end],
            width: self.width,
            height: end_row.saturating_sub(start_row),
            gaps: &[],
        }
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

#[inline]
fn row_shift_gain(width: usize, unshifted: usize, shifted: usize) -> (i64, bool) {
    let max_aligned_diff = width / 100;
    let max_plausible_diff = (width / 2).max(1);
    let min_plausible_gain = (width / 10).max(1);
    let shifted_aligned = shifted <= max_aligned_diff && unshifted > shifted;
    let shifted_plausible =
        shifted <= max_plausible_diff && unshifted >= shifted.saturating_add(min_plausible_gain);
    let unshifted_aligned = unshifted <= max_aligned_diff;

    if unshifted_aligned && shifted > unshifted {
        // Shifting this row breaks a clean unshifted match; penalize by the
        // full row width so unrelated changes above an intact sparse middle
        // section cannot pull `best_start` upward.
        (
            -((width as i64).max(shifted as i64 - unshifted as i64)),
            false,
        )
    } else if shifted_aligned || shifted_plausible {
        (unshifted as i64 - shifted as i64, true)
    } else {
        // Neither unshifted nor shifted aligns this row (in-place visual change);
        // never let accidental partial overlap add positive shift gain.
        ((unshifted as i64 - shifted as i64).min(0), false)
    }
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
        let (delta, supports_shift) = row_shift_gain(shorter.width, unshifted, shifted);
        if supports_shift {
            supporting_rows += 1;
        }
        gain += delta;

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

#[derive(Clone, Copy)]
struct RowSignature {
    full: u64,
    left: u32,
    right: u32,
    full_informative: bool,
    left_informative: bool,
    right_informative: bool,
}

#[derive(Clone, Copy, Debug)]
struct AlignedAnchor {
    ref_start: usize,
    test_start: usize,
    len: usize,
}

impl AlignedAnchor {
    #[inline]
    fn shift(self) -> isize {
        self.test_start as isize - self.ref_start as isize
    }
}

fn compute_row_signatures(image: ImageView<'_>) -> Vec<RowSignature> {
    let samples = image.width.min(MAX_SIGNATURE_SAMPLES).max(1);
    let half = (samples / 2).max(1);
    let mut signatures = Vec::with_capacity(image.height);
    let mut prev_full = None;

    for row in 0..image.height {
        let slice = image.row_slice(row);
        let mut full = 0xcbf2_9ce4_8422_2325_u64;
        let mut left = 0x811c_9dc5_u32;
        let mut right = 0x811c_9dc5_u32;
        let mut first_full = None;
        let mut first_left = None;
        let mut first_right = None;
        let mut full_varied = false;
        let mut left_varied = false;
        let mut right_varied = false;

        for sample_idx in 0..samples {
            let col = (sample_idx * image.width) / samples;
            let p = slice[col];
            let quantized = ((p.r as u16 >> 3) << 11)
                | ((p.g as u16 >> 3) << 6)
                | ((p.b as u16 >> 3) << 1)
                | (p.a as u16 >> 7);
            if let Some(first) = first_full {
                if first != quantized {
                    full_varied = true;
                }
            } else {
                first_full = Some(quantized);
            }
            full = (full ^ u64::from(quantized)).wrapping_mul(0x0100_0000_01b3);
            if sample_idx < half {
                if let Some(first) = first_left {
                    if first != quantized {
                        left_varied = true;
                    }
                } else {
                    first_left = Some(quantized);
                }
                left = (left ^ u32::from(quantized)).wrapping_mul(0x0100_0193);
            } else {
                if let Some(first) = first_right {
                    if first != quantized {
                        right_varied = true;
                    }
                } else {
                    first_right = Some(quantized);
                }
                right = (right ^ u32::from(quantized)).wrapping_mul(0x0100_0193);
            }
        }

        let full_informative = full_varied || prev_full.is_some_and(|prev| prev != full);
        prev_full = Some(full);
        signatures.push(RowSignature {
            full,
            left,
            right,
            full_informative,
            left_informative: left_varied,
            right_informative: right_varied,
        });
    }
    signatures
}

fn vote_bucket_positions(
    r_positions: &[usize],
    t_positions: &[usize],
    weight: usize,
    shift_votes: &mut std::collections::HashMap<isize, usize>,
) {
    if r_positions.is_empty() || t_positions.is_empty() {
        return;
    }
    if t_positions.len() <= MAX_HASH_FREQUENCY {
        for &r in r_positions {
            for &t in t_positions {
                *shift_votes.entry(t as isize - r as isize).or_default() += weight;
            }
        }
        return;
    }
    let t_len = t_positions.len();
    let r_len = r_positions.len();
    for (idx, &r) in r_positions.iter().enumerate() {
        let tail_center = (t_len + idx).saturating_sub(r_len);
        let ranges = [
            0..8.min(t_len),
            t_len.saturating_sub(8)..t_len,
            idx.saturating_sub(8)..(idx + 9).min(t_len),
            tail_center.saturating_sub(8)..(tail_center + 9).min(t_len),
        ];
        for (range_idx, range) in ranges.iter().enumerate() {
            for j in range.clone() {
                if ranges[..range_idx].iter().any(|prev| prev.contains(&j)) {
                    continue;
                }
                let t = t_positions[j];
                *shift_votes.entry(t as isize - r as isize).or_default() += weight;
            }
        }
    }
}

fn find_best_shifted_anchor(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    ref_sigs: &[RowSignature],
    test_sigs: &[RowSignature],
    r_range: std::ops::Range<usize>,
    t_range: std::ops::Range<usize>,
    max_delta: f64,
) -> Option<AlignedAnchor> {
    if r_range.len() < MIN_TAIL_ALIGNED_ROWS || t_range.len() < MIN_TAIL_ALIGNED_ROWS {
        return None;
    }

    let mut full_buckets: std::collections::HashMap<u64, (Vec<usize>, Vec<usize>)> =
        std::collections::HashMap::new();
    let mut left_buckets: std::collections::HashMap<u32, (Vec<usize>, Vec<usize>)> =
        std::collections::HashMap::new();
    let mut right_buckets: std::collections::HashMap<u32, (Vec<usize>, Vec<usize>)> =
        std::collections::HashMap::new();

    for r in r_range.clone() {
        let sig = ref_sigs[r];
        if sig.full_informative {
            full_buckets.entry(sig.full).or_default().0.push(r);
        }
        if sig.left_informative {
            left_buckets.entry(sig.left).or_default().0.push(r);
        }
        if sig.right_informative {
            right_buckets.entry(sig.right).or_default().0.push(r);
        }
    }

    for t in t_range.clone() {
        let sig = test_sigs[t];
        if sig.full_informative
            && let Some(bucket) = full_buckets.get_mut(&sig.full)
        {
            bucket.1.push(t);
        }
        if sig.left_informative
            && let Some(bucket) = left_buckets.get_mut(&sig.left)
        {
            bucket.1.push(t);
        }
        if sig.right_informative
            && let Some(bucket) = right_buckets.get_mut(&sig.right)
        {
            bucket.1.push(t);
        }
    }

    let mut shift_votes: std::collections::HashMap<isize, usize> = std::collections::HashMap::new();
    for (r_pos, t_pos) in full_buckets.values() {
        vote_bucket_positions(r_pos, t_pos, 2, &mut shift_votes);
    }
    for (r_pos, t_pos) in left_buckets.values() {
        vote_bucket_positions(r_pos, t_pos, 1, &mut shift_votes);
    }
    for (r_pos, t_pos) in right_buckets.values() {
        vote_bucket_positions(r_pos, t_pos, 1, &mut shift_votes);
    }

    let mut candidates: Vec<(isize, usize)> = shift_votes
        .into_iter()
        .filter(|&(_, votes)| votes >= MIN_TAIL_ALIGNED_ROWS)
        .collect();
    candidates.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.abs().cmp(&b.0.abs()))
            .then_with(|| b.0.cmp(&a.0))
    });
    candidates.truncate(8);

    let max_aligned_diff = reference.width / 100;
    type AnchorRank = (
        usize,
        usize,
        i64,
        usize,
        std::cmp::Reverse<usize>,
        bool,
        std::cmp::Reverse<usize>,
    );
    let mut best_anchor: Option<(AnchorRank, AlignedAnchor)> = None;

    for (shift, _) in candidates {
        let r_start = (t_range.start as isize - shift).max(r_range.start as isize) as usize;
        let r_end = (t_range.end as isize - shift).min(r_range.end as isize) as usize;
        if r_end <= r_start || r_end - r_start < MIN_TAIL_ALIGNED_ROWS {
            continue;
        }

        let mut run_start = None;
        let mut run_aligned = 0_usize;
        let mut run_closely_aligned = 0_usize;
        let mut run_informative = 0_usize;
        let mut run_gain = 0_i64;
        let mut bad_streak = 0_usize;
        let mut last_good_r = r_start;

        let mut consider_run = |start: usize,
                                end: usize,
                                aligned: usize,
                                closely_aligned: usize,
                                informative: usize,
                                gain: i64| {
            let len = end.saturating_sub(start);
            if len < MIN_TAIL_ALIGNED_ROWS
                || aligned < MIN_TAIL_ALIGNED_ROWS
                || closely_aligned < MIN_TAIL_ALIGNED_ROWS
                || informative < MIN_TAIL_ALIGNED_ROWS.min(len)
                || aligned * 4 < len * 3
                || gain <= 0
            {
                return;
            }
            let candidate = AlignedAnchor {
                ref_start: start,
                test_start: (start as isize + shift) as usize,
                len,
            };
            let rank: AnchorRank = (
                aligned,
                closely_aligned,
                gain,
                len,
                std::cmp::Reverse(shift.unsigned_abs()),
                shift >= 0,
                std::cmp::Reverse(start),
            );
            if best_anchor
                .as_ref()
                .is_none_or(|(best_rank, _)| rank > *best_rank)
            {
                best_anchor = Some((rank, candidate));
            }
        };

        for r in r_start..r_end {
            let t = (r as isize + shift) as usize;
            let shifted = row_difference(reference, test, r, t, max_delta);
            let unshifted = if shift == 0 {
                reference.width
            } else if r < test.height {
                row_difference(reference, test, r, r, max_delta)
            } else {
                reference.width
            };
            let (row_gain, is_good) = if shift == 0 {
                let good = shifted <= max_aligned_diff;
                (
                    if good {
                        (reference.width - shifted) as i64
                    } else {
                        -(shifted as i64)
                    },
                    good,
                )
            } else {
                row_shift_gain(reference.width, unshifted, shifted)
            };

            if is_good {
                if run_start.is_none() {
                    run_start = Some(r);
                    run_aligned = 0;
                    run_closely_aligned = 0;
                    run_informative = 0;
                    run_gain = 0;
                }
                run_aligned += 1;
                if shifted <= max_aligned_diff {
                    run_closely_aligned += 1;
                }
                if ref_sigs[r].full_informative || test_sigs[t].full_informative {
                    run_informative += 1;
                }
                run_gain += row_gain;
                last_good_r = r + 1;
                bad_streak = 0;
            } else if let Some(start) = run_start {
                bad_streak += 1;
                run_gain += row_gain;
                if bad_streak > 2 {
                    consider_run(
                        start,
                        last_good_r,
                        run_aligned,
                        run_closely_aligned,
                        run_informative,
                        run_gain,
                    );
                    run_start = None;
                }
            }
        }
        if let Some(start) = run_start {
            consider_run(
                start,
                last_good_r,
                run_aligned,
                run_closely_aligned,
                run_informative,
                run_gain,
            );
        }
    }

    best_anchor.map(|(_, anchor)| anchor)
}

fn collect_interior_anchors(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    ref_sigs: &[RowSignature],
    test_sigs: &[RowSignature],
    mut r_range: std::ops::Range<usize>,
    mut t_range: std::ops::Range<usize>,
    max_delta: f64,
    depth: usize,
    out: &mut Vec<AlignedAnchor>,
) {
    let max_aligned_diff = reference.width / 100;
    let prefix_r_start = r_range.start;
    let prefix_t_start = t_range.start;
    while r_range.start < r_range.end
        && t_range.start < t_range.end
        && row_difference(reference, test, r_range.start, t_range.start, max_delta)
            <= max_aligned_diff
    {
        r_range.start += 1;
        t_range.start += 1;
    }
    let prefix_len = r_range.start - prefix_r_start;
    if prefix_len >= MIN_TAIL_ALIGNED_ROWS {
        out.push(AlignedAnchor {
            ref_start: prefix_r_start,
            test_start: prefix_t_start,
            len: prefix_len,
        });
    } else {
        r_range.start = prefix_r_start;
        t_range.start = prefix_t_start;
    }

    let suffix_r_end = r_range.end;
    let suffix_t_end = t_range.end;
    while r_range.start < r_range.end
        && t_range.start < t_range.end
        && row_difference(reference, test, r_range.end - 1, t_range.end - 1, max_delta)
            <= max_aligned_diff
    {
        r_range.end -= 1;
        t_range.end -= 1;
    }
    let suffix_len = suffix_r_end - r_range.end;
    let suffix_anchor = if suffix_len >= MIN_TAIL_ALIGNED_ROWS {
        Some(AlignedAnchor {
            ref_start: r_range.end,
            test_start: t_range.end,
            len: suffix_len,
        })
    } else {
        r_range.end = suffix_r_end;
        t_range.end = suffix_t_end;
        None
    };

    if depth < MAX_ALIGNMENT_DEPTH
        && let Some(anchor) = find_best_shifted_anchor(
            reference,
            test,
            ref_sigs,
            test_sigs,
            r_range.clone(),
            t_range.clone(),
            max_delta,
        )
    {
        if anchor.ref_start > r_range.start && anchor.test_start > t_range.start {
            collect_interior_anchors(
                reference,
                test,
                ref_sigs,
                test_sigs,
                r_range.start..anchor.ref_start,
                t_range.start..anchor.test_start,
                max_delta,
                depth + 1,
                out,
            );
        }
        out.push(anchor);
        let next_r = anchor.ref_start + anchor.len;
        let next_t = anchor.test_start + anchor.len;
        if next_r < r_range.end && next_t < t_range.end {
            collect_interior_anchors(
                reference,
                test,
                ref_sigs,
                test_sigs,
                next_r..r_range.end,
                next_t..t_range.end,
                max_delta,
                depth + 1,
                out,
            );
        }
    }

    if let Some(suffix) = suffix_anchor {
        out.push(suffix);
    }
}

fn find_transition_gap_offset(
    shorter: ImageView<'_>,
    taller: ImageView<'_>,
    max_delta: f64,
) -> usize {
    if shorter.height == 0 || shorter.height >= taller.height {
        return 0;
    }
    let height = taller.height - shorter.height;
    let top_equal = (0..shorter.height)
        .take_while(|&row| shorter.row_slice(row) == taller.row_slice(row))
        .count();
    let mut gain = 0_i64;
    let mut best_gain = 0_i64;
    let mut best_start = shorter.height;
    for row in (top_equal..shorter.height).rev() {
        let unshifted = row_difference(shorter, taller, row, row, max_delta);
        let shifted = row_difference(shorter, taller, row, row + height, max_delta);
        let (delta, _) = row_shift_gain(shorter.width, unshifted, shifted);
        gain += delta;
        if gain > best_gain {
            best_gain = gain;
            best_start = row;
        }
    }
    best_start
}

fn push_merged_gap(gaps: &mut Vec<VerticalGap>, gap: VerticalGap) {
    if gap.height == 0 {
        return;
    }
    if let Some(last) = gaps.last_mut()
        && last.start == gap.start
    {
        last.height += gap.height;
    } else {
        gaps.push(gap);
    }
}

fn alignment_mismatch_score(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    alignment: &DiffAlignment,
    max_delta: f64,
) -> u64 {
    let ref_view = ImageView {
        gaps: alignment.reference_gaps(),
        ..reference
    };
    let test_view = ImageView {
        gaps: alignment.test_gaps(),
        ..test
    };
    let width = reference.width.max(test.width);
    let max_aligned_diff = width / 100;
    let height = ref_view
        .aligned_height()
        .unwrap_or(reference.height)
        .max(test_view.aligned_height().unwrap_or(test.height));
    let mut total = 0_u64;
    for y in 0..height {
        match (ref_view.source_row(y), test_view.source_row(y)) {
            (Some(r_row), Some(t_row)) => {
                let shifted = row_difference(reference, test, r_row, t_row, max_delta);
                total += shifted as u64;
                if r_row != t_row && r_row < test.height {
                    let unshifted = row_difference(reference, test, r_row, r_row, max_delta);
                    if unshifted <= max_aligned_diff && shifted > unshifted {
                        total += width as u64;
                    }
                }
            }
            _ => {
                total += width as u64;
            }
        }
    }
    total
}

fn find_multi_band_alignment(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    max_delta: f64,
    top_equal: usize,
    bottom_equal: usize,
) -> Option<DiffAlignment> {
    let ref_sigs = compute_row_signatures(reference);
    let test_sigs = compute_row_signatures(test);

    let mut anchors = Vec::new();
    if top_equal > 0 {
        anchors.push(AlignedAnchor {
            ref_start: 0,
            test_start: 0,
            len: top_equal,
        });
    }
    let r_win_end = reference.height - bottom_equal;
    let t_win_end = test.height - bottom_equal;
    if top_equal < r_win_end && top_equal < t_win_end {
        collect_interior_anchors(
            reference,
            test,
            &ref_sigs,
            &test_sigs,
            top_equal..r_win_end,
            top_equal..t_win_end,
            max_delta,
            0,
            &mut anchors,
        );
    }
    if bottom_equal > 0 {
        anchors.push(AlignedAnchor {
            ref_start: r_win_end,
            test_start: t_win_end,
            len: bottom_equal,
        });
    }

    let mut merged: Vec<AlignedAnchor> = Vec::with_capacity(anchors.len());
    for anchor in anchors {
        if anchor.len == 0 {
            continue;
        }
        if let Some(prev) = merged.last_mut()
            && prev.shift() == anchor.shift()
            && prev.ref_start + prev.len <= anchor.ref_start
            && prev.test_start + prev.len <= anchor.test_start
        {
            prev.len = (anchor.ref_start + anchor.len) - prev.ref_start;
        } else if merged.last().is_none_or(|prev| {
            prev.ref_start + prev.len <= anchor.ref_start
                && prev.test_start + prev.len <= anchor.test_start
        }) {
            merged.push(anchor);
        }
    }

    let shifted_rows: usize = merged
        .iter()
        .filter(|anchor| anchor.shift() != 0)
        .map(|anchor| anchor.len)
        .sum();
    if shifted_rows < MIN_TAIL_ALIGNED_ROWS {
        return None;
    }

    let mut ref_gaps = Vec::new();
    let mut test_gaps = Vec::new();
    let mut cur_r = 0_usize;
    let mut cur_t = 0_usize;

    let endpoints = merged
        .iter()
        .map(|anchor| (anchor.ref_start, anchor.test_start, anchor.len))
        .chain(std::iter::once((reference.height, test.height, 0)));

    for (next_r, next_t, len) in endpoints {
        let delta_r = next_r.saturating_sub(cur_r);
        let delta_t = next_t.saturating_sub(cur_t);
        if delta_t > delta_r {
            let offset = if delta_r == 0 {
                0
            } else {
                find_transition_gap_offset(
                    reference.sub_rows(cur_r, next_r),
                    test.sub_rows(cur_t, next_t),
                    max_delta,
                )
            };
            push_merged_gap(
                &mut ref_gaps,
                VerticalGap {
                    start: cur_r + offset,
                    height: delta_t - delta_r,
                },
            );
        } else if delta_r > delta_t {
            let offset = if delta_t == 0 {
                0
            } else {
                find_transition_gap_offset(
                    test.sub_rows(cur_t, next_t),
                    reference.sub_rows(cur_r, next_r),
                    max_delta,
                )
            };
            push_merged_gap(
                &mut test_gaps,
                VerticalGap {
                    start: cur_t + offset,
                    height: delta_r - delta_t,
                },
            );
        }
        cur_r = next_r + len;
        cur_t = next_t + len;
    }

    let alignment = DiffAlignment::from_gaps(ref_gaps, test_gaps);
    (alignment != DiffAlignment::default() && alignment.is_valid()).then_some(alignment)
}

fn find_alignment_gaps(
    reference: ImageView<'_>,
    test: ImageView<'_>,
    max_delta: f64,
) -> DiffAlignment {
    if reference.width == 0 || reference.width != test.width {
        return DiffAlignment::default();
    }
    let min_h = reference.height.min(test.height);
    let top_equal = (0..min_h)
        .take_while(|&row| reference.row_slice(row) == test.row_slice(row))
        .count();
    if top_equal == min_h {
        return DiffAlignment::default();
    }
    let max_bottom = min_h - top_equal;
    let bottom_equal = (0..max_bottom)
        .take_while(|&offset| {
            reference.row_slice(reference.height - 1 - offset)
                == test.row_slice(test.height - 1 - offset)
        })
        .count();

    if top_equal + bottom_equal == min_h && reference.height != test.height {
        if bottom_equal >= MIN_ALIGNED_ROWS
            || (bottom_equal >= MIN_TAIL_ALIGNED_ROWS && reference.width >= 8)
        {
            return if reference.height < test.height {
                DiffAlignment::from_gaps(
                    vec![VerticalGap {
                        start: top_equal,
                        height: test.height - reference.height,
                    }],
                    Vec::new(),
                )
            } else {
                DiffAlignment::from_gaps(
                    Vec::new(),
                    vec![VerticalGap {
                        start: top_equal,
                        height: reference.height - test.height,
                    }],
                )
            };
        }
        return DiffAlignment::default();
    }

    let single = if reference.height < test.height {
        find_vertical_gap(reference, test, max_delta)
            .map(|gap| DiffAlignment::from_gaps(vec![gap], Vec::new()))
    } else if test.height < reference.height {
        find_vertical_gap(test, reference, max_delta)
            .map(|gap| DiffAlignment::from_gaps(Vec::new(), vec![gap]))
    } else {
        None
    };

    let ref_interior = reference.height - top_equal - bottom_equal;
    let test_interior = test.height - top_equal - bottom_equal;
    let can_try_multi = if reference.height == test.height {
        ref_interior >= MIN_ALIGNED_ROWS
    } else {
        ref_interior >= MIN_TAIL_ALIGNED_ROWS && test_interior >= MIN_TAIL_ALIGNED_ROWS
    };

    if can_try_multi
        && let Some(multi) =
            find_multi_band_alignment(reference, test, max_delta, top_equal, bottom_equal)
        && Some(&multi) != single.as_ref()
    {
        let unaligned_score =
            alignment_mismatch_score(reference, test, &DiffAlignment::default(), max_delta);
        let baseline_score = single
            .as_ref()
            .map(|s| alignment_mismatch_score(reference, test, s, max_delta))
            .unwrap_or(unaligned_score)
            .min(unaligned_score);
        let multi_score = alignment_mismatch_score(reference, test, &multi, max_delta);
        let min_improvement = (MIN_TAIL_ALIGNED_ROWS * (reference.width / 2).max(1)) as u64;
        if multi_score.saturating_add(min_improvement) <= baseline_score {
            return multi;
        }
    }

    single.unwrap_or_default()
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
    let height = reference
        .aligned_height()
        .unwrap_or(reference.height)
        .max(test.aligned_height().unwrap_or(test.height));
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
/// image sized to the union of both images is still produced. Vertical
/// insertions/deletions (single or multi-band) are aligned when matching rows
/// support them; missing bands and all remaining pixel differences are flagged.
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
        gaps: &[],
    };
    let mut test = ImageView {
        pixels: &test_pixels,
        width: test_w,
        height: test_h,
        gaps: &[],
    };

    let out_w = ref_w.max(test_w);
    let base_out_h = ref_h.max(test_h);
    let base_total_pixels = out_w
        .checked_mul(base_out_h)
        .ok_or_else(|| anyhow!("Image dimensions are too large to compare safely"))?;
    if base_total_pixels > MAX_VISUAL_IMAGE_PIXELS as usize {
        anyhow::bail!(
            "Comparison canvas {}x{} exceeds the {} pixel limit",
            out_w,
            base_out_h,
            MAX_VISUAL_IMAGE_PIXELS
        );
    }

    let mut alignment = find_alignment_gaps(reference, test, max_delta);
    let mut out_h = reference
        .height
        .checked_add(alignment.reference_gaps().iter().map(|g| g.height).sum())
        .and_then(|r_h| {
            test.height
                .checked_add(alignment.test_gaps().iter().map(|g| g.height).sum())
                .map(|t_h| r_h.max(t_h))
        })
        .unwrap_or(base_out_h);
    if out_h > MAX_VISUAL_IMAGE_DIMENSION as usize
        || out_w
            .checked_mul(out_h)
            .is_none_or(|pixels| pixels > MAX_VISUAL_IMAGE_PIXELS as usize)
    {
        alignment = DiffAlignment::default();
        out_h = base_out_h;
    }
    reference.gaps = alignment.reference_gaps();
    test.gaps = alignment.test_gaps();

    let total_pixels = out_w * out_h;
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
            alignment,
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
        // Top hero banner (rows 0..40): 24 columns dark blue + 40 columns white in reference,
        // so shifting into the white middle section matches 38 columns (`shifted = 26`).
        for row in 0..40 {
            for col in 0..24 {
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

        for inserted_color in [RGBA8::new(0, 0, 0, 255), white] {
            let mut test = reference[..gap_start * width].to_vec();
            // Top hero banner changed in place in test to solid orange (`unshifted = 64`).
            for row in 0..40 {
                for col in 0..width {
                    test[row * width + col] = RGBA8::new(240, 180, 120, 255);
                }
            }
            // Insert new block at `gap_start = 100`.
            test.extend(vec![inserted_color; gap_height * width]);
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
                    ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
            }
        );
    }

    #[test]
    fn multiple_inserted_bands_align_intermediate_and_trailing_sections() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 160;
        let gap1 = VerticalGap {
            start: 30,
            height: 8,
        };
        let gap2 = VerticalGap {
            start: 100,
            height: 12,
        };
        let reference = patterned_image(width, height);
        let mut test = Vec::with_capacity((height + gap1.height + gap2.height) * width);
        test.extend_from_slice(&reference[..gap1.start * width]);
        test.extend(vec![RGBA8::new(0, 0, 0, 255); gap1.height * width]);
        test.extend_from_slice(&reference[gap1.start * width..gap2.start * width]);
        test.extend(vec![RGBA8::new(255, 255, 255, 255); gap2.height * width]);
        test.extend_from_slice(&reference[gap2.start * width..]);

        fs::write(
            &reference_path,
            lodepng::encode32(&reference, width, height).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&test, width, height + gap1.height + gap2.height).unwrap(),
        )
        .unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert_eq!(
            result.alignment,
            DiffAlignment::from_gaps(vec![gap1, gap2], Vec::new())
        );
        let expected = ((gap1.height + gap2.height) * width) as f64 / test.len() as f64 * 100.0;
        assert!((result.raw_mis_match_percentage - expected).abs() < 1e-10);
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        let pink = RGBA8::new(255, 0, 255, 200);
        // Band 1: aligned rows 30..38
        assert!(
            diff.buffer[30 * width..38 * width]
                .iter()
                .all(|pixel| *pixel == pink)
        );
        // Intermediate section: aligned rows 38..108 (`ref[30..100]`) must have no diff pixels!
        assert!(
            diff.buffer[38 * width..108 * width]
                .iter()
                .all(|pixel| *pixel != pink)
        );
        // Band 2: aligned rows 108..120
        assert!(
            diff.buffer[108 * width..120 * width]
                .iter()
                .all(|pixel| *pixel == pink)
        );
        // Trailing section: aligned rows 120..180 (`ref[100..160]`) must have no diff pixels!
        assert!(
            diff.buffer[120 * width..]
                .iter()
                .all(|pixel| *pixel != pink)
        );
    }

    #[test]
    fn same_height_viewport_insertion_aligns_shifted_content_and_cropped_tail() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 160;
        let insert_row = 36;
        let shift_height = 14;
        let reference = patterned_image(width, height);
        let mut test = Vec::with_capacity(height * width);
        test.extend_from_slice(&reference[..insert_row * width]);
        test.extend(vec![RGBA8::new(0, 0, 0, 255); shift_height * width]);
        test.extend_from_slice(&reference[insert_row * width..(height - shift_height) * width]);

        fs::write(
            &reference_path,
            lodepng::encode32(&reference, width, height).unwrap(),
        )
        .unwrap();
        fs::write(&test_path, lodepng::encode32(&test, width, height).unwrap()).unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();

        assert!(result.is_same_dimensions);
        assert_eq!(
            result.alignment,
            DiffAlignment::from_gaps(
                vec![VerticalGap {
                    start: insert_row,
                    height: shift_height,
                }],
                vec![VerticalGap {
                    start: height,
                    height: shift_height,
                }],
            )
        );
        let diff = lodepng::decode32(diff.unwrap()).unwrap();
        assert_eq!((diff.width, diff.height), (width, height + shift_height));
        let pink = RGBA8::new(255, 0, 255, 200);
        // Shifted middle section (aligned rows 50..160) must be completely aligned and free of diffs.
        assert!(
            diff.buffer[(insert_row + shift_height) * width..height * width]
                .iter()
                .all(|pixel| *pixel != pink)
        );
    }

    #[test]
    fn insertion_with_partially_changed_content_keeps_gap_at_insertion_start() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 100;
        let height = 160;
        let gap_start = 40;
        let gap_height = 20;
        let changed_rows = 40;
        let changed_cols = 20;
        let reference = patterned_image(width, height);
        let mut test = reference[..gap_start * width].to_vec();
        test.extend(vec![RGBA8::new(0, 0, 0, 255); gap_height * width]);
        test.extend_from_slice(&reference[gap_start * width..]);
        for row in (gap_start + gap_height)..(gap_start + gap_height + changed_rows) {
            for col in 0..changed_cols {
                let pixel = &mut test[row * width + col];
                *pixel = RGBA8::new(pixel.r ^ 0xff, pixel.g ^ 0xff, pixel.b ^ 0xff, 255);
            }
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

        let (result, _) = compare_images_with_threshold(&reference_path, &test_path, 0.0).unwrap();

        assert_eq!(
            result.alignment,
            DiffAlignment {
                reference: Some(VerticalGap {
                    start: gap_start,
                    height: gap_height,
                }),
                test: None,
                ..Default::default()
            }
        );
        let expected =
            (gap_height * width + changed_rows * changed_cols) as f64 / test.len() as f64 * 100.0;
        assert!((result.raw_mis_match_percentage - expected).abs() < 1e-10);
        assert_eq!(result.mis_match_percentage, "15.56");
    }

    #[test]
    fn deletion_with_partially_changed_content_keeps_gap_at_deletion_start() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 100;
        let height = 180;
        let gap_start = 40;
        let gap_height = 20;
        let changed_rows = 40;
        let changed_cols = 20;
        let reference = patterned_image(width, height);
        let mut test = reference[..gap_start * width].to_vec();
        test.extend_from_slice(&reference[(gap_start + gap_height) * width..]);
        for row in gap_start..(gap_start + changed_rows) {
            for col in 0..changed_cols {
                let pixel = &mut test[row * width + col];
                *pixel = RGBA8::new(pixel.r ^ 0xff, pixel.g ^ 0xff, pixel.b ^ 0xff, 255);
            }
        }

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

        let (result, _) = compare_images_with_threshold(&reference_path, &test_path, 0.0).unwrap();

        assert_eq!(
            result.alignment,
            DiffAlignment {
                reference: None,
                test: Some(VerticalGap {
                    start: gap_start,
                    height: gap_height,
                }),
                ..Default::default()
            }
        );
        let expected = (gap_height * width + changed_rows * changed_cols) as f64
            / reference.len() as f64
            * 100.0;
        assert!((result.raw_mis_match_percentage - expected).abs() < 1e-10);
        assert_eq!(result.mis_match_percentage, "15.56");
    }

    #[test]
    fn symmetric_repeating_shifts_produce_deterministic_alignment_and_diff_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 96;
        let tile = patterned_image(width, 16);
        let reference: Vec<RGBA8> = (0..height)
            .flat_map(|row| {
                tile[(row % 16) * width..(row % 16 + 1) * width]
                    .iter()
                    .copied()
            })
            .collect();
        let test: Vec<RGBA8> = (0..height)
            .flat_map(|row| {
                let shifted_row = (row + 8) % 16;
                tile[shifted_row * width..(shifted_row + 1) * width]
                    .iter()
                    .copied()
            })
            .collect();
        fs::write(
            &reference_path,
            lodepng::encode32(&reference, width, height).unwrap(),
        )
        .unwrap();
        fs::write(&test_path, lodepng::encode32(&test, width, height).unwrap()).unwrap();

        let (first_result, first_diff) = compare_images(&reference_path, &test_path).unwrap();
        let first_diff = first_diff.unwrap();
        for _ in 0..64 {
            let (result, diff) = compare_images(&reference_path, &test_path).unwrap();
            assert_eq!(result.alignment, first_result.alignment);
            assert_eq!(diff.as_deref(), Some(first_diff.as_slice()));
        }
    }

    #[test]
    fn multiple_inserted_bands_align_repeating_four_row_middle_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let width = 32;
        let height = 160;
        let gap1 = VerticalGap {
            start: 30,
            height: 7,
        };
        let gap2 = VerticalGap {
            start: 100,
            height: 13,
        };
        let mut reference = patterned_image(width, height);
        let pattern = patterned_image(width, 4);
        for row in gap1.start..gap2.start {
            let phase = (row - gap1.start) % 4;
            reference[row * width..(row + 1) * width]
                .copy_from_slice(&pattern[phase * width..(phase + 1) * width]);
        }

        let mut test = Vec::with_capacity((height + gap1.height + gap2.height) * width);
        test.extend_from_slice(&reference[..gap1.start * width]);
        test.extend(vec![RGBA8::new(0, 0, 0, 255); gap1.height * width]);
        test.extend_from_slice(&reference[gap1.start * width..gap2.start * width]);
        test.extend(vec![RGBA8::new(255, 255, 255, 255); gap2.height * width]);
        test.extend_from_slice(&reference[gap2.start * width..]);

        fs::write(
            &reference_path,
            lodepng::encode32(&reference, width, height).unwrap(),
        )
        .unwrap();
        fs::write(
            &test_path,
            lodepng::encode32(&test, width, height + gap1.height + gap2.height).unwrap(),
        )
        .unwrap();

        let (result, _) = compare_images(&reference_path, &test_path).unwrap();

        assert_eq!(
            result.alignment,
            DiffAlignment::from_gaps(vec![gap1, gap2], Vec::new())
        );
        assert_eq!(result.mis_match_percentage, "11.11");
    }

    #[test]
    fn expanded_canvas_exceeding_max_dimension_falls_back_to_bounded_height() {
        let dir = tempfile::tempdir().unwrap();
        let reference_path = dir.path().join("reference.png");
        let test_path = dir.path().join("test.png");
        let diff_path = dir.path().join("diff.png");
        let width = 8;
        let height = MAX_VISUAL_IMAGE_DIMENSION as usize;
        let shift = 16;
        let reference = patterned_image(width, height);
        let mut test = Vec::with_capacity(width * height);
        test.extend(vec![RGBA8::new(0, 0, 0, 255); shift * width]);
        test.extend_from_slice(&reference[..(height - shift) * width]);

        fs::write(
            &reference_path,
            lodepng::encode32(&reference, width, height).unwrap(),
        )
        .unwrap();
        fs::write(&test_path, lodepng::encode32(&test, width, height).unwrap()).unwrap();

        let (result, diff) = compare_images(&reference_path, &test_path).unwrap();
        assert_eq!(result.alignment, DiffAlignment::default());
        let diff_bytes = diff.unwrap();
        fs::write(&diff_path, &diff_bytes).unwrap();
        assert_eq!(
            crate::helpers::png::read_bounded_png_dimensions(&diff_path).unwrap(),
            (width, height)
        );
    }
}
