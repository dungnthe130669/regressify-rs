use std::{fs::File, io::Read, path::Path};

use anyhow::{Result, anyhow};

pub(crate) const MAX_VISUAL_IMAGE_DIMENSION: u32 = 100_000;
pub(crate) const MAX_VISUAL_IMAGE_PIXELS: u64 = 64_000_000;
pub(crate) const MAX_VISUAL_IMAGE_DECOMPRESSED_BYTES: usize = 256_100_000;

/// Read and bound PNG IHDR dimensions before a decoder allocates image data.
pub(crate) fn read_bounded_png_dimensions(path: &Path) -> Result<(usize, usize)> {
    read_bounded_png_dimensions_from_reader(File::open(path)?, path)
}

pub(crate) fn read_bounded_png_dimensions_from_reader(
    mut reader: impl Read,
    path: &Path,
) -> Result<(usize, usize)> {
    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    let mut header = [0_u8; 24];
    reader
        .read_exact(&mut header)
        .map_err(|error| anyhow!("Failed to read PNG header '{}': {}", path.display(), error))?;

    if header[..8] != PNG_SIGNATURE {
        anyhow::bail!("Invalid PNG signature in '{}'", path.display());
    }
    if header[8..12] != [0, 0, 0, 13] || header[12..16] != *b"IHDR" {
        anyhow::bail!("PNG '{}' has no valid IHDR header", path.display());
    }

    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| anyhow!("PNG '{}' dimensions overflow", path.display()))?;
    if width == 0
        || height == 0
        || width > MAX_VISUAL_IMAGE_DIMENSION
        || height > MAX_VISUAL_IMAGE_DIMENSION
        || pixel_count > MAX_VISUAL_IMAGE_PIXELS
    {
        anyhow::bail!(
            "PNG '{}' dimensions {}x{} exceed the {}x{} / {} pixel limit",
            path.display(),
            width,
            height,
            MAX_VISUAL_IMAGE_DIMENSION,
            MAX_VISUAL_IMAGE_DIMENSION,
            MAX_VISUAL_IMAGE_PIXELS
        );
    }

    Ok((width as usize, height as usize))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut header = Vec::from([137, 80, 78, 71, 13, 10, 26, 10]);
        header.extend_from_slice(&13_u32.to_be_bytes());
        header.extend_from_slice(b"IHDR");
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header
    }

    #[test]
    fn reads_bounded_png_dimensions_from_ihdr() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("image.png");
        fs::write(&path, png_header(100, 200)).unwrap();

        assert_eq!(read_bounded_png_dimensions(&path).unwrap(), (100, 200));
    }

    #[test]
    fn rejects_oversized_png_dimensions_before_decode() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("oversized.png");
        fs::write(
            &path,
            png_header(MAX_VISUAL_IMAGE_DIMENSION, MAX_VISUAL_IMAGE_DIMENSION),
        )
        .unwrap();

        let error = read_bounded_png_dimensions(&path).unwrap_err();

        assert!(error.to_string().contains("pixel limit"));
    }
}
