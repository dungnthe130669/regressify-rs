use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, anyhow};
use colored::Colorize;
use rayon::prelude::*;
use rgb::Rgba;
use walkdir::WalkDir;

use crate::{
    cli::args::OptimizeArgs,
    common,
    helpers::{OnceLockExtensions, format_bytes},
};

enum OptimizeResult {
    Pair(usize, usize, u64, u64), // (width, height, original_size, optimized_size)
    TooSmall(u64),                // original_size
    TooLowQuality(usize, usize, u64), // (width, height, original_size)
    AlreadyOptimized(usize, usize, u64), // (width, height, original_size)
}

pub fn optimize_images(args: OptimizeArgs) -> Result<()> {
    eprintln!("Starting image optimization...");
    let src_path = common::ROOT_DIR.get_required()?;

    eprintln!("Searching for PNGs in: {:?}", src_path);

    let png_files: Vec<_> = WalkDir::new(src_path)
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) => Some(entry),
            Err(err) => {
                eprintln!("Error scanning entry: {}", err);
                None
            }
        })
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_owned())
        .filter(|path| {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .collect();

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

    let to_be_print_nth = match png_files.len() {
        n if n < 50 => 1,
        n if n < 200 => 2,
        n if n < 500 => 5,
        n if n < 2000 => 10,
        _ => 20,
    };
    let nth_width = png_files.len().to_string().len();
    png_files
        .par_iter()
        .try_for_each(|path| -> Result<()> {
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown.png");

            let result = optimize_png(path, &args);

            let idx = counter.fetch_add(1, Ordering::Relaxed) + 1;

            if !idx.is_multiple_of(to_be_print_nth) && idx != total_files {
                return Ok(());
            }

            match result {
                Ok(OptimizeResult::Pair(width, height, original_size, optimized_size)) => {
                    eprintln!(
                        "{:>width$} of {}: {}: {} --> {} ({}x{})",
                        idx,
                        total_files,
                        file_name,
                        format_bytes(original_size).yellow(),
                        format_bytes(optimized_size).green(),
                        width,
                        height,
                        width = nth_width
                    );
                }
                Ok(OptimizeResult::AlreadyOptimized(width, height, original_size)) => {
                    eprintln!(
                        "{:>width$} of {}: {}: Already optimized: {} ({}x{}).",
                        idx,
                        total_files,
                        file_name,
                        format_bytes(original_size).yellow(),
                        width,
                        height,
                        width = nth_width
                    );
                }
                Ok(OptimizeResult::TooSmall(original_size)) => {
                    eprintln!(
                        "{:>width$} of {}: {}: {} ({}).",
                        idx,
                        total_files,
                        file_name,
                        "Too Small".blue(),
                        format_bytes(original_size),
                        width = nth_width
                    );
                }
                Ok(OptimizeResult::TooLowQuality(width, height, original_size)) => {
                    eprintln!(
                        "{:>width$} of {}: {}: {}: {} ({}x{}) - quality too low",
                        idx,
                        total_files,
                        file_name,
                        "Skipped".cyan(),
                        format_bytes(original_size),
                        width,
                        height,
                        width = nth_width
                    );
                }
                Err(e) => {
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
            };

            Ok(())
        })
        .ok();

    Ok(())
}

fn optimize_png(path: &Path, args: &OptimizeArgs) -> Result<OptimizeResult> {
    // 1. If the file siz is less than min file size, skip optimization
    let metadata = std::fs::metadata(path)?;
    let original_size = metadata.len();
    if original_size < args.min_size_kb * 1024 {
        return Ok(OptimizeResult::TooSmall(original_size));
    }

    // 2. Load the PNG file into an RGBA pixel buffer
    let mut decoder = lodepng::Decoder::new();
    let image = match decoder.decode_file(path)? {
        lodepng::Image::RGBA(img) => img,
        _ => return Err(anyhow!("Only RGBA PNGs are supported in this sample")),
    };

    let width = image.width;
    let height = image.height;

    // 3. Initialize imagequant library
    let mut liq = imagequant::new();
    liq.set_quality(args.min_quality, args.max_quality)?;
    liq.set_speed(args.speed)?;
    liq.set_min_posterization(4)?;
    liq.set_max_colors(8)?;

    // Create the imagequant image object
    let mut liq_image = liq.new_image(image.buffer, image.width, image.height, 0.0)?;

    // 4. Quantize the image
    let mut res = match liq.quantize(&mut liq_image) {
        Ok(res) => res,
        Err(err) => {
            if format!("{}", err).contains("QUALITY_TOO_LOW") {
                return Ok(OptimizeResult::TooLowQuality(width, height, original_size));
            }
            return Err(err.into());
        }
    };

    // 5. DISABLE DITHERING (Crucial for visual regression stability)
    // 1.0 is default, 0.0 is no dithering.
    res.set_dithering_level(0.0)?;

    // 6. Get the result (palette and 8-bit pixels)
    let (palette, pixels) = res.remapped(&mut liq_image)?;

    // 7. Write the 8-bit PNG back to disk
    let mut encoder = lodepng::Encoder::new();

    // Configure encoder for Palette output
    {
        let info = encoder.info_png_mut();
        info.color.colortype = lodepng::ColorType::PALETTE;
        info.color.set_bitdepth(8);
    }

    // Configure input raw format (pixels are palette indices)
    {
        let info = encoder.info_raw_mut();
        info.colortype = lodepng::ColorType::PALETTE;
        info.set_bitdepth(8);
    }

    // Convert imagequant palette to lodepng palette
    for color in palette {
        let c = Rgba::new(color.r, color.g, color.b, color.a);
        encoder.info_png_mut().color.palette_add(c)?;
        encoder.info_raw_mut().palette_add(c)?;
    }

    // Generate PNG bytes in memory
    let buffer = encoder.encode(&pixels, image.width, image.height)?;

    // 8. Optimize with oxipng
    // Level 6 is a good balance for compression/speed, but we want minimal size so we can go higher (max is 6 in oxipng API for "default" presets usually,
    // or we can manually configure). `oxipng::Options::from_preset(6)` is robust.
    // Calculate oxipng level based on speed arg. speed 1 = level 6 (slowest), speed 10 = level 0 (fastest)
    let oxipng_level = (10 - args.speed).clamp(0, 6) as u8;
    let mut options = oxipng::Options::from_preset(oxipng_level);
    options.strip = oxipng::StripChunks::All;

    let optimized_buffer = oxipng::optimize_from_memory(&buffer, &options)?;

    // 9. Check file size reduction
    let optimized_size = optimized_buffer.len() as u64;
    if optimized_size >= original_size {
        return Ok(OptimizeResult::AlreadyOptimized(
            width,
            height,
            original_size,
        )); // No size reduction, skip writing
    }

    // 10. Write optimized PNG back to disk
    // Overwrite the original file
    // Fail silently if write fails
    std::fs::write(path, optimized_buffer).ok();

    Ok(OptimizeResult::Pair(
        width,
        height,
        original_size,
        optimized_size,
    ))
}
