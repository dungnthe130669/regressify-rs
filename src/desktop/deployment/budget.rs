use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{
    bounded_read, phase, read_report_configuration, report_image_relative, validate_suite_id,
};
use crate::helpers::png::{
    MAX_VISUAL_IMAGE_DECOMPRESSED_BYTES, read_bounded_png_dimensions_from_reader,
};

const MIB: u64 = 1024 * 1024;
const QUALITY_STEPS: [u8; 8] = [99, 95, 90, 85, 80, 70, 60, 50];
const MIN_COMPARISON_QUALITY: u8 = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SwaTier {
    Free,
    Standard,
    Unknown,
}

impl SwaTier {
    pub fn from_response(info: &Value) -> Self {
        match info["sku"].as_str() {
            Some(sku) if sku.eq_ignore_ascii_case("Standard") => Self::Standard,
            Some(sku) if sku.eq_ignore_ascii_case("Free") => Self::Free,
            _ => Self::Unknown,
        }
    }

    pub fn limit(self) -> u64 {
        match self {
            Self::Standard => 500 * MIB,
            Self::Free | Self::Unknown => 250 * MIB,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Standard => "Standard",
            Self::Unknown => "unknown (using the Free limit conservatively)",
        }
    }
}

pub(super) struct PreparedReport {
    source: PathBuf,
    temporary: Option<tempfile::TempDir>,
}

impl PreparedReport {
    pub fn path(&self) -> &Path {
        self.temporary
            .as_ref()
            .map_or(&self.source, |directory| directory.path())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Passed,
    Comparison,
    #[default]
    Protected,
}

struct ReportFile {
    relative: PathBuf,
    bytes: u64,
    priority: Priority,
    quality: u8,
    optimizable: bool,
}

fn inventory(root: &Path) -> Result<(Vec<ReportFile>, u64)> {
    let mut files = Vec::new();
    let mut total = 0_u64;
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry.context("Cannot inspect deployment content")?;
        ensure!(
            !entry.file_type().is_symlink(),
            "Deployment content cannot contain symlinks"
        );
        if entry.file_type().is_dir() {
            continue;
        }
        ensure!(
            entry.file_type().is_file(),
            "Deployment content must contain regular files"
        );
        let bytes = entry.metadata()?.len();
        total = total
            .checked_add(bytes)
            .context("Deployment content size overflow")?;
        files.push(ReportFile {
            relative: entry.path().strip_prefix(root)?.to_path_buf(),
            bytes,
            priority: Priority::Protected,
            quality: 100,
            optimizable: true,
        });
        ensure!(
            files.len() <= 100_000,
            "Azure deployment exceeds the 100000-file package limit; select fewer suites"
        );
    }
    files.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.relative.cmp(&right.relative))
    });
    Ok((files, total))
}

fn check_deadline(deadline: Instant) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "Report optimization reached the deployment deadline; deploy fewer suites or retry. Original reports and baselines are unchanged."
    );
    Ok(())
}

fn classify_images(root: &Path, files: &mut [ReportFile], deadline: Instant) -> Result<()> {
    let mut priorities = BTreeMap::<PathBuf, Priority>::new();
    for file in files
        .iter()
        .filter(|file| file.relative.ends_with("html_report/config.js"))
    {
        check_deadline(deadline)?;
        ensure!(
            file.relative.components().count() == 3,
            "Invalid deployment report layout"
        );
        let suite = file
            .relative
            .iter()
            .next()
            .and_then(|value| value.to_str())
            .context("Report suite path must be UTF-8")?;
        validate_suite_id(suite)?;
        let data = read_report_configuration(&root.join(&file.relative))?;
        ensure!(
            data["id"].as_str() == Some(suite),
            "Report suite identity changed"
        );
        for test in data["tests"].as_array().context("Report has no tests")? {
            let error = test["pair"]
                .get("engineErrorMsg")
                .is_some_and(|value| !value.is_null() && value.as_str() != Some(""));
            for key in ["reference", "test", "diffImage"] {
                let Some(image) = test["pair"][key].as_str().filter(|image| !image.is_empty())
                else {
                    continue;
                };
                let priority = if error {
                    Priority::Protected
                } else if test["status"] == "pass" {
                    Priority::Passed
                } else if key == "diffImage" {
                    Priority::Protected
                } else {
                    Priority::Comparison
                };
                priorities
                    .entry(report_image_relative(suite, key, image)?)
                    .and_modify(|previous| *previous = (*previous).max(priority))
                    .or_insert(priority);
            }
        }
    }
    for file in files {
        file.priority = priorities.get(&file.relative).copied().unwrap_or_default();
    }
    Ok(())
}

