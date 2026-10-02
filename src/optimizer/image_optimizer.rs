use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    cli::args::OptimizeArgs,
    common,
    diagnostics::{record::*, runtime},
    helpers::{
        OnceLockExtensions,
        png::{MAX_VISUAL_IMAGE_DECOMPRESSED_BYTES, read_bounded_png_dimensions},
        write_file_atomically,
    },
};

enum OptimizeResult {
    Pair(u64, u64),        // (original_size, optimized_size)
    TooSmall(u64),         // original_size
    AlreadyOptimized(u64), // original_size
}

pub fn optimize_images(args: OptimizeArgs) -> Result<()> {
    let output_dir = common::REGRESSIFY_DIR.get_required()?;
    let png_files = if output_dir.exists() {
        collect_png_files(output_dir)?
    } else {
        Vec::new()
    };
    runtime::event(
        Stage::PackagingOptimization,
        DiagnosticEvent::Started,
        Severity::Info,
        [(Field::FileCount, FieldValue::Number(png_files.len() as u64))],
    );

    let total_files = png_files.len();
    let total_original_size = AtomicU64::new(0);
    let total_optimized_size = AtomicU64::new(0);
    let total_skipped_files = AtomicUsize::new(0);
    let context = runtime::current();
    png_files.par_iter().try_for_each(|path| -> Result<()> {
        runtime::in_scope(context.clone(), || {
            let started = Instant::now();
            let result = optimize_png(path, &args);
            let mut fields = vec![(
                Field::Artifact,
                FieldValue::Identity(SafeIdentity::fingerprint(
                    path.as_os_str().as_encoded_bytes(),
                )),
            )];
            if let Ok(optimized) = &result {
                let (before, after, reason) = match optimized {
                    OptimizeResult::Pair(before, after) => (*before, *after, SafeValue::Complete),
                    OptimizeResult::AlreadyOptimized(before) => {
                        total_skipped_files.fetch_add(1, Ordering::Relaxed);
                        (*before, *before, SafeValue::Unchanged)
                    }
                    OptimizeResult::TooSmall(before) => {
                        total_skipped_files.fetch_add(1, Ordering::Relaxed);
                        (*before, *before, SafeValue::BelowMinimum)
                    }
                };
                total_original_size.fetch_add(before, Ordering::Relaxed);
                total_optimized_size.fetch_add(after, Ordering::Relaxed);
                fields.extend([
                    (Field::BytesBefore, FieldValue::Number(before)),
                    (Field::BytesAfter, FieldValue::Number(after)),
                    (Field::Reason, FieldValue::Kind(reason)),
                ]);
            }
            if let Some(context) = runtime::current() {
                context.record(
                    Stage::PackagingOptimization,
                    DiagnosticEvent::StageFinished,
                    if result.is_ok() {
                        Severity::Debug
                    } else {
                        Severity::Error
                    },
                    fields,
                    runtime::Completion {
                        outcome: Some(if result.is_ok() {
                            Outcome::Success
                        } else {
                            Outcome::Failure
                        }),
                        error: result.as_ref().err().map(SanitizedError::capture),
                        duration_ms: Some(started.elapsed().as_millis() as u64),
                    },
                );
            }
            result
                .map(|_| ())
                .with_context(|| format!("Failed to optimize '{}'", path.display()))
        })
    })?;

    runtime::event(
        Stage::PackagingOptimization,
        DiagnosticEvent::StageFinished,
        Severity::Info,
        [
            (Field::FileCount, FieldValue::Number(total_files as u64)),
            (
                Field::SkippedFiles,
                FieldValue::Number(total_skipped_files.load(Ordering::Relaxed) as u64),
            ),
            (
                Field::BytesBefore,
                FieldValue::Number(total_original_size.load(Ordering::Relaxed)),
            ),
            (
                Field::BytesAfter,
                FieldValue::Number(total_optimized_size.load(Ordering::Relaxed)),
            ),
            (
                Field::State,
                FieldValue::Kind(if total_files == 0 {
                    SafeValue::Empty
                } else {
                    SafeValue::Complete
                }),
            ),
        ],
    );
    Ok(())
}

