use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    cli::args::OptimizeArgs,
    common,
    helpers::{OnceLockExtensions, format_bytes, write_file_atomically},
};

enum OptimizeResult {
    Pair(u64, u64),        // (original_size, optimized_size)
    TooSmall(u64),         // original_size
    AlreadyOptimized(u64), // original_size
}

pub fn optimize_images(args: OptimizeArgs) -> Result<()> {
    eprintln!("Starting image optimization...");
    let output_dir = common::REGRESSIFY_DIR.get_required()?;

    if !output_dir.exists() {
        eprintln!(
            "No Regressify output directory found: {}",
            output_dir.display()
        );
        return Ok(());
    }

    eprintln!("Searching for PNGs in: {}", output_dir.display());
    let png_files = collect_png_files(output_dir)?;

    if png_files.is_empty() {
        eprintln!("No PNG files found.");
        return Ok(());
    }

    eprintln!(
        "Found {} PNG files. Starting optimization...",
        png_files.len()
    );

    let total_files = png_files.len();
    let counter = AtomicUsize::new(0);
    let total_original_size = AtomicU64::new(0);
    let total_optimized_size = AtomicU64::new(0);
    let total_skipped_files = AtomicUsize::new(0);

    let to_be_print_nth = match png_files.len() {
        n if n < 50 => 1,
        n if n < 200 => 2,
        n if n < 500 => 5,
        n if n < 2000 => 10,
        _ => 20,
    };
    let nth_width = png_files.len().to_string().len();
    png_files.par_iter().try_for_each(|path| -> Result<()> {
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.png");

        let result = optimize_png(path, &args);

        let idx = counter.fetch_add(1, Ordering::Relaxed) + 1;
        let should_print = idx.is_multiple_of(to_be_print_nth) || idx == total_files;

        match result {
            Ok(OptimizeResult::Pair(original_size, optimized_size)) => {
                total_original_size.fetch_add(original_size, Ordering::Relaxed);
                total_optimized_size.fetch_add(optimized_size, Ordering::Relaxed);

                if should_print {
                    eprintln!(
                        "{:>width$} of {}: {}: {} --> {}",
                        idx,
                        total_files,
                        file_name,
                        format_bytes(original_size).yellow(),
                        format_bytes(optimized_size).green(),
                        width = nth_width
                    );
                }
            }
            Ok(OptimizeResult::AlreadyOptimized(original_size)) => {
                total_original_size.fetch_add(original_size, Ordering::Relaxed);
                total_optimized_size.fetch_add(original_size, Ordering::Relaxed);
                total_skipped_files.fetch_add(1, Ordering::Relaxed);

                if should_print {
                    eprintln!(
                        "{:>width$} of {}: {}: Already optimized: {}.",
                        idx,
                        total_files,
                        file_name,
                        format_bytes(original_size).yellow(),
                        width = nth_width
                    );
                }
            }
            Ok(OptimizeResult::TooSmall(original_size)) => {
                total_original_size.fetch_add(original_size, Ordering::Relaxed);
                total_optimized_size.fetch_add(original_size, Ordering::Relaxed);
                total_skipped_files.fetch_add(1, Ordering::Relaxed);

                if should_print {
                    eprintln!(
                        "{:>width$} of {}: {}: {} ({}).",
                        idx,
                        total_files,
                        file_name,
                        "Too Small".cyan(),
                        format_bytes(original_size),
                        width = nth_width
                    );
                }
            }
            Err(e) => {
                let original_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

                total_original_size.fetch_add(original_size, Ordering::Relaxed);
                total_optimized_size.fetch_add(original_size, Ordering::Relaxed);

                if should_print {
                    eprintln!(
                        "{:>width$} of {}: {}: {}: {}",
                        idx,
                        total_files,
                        file_name,
                        "Error".red(),
                        e,
                        width = nth_width
                    );
                }

                return Err(anyhow!("Failed to optimize '{}': {}", path.display(), e));
            }
        };

        Ok(())
    })?;

    // Report summary
    let final_original_size = total_original_size.load(Ordering::Relaxed);
    let final_optimized_size = total_optimized_size.load(Ordering::Relaxed);
    let final_skipped_files = total_skipped_files.load(Ordering::Relaxed);

    eprintln!("\nOptimization Summary:");
    eprintln!("Total files processed: {}", total_files);
    eprintln!(
        "Total files optimized: {}",
        total_files - final_skipped_files
    );
    eprintln!("Total files skipped: {}", final_skipped_files);
    eprintln!(
        "Total original size: {}",
        format_bytes(final_original_size).yellow()
    );
    eprintln!(
        "Total optimized size: {}",
        format_bytes(final_optimized_size).green()
    );
    let savings = final_original_size.saturating_sub(final_optimized_size);
    eprintln!("Total savings: {}", format_bytes(savings).green());

    Ok(())
}

fn collect_png_files(output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut png_files = Vec::new();
    for entry in WalkDir::new(output_dir).into_iter() {
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

fn optimize_png(path: &Path, args: &OptimizeArgs) -> Result<OptimizeResult> {
    let metadata = fs::metadata(path)?;
    let original_size = metadata.len();
    if original_size < args.min_size_kb.saturating_mul(1024) {
        return Ok(OptimizeResult::TooSmall(original_size));
    }

    let source =
        fs::read(path).with_context(|| format!("Failed to read PNG '{}'", path.display()))?;

    // oxipng changes compression only. Do not quantize, resize, or strip color
    // data: visual-regression artifacts must retain their decoded pixels.
    let oxipng_level = (10 - args.speed).clamp(0, 6) as u8;
    let options = oxipng::Options::from_preset(oxipng_level);
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
}
