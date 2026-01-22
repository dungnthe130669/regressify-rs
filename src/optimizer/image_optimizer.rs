use std::path::Path;

use anyhow::{Context, Result, anyhow};
use rgb::Rgba;
use walkdir::WalkDir;

use crate::{
    cli::args::OptimizeArgs,
    common,
    helpers::{IOExtensions, OnceLockExtensions},
};

pub fn optimize_images(args: OptimizeArgs) -> Result<()> {
    eprintln!("Starting image optimization...");
    let src_path = common::ROOT_DIR.get_required()?;

    for entry in WalkDir::new(src_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "png" => {
                let normalized_path = path.normalize()?;
                let relative_path = if let Ok(p) = normalized_path.strip_prefix(src_path) {
                    p
                } else {
                    path
                };
                eprintln!("Optimizing PNG: {:?}", relative_path.display());
                optimize_png(path, &args)
                    .with_context(|| format!("Failed to optimize PNG: {:?}", path))?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn optimize_png(path: &Path, args: &OptimizeArgs) -> Result<()> {
    // 1. Load the PNG file into an RGBA pixel buffer
    let mut decoder = lodepng::Decoder::new();
    let image = match decoder.decode_file(path)? {
        lodepng::Image::RGBA(img) => img,
        _ => return Err(anyhow!("Only RGBA PNGs are supported in this sample")),
    };

    // 2. Initialize imagequant library
    let mut liq = imagequant::new();
    liq.set_quality(args.min_quality, args.max_quality)?;
    liq.set_speed(args.speed)?;
    liq.set_min_posterization(4)?;
    liq.set_max_colors(8)?;
    // 3. SETTINGS FOR MINIMAL SIZE & DETERMINISTIC PIXELS
    // Set max colors low (e.g., 16 or 32) for UI regression tests
    liq.set_quality(0, 20)?;
    liq.set_speed(1)?; // Slowest speed = best compression

    // Create the imagequant image object
    let mut liq_image = liq.new_image(image.buffer, image.width, image.height, 0.0)?;

    // 4. Quantize the image
    let mut res = liq.quantize(&mut liq_image)?;

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
    let mut options = oxipng::Options::from_preset(6);
    options.strip = oxipng::StripChunks::All;

    let optimized_buffer = oxipng::optimize_from_memory(&buffer, &options)?;

    // 9. Write optimized PNG back to disk
    // Overwrite the original file
    // Fail silently if write fails
    std::fs::write(path, optimized_buffer).ok();

    Ok(())
}
