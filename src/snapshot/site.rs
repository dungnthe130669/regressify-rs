use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Deserialize, Serialize};

use super::helpers::get_asset_version;
use crate::{cli::args::SnapshotArgs, config::WorkspaceConfig, helpers::write_file_atomically};

include!("data/assets.rs");

pub(super) const MAX_REPORT_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_IMAGE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Manifest {
    pub version: u8,
    pub title: String,
    pub heading: String,
    pub subheading: String,
    pub generated_at: String,
    pub suites: Vec<SuiteSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct SuiteSummary {
    pub id: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub report: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SuiteReport {
    pub id: String,
    pub run_id: String,
    pub tests: Vec<Capture>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Capture {
    pub id: String,
    pub label: String,
    pub scenario: String,
    pub selector: String,
    pub status: String,
    pub threshold: f64,
    pub mismatch: Option<f64>,
    pub error: Option<String>,
    pub reference: Option<String>,
    pub test: Option<String>,
    pub diff: Option<String>,
    pub reference_url: Option<String>,
    pub test_url: Option<String>,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Dimensions {
    pub same: bool,
    pub width: i64,
    pub height: i64,
}

pub(super) fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.chars().any(char::is_control)
        && !value.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
        && !value.ends_with(['.', ' '])
        && !crate::helpers::is_windows_reserved_path_component(value)
}

fn normal_relative(relative: &Path) -> Result<()> {
    ensure!(
        !relative.as_os_str().is_empty()
            && relative.components().all(|part| match part {
                Component::Normal(name) => name.to_str().is_some_and(valid_segment),
                _ => false,
            }),
        "Snapshot artifact path must remain inside its output directory"
    );
    Ok(())
}

pub(super) fn checked_file(
    root: &Path,
    relative: &Path,
    optional: bool,
) -> Result<Option<PathBuf>> {
    normal_relative(relative)?;
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "Snapshot artifacts cannot be symlinks: {}",
                relative.display()
            ),
            Err(error) if optional && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Snapshot artifact is unavailable: {}", relative.display())
                });
            }
        }
    }
    ensure!(
        path.is_file() && path.canonicalize()?.starts_with(root),
        "Snapshot artifact must be a regular file inside its output directory"
    );
    Ok(Some(path))
}

pub(super) fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = fs::File::open(path)
        .with_context(|| format!("Failed to open snapshot artifact '{}'", path.display()))?;
    ensure!(
        file.metadata()?.len() <= limit as u64,
        "Snapshot artifact exceeds its size limit: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "Snapshot artifact grew beyond its size limit"
    );
    Ok(bytes)
}

fn output_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    let relative = Path::new(relative);
    normal_relative(relative)?;
    let mut parent = root.to_path_buf();
    for component in relative
        .parent()
        .context("Snapshot path has no parent")?
        .components()
    {
        parent.push(component);
        match fs::symlink_metadata(&parent) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Snapshot output parent must be a real directory"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&parent)?,
            Err(error) => return Err(error.into()),
        }
    }
    if let Some(existing) = checked_file(root, relative, true)?
        && fs::read(&existing)? == bytes
    {
        return Ok(());
    }
    write_file_atomically(&root.join(relative), bytes)
}