fn read_png(path: &Path) -> Result<Vec<u8>> {
    let bytes = bounded_read(fs::File::open(path)?, 128 * MIB as usize, "Deployment PNG")?;
    read_bounded_png_dimensions_from_reader(bytes.as_slice(), path)?;
    Ok(bytes)
}

fn quantize(
    image: &lodepng::Bitmap<rgb::RGBA8>,
    quality: u8,
    deadline: Instant,
) -> Result<Option<Vec<u8>>> {
    check_deadline(deadline)?;
    let mut attributes = imagequant::new();
    attributes.set_speed(6)?;
    attributes.set_quality(quality, quality)?;
    let progress = move |_| {
        if Instant::now() < deadline {
            imagequant::ControlFlow::Continue
        } else {
            imagequant::ControlFlow::Break
        }
    };
    attributes.set_progress_callback(progress);
    let mut input = attributes.new_image_borrowed(&image.buffer, image.width, image.height, 0.0)?;
    let mut result = match attributes.quantize(&mut input) {
        Ok(result) => result,
        Err(imagequant::Error::QualityTooLow) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    result.set_dithering_level(1.0)?;
    result.set_progress_callback(progress);
    let (palette, indices) = result.remapped(&mut input)?;
    if result
        .remapping_quality()
        .is_some_and(|actual| actual < quality)
    {
        return Ok(None);
    }
    let mut encoder = lodepng::Encoder::new();
    encoder.set_palette(&palette)?;
    Ok(Some(encoder.encode(&indices, image.width, image.height)?))
}

fn refine_candidate(
    mut bytes: Vec<u8>,
    mut quality: u8,
    previous_quality: u8,
    target_bytes: u64,
    mut encode: impl FnMut(u8) -> Result<Option<Vec<u8>>>,
) -> Result<(Vec<u8>, u8)> {
    if bytes.len() as u64 <= target_bytes {
        for higher in (quality + 1..previous_quality).rev() {
            if let Some(candidate) = encode(higher)?
                && candidate.len() as u64 <= target_bytes
            {
                bytes = candidate;
                quality = higher;
                break;
            }
        }
    }
    Ok((bytes, quality))
}

fn reduce_images(
    root: &Path,
    destination: &Path,
    files: &mut [ReportFile],
    total: &mut u64,
    limit: u64,
    skipped: &mut usize,
    deadline: Instant,
) -> Result<()> {
    for priority in [Priority::Passed, Priority::Comparison] {
        let mut previous_quality = 100;
        for quality in QUALITY_STEPS {
            if *total <= limit
                || priority == Priority::Comparison && quality < MIN_COMPARISON_QUALITY
            {
                break;
            }
            let label = if priority == Priority::Passed {
                "passed comparisons"
            } else {
                "failed Reference/Test images"
            };
            let candidates = files
                .iter()
                .filter(|file| file.priority == priority && file.optimizable)
                .count();
            if candidates == 0 {
                break;
            }
            phase(&format!(
                "Optimizing {label}: quality floor {quality}, {:.2} MiB still needed",
                (*total - limit) as f64 / MIB as f64
            ));
            files.sort_by(|left, right| {
                right
                    .bytes
                    .cmp(&left.bytes)
                    .then_with(|| left.relative.cmp(&right.relative))
            });
            let mut last_update = Instant::now();
            for (index, file) in files
                .iter_mut()
                .filter(|file| file.priority == priority && file.optimizable)
                .enumerate()
            {
                check_deadline(deadline)?;
                if *total <= limit {
                    break;
                }
                let result = (|| -> Result<Option<(Vec<u8>, u8)>> {
                    let image = lodepng::decode32(&read_png(&root.join(&file.relative))?)?;
                    let Some(bytes) = quantize(&image, quality, deadline)? else {
                        return Ok(None);
                    };
                    if bytes.len() as u64 >= file.bytes {
                        return Ok(None);
                    }
                    let target_bytes = file.bytes.saturating_sub(*total - limit);
                    Ok(Some(refine_candidate(
                        bytes,
                        quality,
                        previous_quality,
                        target_bytes,
                        |higher| quantize(&image, higher, deadline),
                    )?))
                })();
                check_deadline(deadline)?;
                if index == 0
                    || (index + 1).is_multiple_of(10)
                    || last_update.elapsed().as_secs() >= 1
                {
                    phase(&format!(
                        "Optimizing {label}: image {} of {candidates}, quality floor {quality}; report {:.2} MiB",
                        index + 1,
                        *total as f64 / MIB as f64
                    ));
                    last_update = Instant::now();
                }
                match result {
                    Ok(Some((bytes, applied_quality))) => {
                        crate::helpers::write_file_atomically(
                            &destination.join(&file.relative),
                            &bytes,
                        )?;
                        *total -= file.bytes - bytes.len() as u64;
                        file.bytes = bytes.len() as u64;
                        file.quality = applied_quality;
                        phase(&format!(
                            "Temporary image optimized: {}; quality floor {applied_quality}; report {:.2} MiB",
                            file.relative.display(),
                            *total as f64 / MIB as f64
                        ));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        file.optimizable = false;
                        *skipped += 1;
                        phase(&format!(
                            "Keeping image unchanged: {} ({error})",
                            file.relative.display()
                        ));
                    }
                }
            }
            previous_quality = quality;
        }
    }
    Ok(())
}

pub(super) fn prepare(root: &Path, limit: u64, deadline: Instant) -> Result<PreparedReport> {
    check_deadline(deadline)?;
    phase("Measuring uncompressed Azure report content");
    let (mut files, original_bytes) = inventory(root)?;
    phase(&format!(
        "Deployment content: {:.2} MiB; storage budget: {:.2} MiB (uncompressed)",
        original_bytes as f64 / MIB as f64,
        limit as f64 / MIB as f64
    ));
    let mut prepared = PreparedReport {
        source: root.to_path_buf(),
        temporary: None,
    };
    if original_bytes <= limit {
        return Ok(prepared);
    }

    classify_images(root, &mut files, deadline)?;
    let temporary = tempfile::Builder::new()
        .prefix("regressify-deploy-")
        .tempdir()
        .context(
            "Cannot create temporary report optimization directory; check available disk space",
        )?;
    phase("Creating an isolated temporary copy for report optimization");
    let mut last_update = Instant::now();
    for (index, file) in files.iter().enumerate() {
        check_deadline(deadline)?;
        let destination = temporary.path().join(&file.relative);
        fs::create_dir_all(
            destination
                .parent()
                .context("Deployment file has no parent")?,
        )?;
        let copied = fs::copy(root.join(&file.relative), &destination).with_context(|| {
            format!(
                "Cannot copy '{}' to temporary deployment storage; check available disk space",
                file.relative.display()
            )
        })?;
        ensure!(
            copied == file.bytes,
            "Report changed while preparing deployment; regenerate it and retry"
        );
        if index == 0 || (index + 1).is_multiple_of(50) || last_update.elapsed().as_secs() >= 1 {
            phase(&format!(
                "Temporary optimization copy: {} of {} files",
                index + 1,
                files.len()
            ));
            last_update = Instant::now();
        }
    }
    prepared.temporary = Some(temporary);
    let mut total = original_bytes;
    let mut skipped = 0;
    phase("Optimizing temporary report images losslessly");
    let mut options = oxipng::Options::from_preset(1);
    options.optimize_alpha = false;
    options.scale_16 = false;
    options.interlace = None;
    options.strip = oxipng::StripChunks::None;
    options.max_decompressed_size = Some(MAX_VISUAL_IMAGE_DECOMPRESSED_BYTES);
    let image_count = files
        .iter()
        .filter(|file| {
            file.relative
                .extension()
                .is_some_and(|extension| extension == "png")
        })
        .count();
    let mut processed = 0_usize;
    let mut last_update = Instant::now();
    for file in &mut files {
        check_deadline(deadline)?;
        if total <= limit {
            break;
        }
        if file
            .relative
            .extension()
            .is_none_or(|extension| extension != "png")
        {
            continue;
        }
        let source = root.join(&file.relative);
        options.timeout = Some(deadline.saturating_duration_since(Instant::now()));
        let optimized = (|| -> Result<Vec<u8>> {
            Ok(oxipng::optimize_from_memory(&read_png(&source)?, &options)?)
        })();
        check_deadline(deadline)?;
        processed += 1;
        match optimized {
            Ok(bytes) if (bytes.len() as u64) < file.bytes => {
                crate::helpers::write_file_atomically(
                    &prepared.path().join(&file.relative),
                    &bytes,
                )?;
                total -= file.bytes - bytes.len() as u64;
                file.bytes = bytes.len() as u64;
            }
            Ok(_) => {}
            Err(error) => {
                skipped += 1;
                file.optimizable = false;
                phase(&format!(
                    "Keeping image unchanged: {} ({error})",
                    file.relative.display()
                ));
            }
        }
        if processed == 1 || processed.is_multiple_of(10) || last_update.elapsed().as_secs() >= 1 {
            phase(&format!(
                "Lossless optimization: image {processed} of {image_count}; report {:.2} MiB",
                total as f64 / MIB as f64
            ));
            last_update = Instant::now();
        }
    }
    if total > limit {
        reduce_images(
            root,
            prepared.path(),
            &mut files,
            &mut total,
            limit,
            &mut skipped,
            deadline,
        )?;
    }
    ensure!(
        total <= limit,
        "Report cannot fit the Azure storage limit: {:.2} MiB originally, {:.2} MiB after optimization, {:.2} MiB allowed ({skipped} images could not be optimized). Failed diffs, error placeholders, and image dimensions were preserved. Select fewer suites or upgrade Free to Standard; also check Azure's total storage usage across preview environments. Original reports and baselines are unchanged.",
        original_bytes as f64 / MIB as f64,
        total as f64 / MIB as f64,
        limit as f64 / MIB as f64
    );
    let reduced = files.iter().filter(|file| file.quality < 100).count();
    phase(&format!(
        "Temporary deployment report: {:.2} -> {:.2} MiB; {reduced} images quality-reduced; {skipped} skipped; original reports and baselines unchanged",
        original_bytes as f64 / MIB as f64,
        total as f64 / MIB as f64
    ));
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn deadline() -> Instant {
        Instant::now() + std::time::Duration::from_secs(60)
    }

    fn sample_png() -> Result<Vec<u8>> {
        let mut state = 12345_u32;
        let pixels = (0..256 * 128)
            .map(|index| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let base = if index / 256 % 32 < 4 { 20 } else { 220 };
                rgb::RGBA8::new(
                    base + (state & 15) as u8,
                    base + ((state >> 8) & 15) as u8,
                    base + ((state >> 16) & 15) as u8,
                    if index < 256 { 0 } else { 255 },
                )
            })
            .collect::<Vec<_>>();
        Ok(lodepng::encode32(&pixels, 256, 128)?)
    }

    #[test]
    fn palette_reduction_preserves_dimensions_and_transparency() -> Result<()> {
        let original = sample_png()?;
        let image = lodepng::decode32(&original)?;
        let bytes =
            quantize(&image, 80, deadline())?.context("Fixture should meet quality floor")?;
        assert!(bytes.len() < original.len());
        let optimized = lodepng::decode32(&bytes)?;
        assert_eq!(
            (optimized.width, optimized.height),
            (image.width, image.height)
        );
        assert_eq!(optimized.buffer[0].a, 0);
        assert_eq!(optimized.buffer[256].a, 255);
        Ok(())
    }

    #[test]
    fn final_image_uses_the_highest_quality_that_actually_fits() -> Result<()> {
        let mut attempted = Vec::new();
        let (bytes, quality) = refine_candidate(vec![0; 40], 80, 90, 50, |quality| {
            attempted.push(quality);
            Ok(Some(vec![0; if quality == 88 { 49 } else { 60 }]))
        })?;
        assert_eq!(attempted, [89, 88]);
        assert_eq!(quality, 88);
        assert_eq!(bytes.len(), 49);
        Ok(())
    }

    fn report_fixture() -> Result<tempfile::TempDir> {
        let root = tempfile::tempdir()?;
        fs::create_dir_all(root.path().join("one/html_report"))?;
        fs::create_dir_all(root.path().join("one/bitmaps_reference"))?;
        fs::create_dir_all(root.path().join("one/bitmaps_test/run"))?;
        fs::write(root.path().join("index.html"), "report")?;
        fs::write(root.path().join("one/html_report/index.html"), "suite")?;
        let png = sample_png()?;
        let mut tests = Vec::new();
        for (label, status) in [("passed", "pass"), ("failed", "fail")] {
            fs::write(
                root.path()
                    .join(format!("one/bitmaps_reference/{label}.png")),
                &png,
            )?;
            fs::write(
                root.path()
                    .join(format!("one/bitmaps_test/run/{label}.png")),
                &png,
            )?;
            fs::write(
                root.path()
                    .join(format!("one/bitmaps_test/run/{label}-diff.png")),
                &png,
            )?;
            tests.push(json!({"status":status,"pair":{
                "reference":format!("../bitmaps_reference/{label}.png?original-hash"),
                "test":format!("../bitmaps_test/run/{label}.png?original-hash"),
                "diffImage":format!("../bitmaps_test/run/{label}-diff.png?original-hash"),
                "diff":{"misMatchPercentage":"12.34"}}}));
        }
        fs::write(
            root.path().join("one/html_report/config.js"),
            format!("report({});", json!({"id":"one","tests":tests})),
        )?;
        Ok(root)
    }

    #[test]
    fn oversized_reports_reduce_passed_images_in_temp_and_preserve_failures_and_sources()
    -> Result<()> {
        let root = report_fixture()?;
        let (files, original_total) = inventory(root.path())?;
        let before = files
            .iter()
            .map(|file| {
                Ok((
                    file.relative.clone(),
                    fs::read(root.path().join(&file.relative))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let limit = original_total * 85 / 100;
        let prepared = prepare(root.path(), limit, deadline())?;
        assert_ne!(prepared.path(), root.path());
        assert!(inventory(prepared.path())?.1 <= limit);
        let package = super::super::package::Package::create(prepared.path(), 100_000)?;
        assert!(package.bytes <= limit);
        assert_eq!(package.files as usize, before.len());
        let mut archive = zip::ZipArchive::new(package.file.reopen()?)?;
        let mut reduced = 0;
        for (relative, bytes) in &before {
            assert_eq!(fs::read(root.path().join(relative))?, *bytes);
            let uploaded = fs::read(prepared.path().join(relative))?;
            let mut archived = Vec::new();
            std::io::Read::read_to_end(
                &mut archive.by_name(&relative.to_string_lossy().replace('\\', "/"))?,
                &mut archived,
            )?;
            assert_eq!(archived, uploaded);
            if relative
                .extension()
                .is_some_and(|extension| extension == "png")
            {
                let original = lodepng::decode32(bytes)?;
                let optimized = lodepng::decode32(&uploaded)?;
                assert_eq!(
                    (original.width, original.height),
                    (optimized.width, optimized.height)
                );
                assert_eq!(optimized.buffer[0].a, 0);
                if relative.to_string_lossy().contains("failed") {
                    assert_eq!(original.buffer, optimized.buffer);
                } else if original.buffer != optimized.buffer {
                    reduced += 1;
                }
            } else {
                assert_eq!(uploaded, *bytes);
            }
        }
        assert!(reduced > 0, "Fixture must require quality reduction");
        let temporary = prepared.path().to_path_buf();
        drop(prepared);
        assert!(!temporary.exists());
        Ok(())
    }

    #[test]
    fn shared_images_take_the_more_protective_priority() -> Result<()> {
        let root = report_fixture()?;
        let config = root.path().join("one/html_report/config.js");
        let mut data = read_report_configuration(&config)?;
        data["tests"][1]["pair"]["reference"] = data["tests"][0]["pair"]["reference"].clone();
        data["tests"][1]["pair"]["engineErrorMsg"] = json!("Capture failed");
        fs::write(config, format!("report({data});"))?;
        let (mut files, _) = inventory(root.path())?;
        classify_images(root.path(), &mut files, deadline())?;
        let priorities = files
            .into_iter()
            .map(|file| (file.relative, file.priority))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            priorities[Path::new("one/bitmaps_reference/passed.png")],
            Priority::Protected
        );
        assert_eq!(
            priorities[Path::new("one/bitmaps_test/run/passed.png")],
            Priority::Passed
        );
        assert_eq!(
            priorities[Path::new("one/bitmaps_test/run/failed-diff.png")],
            Priority::Protected
        );
        Ok(())
    }

    #[test]
    fn storage_budget_uses_the_detected_tier_and_falls_back_conservatively() {
        for (info, expected) in [
            (json!({"sku":"Free"}), 250),
            (json!({"sku":"Standard"}), 500),
            (json!({"sku":"standard"}), 500),
            (json!({}), 250),
            (json!({"sku":"unexpected"}), 250),
        ] {
            assert_eq!(SwaTier::from_response(&info).limit(), expected * MIB);
        }
    }

    #[test]
    fn fitting_reports_are_byte_identical_and_need_no_optimization_copy() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("index.html"), "report")?;
        let prepared = prepare(root.path(), 6, deadline())?;
        assert!(prepared.temporary.is_none());
        assert_eq!(prepared.path(), root.path());
        assert_eq!(fs::read(root.path().join("index.html"))?, b"report");
        Ok(())
    }

    #[test]
    fn an_unfittable_report_never_changes_source_files() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("index.html"), "report")?;
        let path = root.path().join("capture.png");
        let pixels = vec![rgb::RGBA8::new(10, 20, 30, 255); 64 * 64];
        let original = lodepng::encode32(&pixels, 64, 64)?;
        fs::write(&path, &original)?;
        let error = prepare(root.path(), 1, deadline())
            .err()
            .context("Expected a size failure")?;
        assert!(
            error
                .to_string()
                .contains("Original reports and baselines are unchanged")
        );
        assert_eq!(fs::read(path)?, original);
        assert_eq!(fs::read(root.path().join("index.html"))?, b"report");
        Ok(())
    }

    #[test]
    fn failed_reference_and_test_can_shrink_without_changing_failed_diffs() -> Result<()> {
        let root = report_fixture()?;
        let config = root.path().join("one/html_report/config.js");
        let mut data = read_report_configuration(&config)?;
        data["tests"][0]["status"] = json!("fail");
        fs::write(&config, format!("report({data});"))?;
        let (_, original_total) = inventory(root.path())?;
        let prepared = prepare(root.path(), original_total * 85 / 100, deadline())?;
        let mut reduced = 0;
        for file in inventory(root.path())?.0.into_iter().filter(|file| {
            file.relative
                .extension()
                .is_some_and(|extension| extension == "png")
        }) {
            let original = lodepng::decode32(&fs::read(root.path().join(&file.relative))?)?;
            let uploaded = lodepng::decode32(&fs::read(prepared.path().join(&file.relative))?)?;
            if file.relative.to_string_lossy().contains("-diff") {
                assert_eq!(original.buffer, uploaded.buffer);
            } else if original.buffer != uploaded.buffer {
                reduced += 1;
            }
        }
        assert!(reduced > 0);
        assert_eq!(
            read_report_configuration(&prepared.path().join("one/html_report/config.js"))?,
            data
        );
        Ok(())
    }

    #[test]
    fn expired_deadlines_abort_without_changing_the_source() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("index.html"), "report")?;
        let error = prepare(root.path(), 1, Instant::now())
            .err()
            .context("Expected deadline")?;
        assert!(error.to_string().contains("deadline"));
        assert_eq!(fs::read(root.path().join("index.html"))?, b"report");
        Ok(())
    }
}