fn collect_png_files(output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut png_files = Vec::new();
    // Published image bytes must continue matching their immutable URL hashes.
    for entry in WalkDir::new(output_dir)
        .into_iter()
        .filter_entry(|entry| entry.depth() != 1 || entry.file_name() != "_snapshot")
    {
        let entry = entry.with_context(|| {
            format!(
                "Failed to scan Regressify output directory '{}'",
                output_dir.display()
            )
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let is_png = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
        if is_png {
            png_files.push(path.to_owned());
        }
    }
    png_files.sort();
    Ok(png_files)
}

/// Configure OxiPNG for bounded, lossless recompression without altering PNG
/// color representation, bit depth, palette, or interlacing.
fn compression_only_options(speed: u8) -> oxipng::Options {
    let oxipng_level = (10 - speed).clamp(0, 6);
    let mut options = oxipng::Options::from_preset(oxipng_level);
    options.interlace = None;
    options.optimize_alpha = false;
    options.bit_depth_reduction = false;
    options.color_type_reduction = false;
    options.palette_reduction = false;
    options.grayscale_reduction = false;
    options.scale_16 = false;
    options.strip = oxipng::StripChunks::None;
    options.max_decompressed_size = Some(MAX_VISUAL_IMAGE_DECOMPRESSED_BYTES);
    options
}

fn optimize_png(path: &Path, args: &OptimizeArgs) -> Result<OptimizeResult> {
    let metadata = fs::metadata(path)?;
    let original_size = metadata.len();
    if original_size < args.min_size_kb.saturating_mul(1024) {
        return Ok(OptimizeResult::TooSmall(original_size));
    }

    read_bounded_png_dimensions(path)?;
    let source =
        fs::read(path).with_context(|| format!("Failed to read PNG '{}'", path.display()))?;

    let options = compression_only_options(args.speed);
    let optimized_buffer = oxipng::optimize_from_memory(&source, &options)
        .with_context(|| format!("Failed to losslessly optimize PNG '{}'", path.display()))?;

    let optimized_size = optimized_buffer.len() as u64;
    if optimized_size >= original_size {
        return Ok(OptimizeResult::AlreadyOptimized(original_size));
    }

    write_file_atomically(path, &optimized_buffer)?;

    Ok(OptimizeResult::Pair(original_size, optimized_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::helpers::png::MAX_VISUAL_IMAGE_DIMENSION;

    fn optimize_args(min_size_kb: u64) -> OptimizeArgs {
        OptimizeArgs {
            speed: 8,
            min_size_kb,
        }
    }

    fn decode_rgba(path: &Path) -> Vec<rgb::RGBA8> {
        let mut decoder = lodepng::Decoder::new();
        decoder.color_convert(true);
        decoder.info_raw_mut().colortype = lodepng::ColorType::RGBA;
        decoder.info_raw_mut().set_bitdepth(8);
        match decoder.decode_file(path).unwrap() {
            lodepng::Image::RGBA(image) => image.buffer.to_vec(),
            image => panic!("expected RGBA image, got {image:?}"),
        }
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut header = Vec::from([137, 80, 78, 71, 13, 10, 26, 10]);
        header.extend_from_slice(&13_u32.to_be_bytes());
        header.extend_from_slice(b"IHDR");
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header
    }

    #[test]
    fn compression_only_options_preserve_png_representation() {
        let options = compression_only_options(8);

        assert_eq!(options.interlace, None);
        assert!(!options.optimize_alpha);
        assert!(!options.bit_depth_reduction);
        assert!(!options.color_type_reduction);
        assert!(!options.palette_reduction);
        assert!(!options.grayscale_reduction);
        assert!(!options.scale_16);
        assert_eq!(options.strip, oxipng::StripChunks::None);
        assert_eq!(
            options.max_decompressed_size,
            Some(MAX_VISUAL_IMAGE_DECOMPRESSED_BYTES)
        );
    }

    #[test]
    fn lossless_optimization_preserves_decoded_pixels() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("capture.png");
        let mut pixels = Vec::with_capacity(64 * 64 * 4);
        for pixel_index in 0..(64 * 64) {
            pixels.extend_from_slice(&[
                (pixel_index % 251) as u8,
                (pixel_index % 127) as u8,
                (pixel_index % 61) as u8,
                255,
            ]);
        }
        fs::write(&path, lodepng::encode32(&pixels, 64, 64).unwrap()).unwrap();

        let before = decode_rgba(&path);
        optimize_png(&path, &optimize_args(0)).unwrap();
        let after = decode_rgba(&path);

        assert_eq!(after, before);
    }

    #[test]
    fn collect_png_files_is_sorted_and_ignores_other_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("nested")).unwrap();
        fs::write(temp.path().join("b.png"), []).unwrap();
        fs::write(temp.path().join("nested/a.PNG"), []).unwrap();
        fs::write(temp.path().join("not-a-capture.jpg"), []).unwrap();
        fs::create_dir_all(temp.path().join("_snapshot/images")).unwrap();
        fs::write(
            temp.path().join("_snapshot/images/immutable.png"),
            b"published",
        )
        .unwrap();

        let files = collect_png_files(temp.path()).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|path| path.strip_prefix(temp.path()).unwrap().to_path_buf())
            .collect();

        assert_eq!(
            names,
            vec![PathBuf::from("b.png"), PathBuf::from("nested/a.PNG")]
        );
    }

    #[test]
    fn files_below_minimum_size_are_not_rewritten() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("small.png");
        fs::write(&path, [1_u8, 2, 3]).unwrap();

        let result = optimize_png(&path, &optimize_args(1)).unwrap();

        assert!(matches!(result, OptimizeResult::TooSmall(3)));
        assert_eq!(fs::read(path).unwrap(), [1_u8, 2, 3]);
    }

    #[test]
    fn oversized_pngs_are_rejected_before_optimization() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("oversized.png");
        fs::write(
            &path,
            png_header(MAX_VISUAL_IMAGE_DIMENSION, MAX_VISUAL_IMAGE_DIMENSION),
        )
        .unwrap();

        let error = match optimize_png(&path, &optimize_args(0)) {
            Ok(_) => panic!("optimizer unexpectedly accepted an oversized image"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("pixel limit"));
    }
}