fn encoded_path(relative: &str) -> Result<String> {
    ensure!(
        relative.split('/').all(valid_segment),
        "Invalid snapshot URL path"
    );
    const URL_SEGMENT: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    Ok(relative
        .split('/')
        .map(|part| utf8_percent_encode(part, URL_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

pub(super) fn store_asset(root: &Path, relative: &str, bytes: &[u8]) -> Result<String> {
    let kind = if relative.starts_with("_snapshot/data/") {
        "data"
    } else {
        "images"
    };
    let url = format!(
        "{}?v={}",
        encoded_path(relative)?,
        get_asset_version(bytes, 20)
    );
    versioned_path(&url, kind)?;
    output_file(root, relative, bytes)?;
    Ok(url)
}

pub(super) fn store_json(root: &Path, relative: &str, value: &impl Serialize) -> Result<String> {
    store_asset(root, relative, &serde_json::to_vec(value)?)
}

fn html_title(title: &str) -> String {
    title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn publish_index(root: &Path, manifest: &Manifest) -> Result<()> {
    let manifest_url = store_json(root, "_snapshot/data/manifest.json", manifest)?;
    let index = include_str!("data/index.html")
        .replace("__SNAPSHOT_MANIFEST__", &manifest_url)
        .replace("__SNAPSHOT_TITLE__", &html_title(&manifest.title));
    // Publish the entry point only after all of its versioned dependencies are complete.
    output_file(root, "index.html", index.as_bytes())
}

pub(crate) fn generate_site(
    root: &Path,
    suites: &[String],
    args: &SnapshotArgs,
    workspace: &WorkspaceConfig,
) -> Result<()> {
    let root = root.canonicalize()?;
    let staging = tempfile::Builder::new()
        .prefix(".snapshot-")
        .tempdir_in(&root)?;
    let staged_root = staging.path().canonicalize()?;
    let mut summaries = Vec::new();
    let mut images = BTreeMap::new();
    for id in suites {
        if let Some(report) =
            super::capture_report::load_suite(&root, &staged_root, id, &mut images)
                .with_context(|| format!("Failed to process snapshot suite '{id}'"))?
        {
            let passed = report
                .tests
                .iter()
                .filter(|capture| capture.status == "pass")
                .count();
            summaries.push(SuiteSummary {
                id: id.clone(),
                total: report.tests.len(),
                passed,
                failed: report.tests.len() - passed,
                errors: report
                    .tests
                    .iter()
                    .filter(|capture| capture.error.is_some())
                    .count(),
                report: store_json(
                    &staged_root,
                    &format!("_snapshot/data/suites/{id}.json"),
                    &report,
                )?,
            });
        }
    }
    summaries.sort_by(|first, second| first.id.cmp(&second.id));
    let manifest = Manifest {
        version: 2,
        title: args
            .title
            .as_deref()
            .or(workspace.snapshot_title.as_deref())
            .unwrap_or("Visual Tests Summary | Regressify")
            .into(),
        heading: args
            .heading
            .as_deref()
            .or(workspace.snapshot_heading.as_deref())
            .unwrap_or("Visual Tests Summary")
            .into(),
        subheading: args
            .subheading
            .as_deref()
            .or(workspace.snapshot_subheading.as_deref())
            .unwrap_or("Snapshot regression test results")
            .into(),
        generated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        suites: summaries,
    };
    for (name, bytes) in FRONTEND_FILES {
        output_file(
            &staged_root,
            if *name == "gitignore" {
                ".gitignore"
            } else {
                name
            },
            bytes,
        )?;
    }
    publish_index(&staged_root, &manifest)?;
    let selected = manifest
        .suites
        .iter()
        .map(|suite| suite.id.clone())
        .collect::<Vec<_>>();
    let dependencies = artifact_paths(&staged_root, &selected)?;
    // Invalid report input must never overwrite a stable path in the published site.
    for path in dependencies
        .iter()
        .filter(|path| path.as_path() != Path::new("index.html"))
    {
        output_file(
            &root,
            &path.to_string_lossy().replace('\\', "/"),
            &read_bounded(&staged_root.join(path), MAX_IMAGE_BYTES)?,
        )?;
    }
    output_file(
        &root,
        ".gitignore",
        &fs::read(staged_root.join(".gitignore"))?,
    )?;
    output_file(
        &root,
        "index.html",
        &fs::read(staged_root.join("index.html"))?,
    )?;
    prune_generated_files(&root, &dependencies)?;
    for suite in &manifest.suites {
        remove_legacy(&root, &format!("{}/html_report/index.html", suite.id))?;
        for name in [
            "assets",
            "diff.js",
            "diverged.js",
            "divergedWorker.js",
            "index_bundle.js",
            "index_bundle.js.LICENSE.txt",
        ] {
            remove_legacy(&root, &format!("{}/html_report/{name}", suite.id))?;
        }
    }
    for name in [
        "styles.css",
        "script.js",
        "index_bundle.js",
        "diff.js",
        "diverged.js",
        "divergedWorker.js",
        "Lato-Bold.ttf",
        "Lato-Regular.ttf",
        "lato-bold-webfont.woff",
        "lato-bold-webfont.woff2",
        "lato-regular-webfont.woff",
        "lato-regular-webfont.woff2",
        "favicon.ico",
        "favicon.svg",
        "favicon-16x16.png",
        "favicon-32x32.png",
        "apple-touch-icon.png",
    ] {
        remove_legacy(&root, name)?;
    }
    eprintln!(
        "Snapshot generated at: {}",
        root.join("index.html").display()
    );
    Ok(())
}

fn remove_legacy(root: &Path, relative: &str) -> Result<()> {
    normal_relative(Path::new(relative))?;
    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            ensure!(
                !metadata.file_type().is_symlink(),
                "Legacy snapshot artifacts cannot be symlinks"
            );
            if metadata.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn prune_generated_files(root: &Path, active: &[PathBuf]) -> Result<()> {
    let active: BTreeSet<_> = active.iter().collect();
    let entries = walkdir::WalkDir::new(root.join("_snapshot"))
        .follow_links(false)
        .contents_first(true)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(
        entries.iter().all(|entry| !entry.file_type().is_symlink()),
        "Generated snapshot files cannot be symlinks"
    );
    for entry in entries {
        if entry.file_type().is_file()
            && !active.contains(&entry.path().strip_prefix(root)?.to_path_buf())
        {
            fs::remove_file(entry.path())?;
        } else if entry.file_type().is_dir() && fs::read_dir(entry.path())?.next().is_none() {
            fs::remove_dir(entry.path())?;
        }
    }
    Ok(())
}

fn versioned_path(path: &str, kind: &str) -> Result<PathBuf> {
    let extension = if kind == "data" { ".json" } else { ".png" };
    let (relative, version) = path
        .split_once("?v=")
        .context("Snapshot URL requires a cache version")?;
    ensure!(
        relative.starts_with(&format!("_snapshot/{kind}/"))
            && relative.to_ascii_lowercase().ends_with(extension)
            && version.len() == 20
            && version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
        "Invalid versioned snapshot URL"
    );
    let decoded = relative
        .split('/')
        .map(|part| {
            let decoded = percent_decode_str(part)
                .decode_utf8()
                .context("Snapshot path is not UTF-8")?;
            ensure!(
                valid_segment(&decoded),
                "Snapshot URL escapes its artifact directory"
            );
            Ok(decoded.into_owned())
        })
        .collect::<Result<Vec<_>>>()?
        .join("/");
    ensure!(
        encoded_path(&decoded)? == relative,
        "Snapshot URL is not canonically encoded"
    );
    let path = PathBuf::from(decoded);
    normal_relative(&path)?;
    Ok(path)
}

fn read_versioned(root: &Path, path: &str, kind: &str) -> Result<Vec<u8>> {
    let relative = versioned_path(path, kind)?;
    let source = checked_file(root, &relative, false)?.context("Snapshot artifact missing")?;
    let bytes = read_bounded(
        &source,
        if kind == "data" {
            MAX_REPORT_BYTES
        } else {
            MAX_IMAGE_BYTES
        },
    )?;
    ensure!(
        path.split_once("?v=").map(|(_, version)| version)
            == Some(get_asset_version(&bytes, 20).as_str()),
        "Snapshot content does not match its cache version; regenerate the snapshot"
    );
    Ok(bytes)
}

fn read_manifest(root: &Path) -> Result<(String, Manifest)> {
    let source = checked_file(root, Path::new("index.html"), false)?
        .context("Run snapshot before deploying reports")?;
    let html = String::from_utf8(read_bounded(&source, MAX_REPORT_BYTES)?)?;
    let marker = "name=\"regressify-manifest\" content=\"";
    ensure!(
        html.matches(marker).count() == 1,
        "Snapshot entry page has no unique manifest; regenerate the snapshot"
    );
    let path = html
        .split_once(marker)
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(path, _)| path)
        .context("Snapshot manifest URL is missing")?;
    let manifest: Manifest = serde_json::from_slice(&read_versioned(root, path, "data")?)?;
    ensure!(
        manifest.version == 2,
        "Unsupported snapshot manifest version"
    );
    Ok((path.to_owned(), manifest))
}

/// Inventory only the active manifest, never stale assets, captures, or workspace state.
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub(crate) fn artifact_paths(output: &Path, selected: &[String]) -> Result<Vec<PathBuf>> {
    let root = output.canonicalize()?;
    let (manifest_path, manifest) = read_manifest(&root)?;
    let mut expected = BTreeSet::new();
    for id in selected {
        ensure!(
            valid_segment(id) && expected.insert(id.as_str()),
            "Invalid or duplicate snapshot suite selection"
        );
    }
    ensure!(
        manifest.suites.len() == expected.len()
            && manifest
                .suites
                .iter()
                .all(|suite| expected.contains(suite.id.as_str())),
        "Regenerate the snapshot for the selected suites before deploying"
    );
    let mut seen = BTreeSet::new();
    let mut paths = BTreeSet::from([
        PathBuf::from("index.html"),
        versioned_path(&manifest_path, "data")?,
    ]);
    let mut image_versions = BTreeMap::<PathBuf, String>::new();
    for (name, _) in FRONTEND_FILES {
        if *name != "gitignore" {
            paths.insert(PathBuf::from(name));
        }
    }
    for summary in manifest.suites {
        ensure!(
            seen.insert(summary.id.clone())
                && summary.passed.checked_add(summary.failed) == Some(summary.total)
                && summary.errors <= summary.failed,
            "Invalid snapshot suite summary"
        );
        let report: SuiteReport =
            serde_json::from_slice(&read_versioned(&root, &summary.report, "data")?)?;
        ensure!(
            report.id == summary.id && report.tests.len() == summary.total,
            "Snapshot suite data does not match its summary"
        );
        paths.insert(versioned_path(&summary.report, "data")?);
        let mut captures = BTreeSet::new();
        for capture in report.tests {
            ensure!(
                valid_segment(&capture.id)
                    && captures.insert(capture.id)
                    && matches!(capture.status.as_str(), "pass" | "fail"),
                "Invalid snapshot capture"
            );
            for image in [capture.reference, capture.test, capture.diff]
                .into_iter()
                .flatten()
            {
                let relative = versioned_path(&image, "images")?;
                if let Some(previous) = image_versions.insert(relative.clone(), image.clone()) {
                    ensure!(
                        previous == image,
                        "Conflicting cache versions for a snapshot image"
                    );
                }
                if paths.insert(relative) {
                    read_versioned(&root, &image, "images")?;
                }
            }
        }
    }
    for path in &paths {
        checked_file(&root, path, false)?;
    }
    Ok(paths.into_iter().collect())
}

#[cfg(feature = "desktop")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ImageRole {
    Passed,
    Comparison,
    Protected,
}

#[cfg(feature = "desktop")]
pub(crate) fn image_roles(output: &Path) -> Result<Option<BTreeMap<PathBuf, ImageRole>>> {
    let root = output.canonicalize()?;
    let source = checked_file(&root, Path::new("index.html"), false)?
        .context("Snapshot entry page missing")?;
    let html = String::from_utf8(read_bounded(&source, MAX_REPORT_BYTES)?)?;
    if !html.contains("name=\"regressify-manifest\"") {
        return Ok(None);
    }
    let (_, manifest) = read_manifest(&root)?;
    let mut roles = BTreeMap::<PathBuf, ImageRole>::new();
    for summary in manifest.suites {
        let report: SuiteReport =
            serde_json::from_slice(&read_versioned(&root, &summary.report, "data")?)?;
        for capture in report.tests {
            for (image, diff) in [
                (capture.reference, false),
                (capture.test, false),
                (capture.diff, true),
            ] {
                if let Some(image) = image {
                    let role = if diff || capture.error.is_some() {
                        ImageRole::Protected
                    } else if capture.status == "pass" {
                        ImageRole::Passed
                    } else {
                        ImageRole::Comparison
                    };
                    roles
                        .entry(versioned_path(&image, "images")?)
                        .and_modify(|previous| *previous = (*previous).max(role))
                        .or_insert(role);
                }
            }
        }
    }
    Ok(Some(roles))
}

/// Deployment compression changes bytes, so refresh every affected URL's cache version.
#[cfg(feature = "desktop")]
pub(crate) fn refresh_site_versions(output: &Path) -> Result<()> {
    let root = output.canonicalize()?;
    let (_, mut manifest) = read_manifest(&root)?;
    let mut replacements = BTreeMap::<PathBuf, String>::new();
    for summary in &mut manifest.suites {
        let mut report: SuiteReport =
            serde_json::from_slice(&read_versioned(&root, &summary.report, "data")?)?;
        for capture in &mut report.tests {
            for image in [&mut capture.reference, &mut capture.test, &mut capture.diff]
                .into_iter()
                .flatten()
            {
                let relative = versioned_path(image, "images")?;
                let replacement = if let Some(replacement) = replacements.get(&relative) {
                    replacement.clone()
                } else {
                    let source = checked_file(&root, &relative, false)?
                        .context("Compressed snapshot image is missing")?;
                    let path = image
                        .split_once("?v=")
                        .context("Snapshot image version missing")?
                        .0;
                    let replacement = format!(
                        "{path}?v={}",
                        get_asset_version(&read_bounded(&source, MAX_IMAGE_BYTES)?, 20)
                    );
                    replacements.insert(relative, replacement.clone());
                    replacement
                };
                *image = replacement;
            }
        }
        summary.report = store_json(
            &root,
            &format!("_snapshot/data/suites/{}.json", summary.id),
            &report,
        )?;
    }
    publish_index(&root, &manifest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> SnapshotArgs {
        SnapshotArgs {
            test_suite: Vec::new(),
            title: None,
            heading: None,
            subheading: None,
        }
    }

    #[test]
    fn frontend_resource_versions_match_the_rust_asset_helper() {
        let html = include_str!("data/index.html");
        let css = FRONTEND_FILES
            .iter()
            .filter(|(name, _)| name.ends_with(".css"))
            .map(|(_, bytes)| std::str::from_utf8(bytes).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        for (name, bytes) in FRONTEND_FILES {
            if name.ends_with(".js") || name.ends_with(".css") {
                assert!(
                    html.contains(&format!("/{name}?v={}", get_asset_version(bytes, 20))),
                    "{name}"
                );
            } else if name.ends_with(".woff2") {
                assert!(
                    css.contains(&format!("/{name}?v={}", get_asset_version(bytes, 20))),
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn versioned_urls_encode_unicode_and_reserved_characters_and_reject_escapes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = "_snapshot/images/caf\u{e9} & shop/reference/home #1 %2F.png";
        let url = store_asset(&root, path, b"image").unwrap();
        assert!(url.contains("caf%C3%A9%20%26%20shop/reference/home%20%231%20%252F.png?v="));
        assert_eq!(versioned_path(&url, "images").unwrap(), PathBuf::from(path));
        assert_eq!(read_versioned(&root, &url, "images").unwrap(), b"image");
        for unsafe_path in [
            "_snapshot/images/../private.png",
            "_snapshot/images/%2E%2E/private.png",
            "_snapshot/images/suite/%2Fprivate.png",
            "_snapshot/images/suite/%5Cprivate.png",
            "_snapshot/images/suite/%00.png",
            "_snapshot/images/suite/CON.png",
            "_snapshot/images/suite/file.png/",
            "https://example.com/_snapshot/images/home.png",
        ] {
            assert!(
                versioned_path(&format!("{unsafe_path}?v={}", "A".repeat(20)), "images").is_err(),
                "{unsafe_path}"
            );
        }
        for invalid in [
            path.to_owned(),
            format!("{url}&extra=1"),
            format!("{url}#fragment"),
            format!("{url}?v=again"),
        ] {
            assert!(versioned_path(&invalid, "images").is_err());
        }
    }

    #[test]
    fn snapshot_text_precedence_empty_values_and_hostile_text_are_preserved_safely() {
        let temp = tempfile::tempdir().unwrap();
        let mut workspace = WorkspaceConfig::default();
        workspace.snapshot_title = Some("Shop <report> & \"results\"".into());
        workspace.snapshot_heading = Some("Shop & results".into());
        workspace.snapshot_subheading = Some("**Staging** <script>alert(1)</script>".into());
        generate_site(temp.path(), &[], &args(), &workspace).unwrap();
        let (_, manifest) = read_manifest(&temp.path().canonicalize().unwrap()).unwrap();
        assert_eq!(manifest.heading, "Shop & results");
        assert_eq!(manifest.subheading, "**Staging** <script>alert(1)</script>");
        let html = fs::read_to_string(temp.path().join("index.html")).unwrap();
        assert!(html.contains("Shop &lt;report&gt; &amp; &quot;results&quot;"));
        assert!(!html.contains("<script>alert(1)"));
        let overrides = SnapshotArgs {
            title: Some(String::new()),
            heading: Some("Visual Tests Summary".into()),
            subheading: Some(String::new()),
            ..args()
        };
        generate_site(temp.path(), &[], &overrides, &workspace).unwrap();
        let (_, manifest) = read_manifest(&temp.path().canonicalize().unwrap()).unwrap();
        assert_eq!(manifest.title, "");
        assert_eq!(manifest.heading, "Visual Tests Summary");
        assert_eq!(manifest.subheading, "");
        generate_site(temp.path(), &[], &args(), &WorkspaceConfig::default()).unwrap();
        let (_, manifest) = read_manifest(&temp.path().canonicalize().unwrap()).unwrap();
        assert_eq!(manifest.title, "Visual Tests Summary | Regressify");
        assert_eq!(manifest.heading, "Visual Tests Summary");
    }

    #[test]
    fn snapshots_have_only_versioned_dependencies_and_matching_host_policies() {
        let temp = tempfile::tempdir().unwrap();
        generate_site(temp.path(), &[], &args(), &WorkspaceConfig::default()).unwrap();
        let paths = artifact_paths(temp.path(), &[]).unwrap();
        assert!(paths.iter().any(|path| {
            path.extension()
                .is_some_and(|extension| extension == "woff2")
        }));
        assert!(
            paths
                .iter()
                .all(|path| path.file_name().is_none_or(|name| name != "config.js"))
        );
        let config: serde_json::Value = serde_json::from_slice(
            &fs::read(temp.path().join("staticwebapp.config.json")).unwrap(),
        )
        .unwrap();
        let headers = fs::read_to_string(temp.path().join("_headers")).unwrap();
        for (name, value) in config["globalHeaders"].as_object().unwrap() {
            assert!(headers.contains(&format!("{name}: {}", value.as_str().unwrap())));
        }
        assert_eq!(config["navigationFallback"]["rewrite"], "/index.html");
        assert!(headers.contains("public, max-age=31536000, immutable"));
        assert!(
            fs::read_to_string(temp.path().join("_redirects"))
                .unwrap()
                .contains("/suites/* /index.html 200")
        );
    }

    #[test]
    fn asset_paths_stay_stable_while_versions_follow_bytes_and_reject_escapes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = "_snapshot/images/suite/test/home.png";
        let first = store_asset(&root, path, b"image").unwrap();
        assert_eq!(first, store_asset(&root, path, b"image").unwrap());
        let changed = store_asset(&root, path, b"changed").unwrap();
        assert_ne!(first, changed);
        assert_eq!(
            versioned_path(&first, "images").unwrap(),
            versioned_path(&changed, "images").unwrap()
        );
        assert_eq!(
            changed,
            format!("{path}?v={}", get_asset_version(b"changed", 20))
        );
        assert!(read_versioned(&root, &first, "images").is_err());
        assert_eq!(
            read_versioned(&root, &changed, "images").unwrap(),
            b"changed"
        );
        for path in [
            "../escape.png",
            "/escape.png",
            "images/../../escape.png",
            "C:\\escape.png",
            "images/CON.png",
        ] {
            assert!(output_file(&root, path, b"bad").is_err(), "{path}");
        }
        assert!(versioned_path("_snapshot/images/../../state.json", "images").is_err());
        fs::write(root.join(path), b"corrupted").unwrap();
        assert!(read_versioned(&root, &first, "images").is_err());
    }

    #[test]
    fn write_failure_preserves_previous_index_and_optional_read_errors_are_not_absence() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("index.html"), "old report").unwrap();
        fs::write(root.join("_snapshot"), "blocking file").unwrap();
        assert!(generate_site(&root, &[], &args(), &WorkspaceConfig::default()).is_err());
        assert_eq!(
            fs::read_to_string(root.join("index.html")).unwrap(),
            "old report"
        );
        fs::create_dir(root.join("not-a-file")).unwrap();
        assert!(checked_file(&root, Path::new("not-a-file"), true).is_err());
        assert!(read_bounded(&root.join("index.html"), 2).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_artifacts_reject_symlinked_parents_and_files() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("_snapshot")).unwrap();
        assert!(store_asset(&root, "_snapshot/images/suite/test/home.png", b"image").is_err());
        fs::write(outside.path().join("image.png"), "private").unwrap();
        std::os::unix::fs::symlink(outside.path().join("image.png"), root.join("linked.png"))
            .unwrap();
        assert!(checked_file(&root, Path::new("linked.png"), false).is_err());
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn compressed_images_refresh_all_versions_without_renaming_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let first = store_asset(
            &root,
            "_snapshot/images/one/reference/home.png",
            b"first image",
        )
        .unwrap();
        let second =
            store_asset(&root, "_snapshot/images/one/test/home.png", b"second image").unwrap();
        let capture: Capture = serde_json::from_value(serde_json::json!({
            "id": "home.png", "label": "Home", "scenario": "Home", "selector": "document", "status": "fail", "threshold": 0.1,
            "mismatch": 1.0, "error": null, "reference": first, "test": second, "diff": null, "referenceUrl": null, "testUrl": null, "dimensions": null
        })).unwrap();
        let report = SuiteReport {
            id: "one".into(),
            run_id: "run".into(),
            tests: vec![capture],
        };
        let manifest = Manifest {
            version: 2,
            title: "Review".into(),
            heading: "Review".into(),
            subheading: String::new(),
            generated_at: "2026-09-23T00:00:00Z".into(),
            suites: vec![SuiteSummary {
                id: "one".into(),
                total: 1,
                passed: 0,
                failed: 1,
                errors: 0,
                report: store_json(&root, "_snapshot/data/suites/one.json", &report).unwrap(),
            }],
        };
        for (name, bytes) in FRONTEND_FILES {
            if *name != "gitignore" {
                output_file(&root, name, bytes).unwrap();
            }
        }
        publish_index(&root, &manifest).unwrap();
        let old_index = fs::read(root.join("index.html")).unwrap();
        let first_path = versioned_path(&first, "images").unwrap();
        let second_path = versioned_path(&second, "images").unwrap();
        fs::write(root.join(&first_path), b"second image").unwrap();
        fs::write(root.join(&second_path), b"third image").unwrap();
        refresh_site_versions(&root).unwrap();
        artifact_paths(&root, &["one".into()]).unwrap();
        assert_ne!(fs::read(root.join("index.html")).unwrap(), old_index);
        let (_, updated) = read_manifest(&root).unwrap();
        let report: SuiteReport = serde_json::from_slice(
            &read_versioned(&root, &updated.suites[0].report, "data").unwrap(),
        )
        .unwrap();
        assert_eq!(
            read_versioned(&root, report.tests[0].reference.as_ref().unwrap(), "images").unwrap(),
            b"second image"
        );
        assert_eq!(
            read_versioned(&root, report.tests[0].test.as_ref().unwrap(), "images").unwrap(),
            b"third image"
        );
        assert_eq!(
            versioned_path(report.tests[0].reference.as_ref().unwrap(), "images").unwrap(),
            first_path
        );
        assert_eq!(
            versioned_path(report.tests[0].test.as_ref().unwrap(), "images").unwrap(),
            second_path
        );
        let index = fs::read(root.join("index.html")).unwrap();
        refresh_site_versions(&root).unwrap();
        assert_eq!(fs::read(root.join("index.html")).unwrap(), index);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn versioned_image_roles_protect_errors_diffs_and_shared_captures() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let shared = store_asset(&root, "_snapshot/images/one/test/shared.png", b"shared").unwrap();
        let passed = store_asset(
            &root,
            "_snapshot/images/one/reference/passed.png",
            b"passed",
        )
        .unwrap();
        let diff = store_asset(&root, "_snapshot/images/one/diff/shared.png", b"diff").unwrap();
        let tests = [("pass", None, passed.clone()), ("fail", Some("Timeout"), shared.clone())].into_iter().enumerate().map(|(index, (status, error, reference))| serde_json::from_value(serde_json::json!({
            "id": format!("capture-{index}"), "label": "", "scenario": "", "selector": "", "status": status, "threshold": 0.1, "mismatch": null,
            "error": error, "reference": reference, "test": shared, "diff": diff, "referenceUrl": null, "testUrl": null, "dimensions": null
        })).unwrap()).collect();
        let report = SuiteReport {
            id: "one".into(),
            run_id: "run".into(),
            tests,
        };
        let manifest = Manifest {
            version: 2,
            title: "Review".into(),
            heading: "Review".into(),
            subheading: String::new(),
            generated_at: "2026-09-23T00:00:00Z".into(),
            suites: vec![SuiteSummary {
                id: "one".into(),
                total: 2,
                passed: 1,
                failed: 1,
                errors: 1,
                report: store_json(&root, "_snapshot/data/suites/one.json", &report).unwrap(),
            }],
        };
        publish_index(&root, &manifest).unwrap();
        let roles = image_roles(&root).unwrap().unwrap();
        assert_eq!(
            roles[&versioned_path(&shared, "images").unwrap()],
            ImageRole::Protected
        );
        assert_eq!(
            roles[&versioned_path(&diff, "images").unwrap()],
            ImageRole::Protected
        );
        assert_eq!(
            roles[&versioned_path(&passed, "images").unwrap()],
            ImageRole::Passed
        );
    }
}
