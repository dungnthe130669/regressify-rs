use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, create_dir_all};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, anyhow};
use chromiumoxide::{
    Page,
    cdp::browser_protocol::page::{CaptureScreenshotFormat, Viewport},
    page::ScreenshotParams,
};
use serde::{Deserialize, Serialize};

use crate::{
    common,
    helpers::{
        OnceLockExtensions, is_windows_reserved_path_component,
        png::{MAX_VISUAL_IMAGE_DIMENSION, MAX_VISUAL_IMAGE_PIXELS},
        slugify, write_file_atomically,
    },
    snapshot::get_asset_version,
    types::{ScreenshotType, Selectors, TestAction, TestScenario, TestSuite},
};

/// Chunks retained by normalization. Color-management chunks affect rendered
/// appearance and must not be stripped from visual-regression artifacts.
const PRESERVED_PNG_CHUNKS: [&[u8; 4]; 9] = [
    b"IHDR", b"PLTE", b"tRNS", b"gAMA", b"cHRM", b"sRGB", b"iCCP", b"IDAT", b"IEND",
];
const SELECTOR_SCROLL_SETTLE: Duration = Duration::from_millis(100);
const MAX_SELECTOR_SLUG_LEN: usize = 48;
const MAX_STEP_NAME_BYTES: usize = 100;
const MAX_STATE_NAME_BYTES: usize = 80;

/// Marker for failures while producing on-disk regression artifacts. These are
/// tool failures, unlike a page that fails to render or perform an action.
#[derive(Debug)]
pub struct ArtifactWriteError {
    message: String,
}

impl std::fmt::Display for ArtifactWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ArtifactWriteError {}

pub(crate) fn artifact_write_error(
    operation: &str,
    error: impl std::fmt::Display,
) -> anyhow::Error {
    ArtifactWriteError {
        message: format!("{}: {}", operation, error),
    }
    .into()
}

pub fn is_artifact_write_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ArtifactWriteError>().is_some()
}

/// Strip non-essential metadata chunks from a PNG byte buffer so that
/// identical pixel content always produces byte-identical files.
fn strip_png_metadata(png: &[u8]) -> Result<Vec<u8>> {
    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

    if png.len() < 8 || png[..8] != PNG_SIGNATURE {
        return Err(anyhow!("Invalid PNG signature"));
    }

    let mut out = Vec::with_capacity(png.len());
    out.extend_from_slice(&PNG_SIGNATURE);

    let mut pos = 8;
    let mut found_iend = false;
    while pos < png.len() {
        if png.len() - pos < 12 {
            return Err(anyhow!("Truncated PNG chunk"));
        }
        let data_len =
            u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]]) as usize;
        let chunk_type = &png[pos + 4..pos + 8];
        // total chunk size = 4 (length) + 4 (type) + data_len + 4 (CRC)
        let chunk_total = data_len
            .checked_add(12)
            .ok_or_else(|| anyhow!("PNG chunk length overflows platform size"))?;
        let chunk_end = pos
            .checked_add(chunk_total)
            .ok_or_else(|| anyhow!("PNG chunk offset overflows platform size"))?;

        if chunk_end > png.len() {
            return Err(anyhow!("Truncated PNG chunk"));
        }

        if PRESERVED_PNG_CHUNKS
            .iter()
            .any(|t| t.as_slice() == chunk_type)
        {
            out.extend_from_slice(&png[pos..chunk_end]);
        }

        pos = chunk_end;
        if chunk_type == b"IEND" {
            if data_len != 0 || pos != png.len() {
                return Err(anyhow!("Invalid PNG IEND chunk"));
            }
            found_iend = true;
        }
    }

    if !found_iend {
        return Err(anyhow!("PNG is missing its IEND chunk"));
    }
    Ok(out)
}

/// Resolve final capture targets for one scenario. Without `selectors`, a
/// scenario continues to produce one full-page screenshot.
pub fn final_screenshot_types(scenario: &TestScenario) -> Result<Vec<ScreenshotType>> {
    let Some(selectors) = scenario.selectors.as_ref() else {
        return Ok(vec![ScreenshotType::Final]);
    };

    let selectors: Vec<&str> = match selectors {
        Selectors::String(selector) => vec![selector],
        Selectors::StringArray(selectors) => selectors.iter().map(String::as_str).collect(),
    };
    if selectors.is_empty() {
        anyhow::bail!("Scenario selectors must contain at least one capture target");
    }

    let mut captures = Vec::with_capacity(selectors.len());
    let mut names = HashSet::with_capacity(selectors.len());
    for selector in selectors {
        let selector = selector.trim();
        if selector.is_empty() {
            anyhow::bail!("Scenario selectors must not contain an empty value");
        }
        let capture = if selector == "viewport" {
            ScreenshotType::Viewport
        } else {
            ScreenshotType::Selector(selector.to_string())
        };
        if !names.insert(screenshot_type_name(&capture)) {
            anyhow::bail!(
                "Scenario selectors contain duplicate capture target '{}'",
                selector
            );
        }
        captures.push(capture);
    }
    Ok(captures)
}

/// Validate portable names and unconditional collisions before viewport resolution.
/// Suite preflight checks conditional collisions against each resolved capture inventory.
pub fn validate_scenario_capture_names(scenario: &TestScenario) -> Result<()> {
    let actions = scenario.actions.as_deref().unwrap_or_default();
    for action in actions {
        if let TestAction::Save(save) = action {
            validate_screenshot_step_name(&save.save)?;
        }
    }
    let mut screenshot_types = saved_screenshot_types(
        actions
            .iter()
            .filter(|action| super::test_scenario::action_applies_to_viewport(action, None)),
    );
    screenshot_types.extend(final_screenshot_types(scenario)?);
    validate_screenshot_types(&screenshot_types)
}

pub(crate) fn saved_screenshot_types<'a>(
    actions: impl IntoIterator<Item = &'a TestAction>,
) -> Vec<ScreenshotType> {
    actions
        .into_iter()
        .filter_map(|action| match action {
            TestAction::Save(save) => Some(ScreenshotType::Step(save.save.clone())),
            _ => None,
        })
        .collect()
}

pub(crate) fn validate_screenshot_types(screenshot_types: &[ScreenshotType]) -> Result<()> {
    let mut names = HashSet::with_capacity(screenshot_types.len());
    for screenshot_type in screenshot_types {
        register_capture_name(&mut names, screenshot_type)?;
    }
    Ok(())
}

/// Validate a name used by the `save` action before it becomes part of a PNG
/// filename. The `_diff` suffix is reserved for comparison artifacts.
pub fn validate_screenshot_step_name(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || name != name.trim()
        || name.len() > MAX_STEP_NAME_BYTES
        || name.ends_with(['.', ' '])
        || name.ends_with("_diff")
        || name.contains(['/', '\\', '\0', '<', '>', ':', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
    {
        anyhow::bail!(
            "Invalid save name '{}': use a non-empty portable filename component no longer than {} bytes and do not end it with '_diff'",
            name,
            MAX_STEP_NAME_BYTES
        );
    }
    Ok(())
}

fn register_capture_name(
    names: &mut HashSet<String>,
    screenshot_type: &ScreenshotType,
) -> Result<()> {
    if let ScreenshotType::Step(name) = screenshot_type {
        validate_screenshot_step_name(name)?;
    }
    let name = screenshot_type_name(screenshot_type);
    if !names.insert(name.to_lowercase()) {
        anyhow::bail!(
            "Scenario capture targets produce duplicate filename '{}'.",
            name
        );
    }
    Ok(())
}

/// Return a manifest key matching a screenshot filename without its `.png`
/// suffix. The key is stable across runs and selector declaration order.
pub fn final_screenshot_order_key(
    scenario_id: &str,
    viewport_label: Option<&str>,
    screenshot_type: &ScreenshotType,
) -> String {
    format!(
        "{}{}__{}",
        scenario_id,
        viewport_label
            .map(|label| format!("_{}", slugify(label)))
            .unwrap_or_default(),
        screenshot_type_name(screenshot_type)
    )
}

/// Describe a target succinctly in capture logs and error messages.
pub fn screenshot_type_description(screenshot_type: &ScreenshotType) -> String {
    match screenshot_type {
        ScreenshotType::Step(name) => format!("step '{}'", name),
        ScreenshotType::Final => "full page".to_string(),
        ScreenshotType::Viewport => "viewport".to_string(),
        ScreenshotType::Selector(selector) => format!("selector '{}'", selector),
    }
}

fn screenshot_type_name(screenshot_type: &ScreenshotType) -> String {
    match screenshot_type {
        ScreenshotType::Step(step) => format!("${step}"),
        ScreenshotType::Final => "FINAL".to_string(),
        ScreenshotType::Viewport => "VIEWPORT".to_string(),
        ScreenshotType::Selector(selector) => selector_capture_name(selector),
    }
}

fn selector_capture_name(selector: &str) -> String {
    let slug = slugify(selector);
    let readable = if slug.is_empty() {
        "target"
    } else {
        &slug[..slug.len().min(MAX_SELECTOR_SLUG_LEN)]
    };
    let hash = get_asset_version(selector.as_bytes(), 8);
    format!("SELECTOR-{readable}-{hash}")
}

/// Build the on-disk path for a scenario screenshot, creating the run directory.
/// Shared by real captures and the error-placeholder writer so both land on the
/// exact same filename (`<scenario-id>[_<viewport>]__<target>.png`).
fn screenshot_path(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    screenshot_type: &ScreenshotType,
    viewport_label: Option<&str>,
) -> Result<PathBuf> {
    let output_dir = common::REGRESSIFY_DIR.get_required()?;
    screenshot_path_in(
        output_dir,
        test_suite,
        scenario,
        screenshot_type,
        viewport_label,
    )
}

fn screenshot_path_in(
    output_dir: &Path,
    test_suite: &TestSuite,
    scenario: &TestScenario,
    screenshot_type: &ScreenshotType,
    viewport_label: Option<&str>,
) -> Result<PathBuf> {
    if let ScreenshotType::Step(name) = screenshot_type {
        validate_screenshot_step_name(name)?;
    }
    let test_suite_id = test_suite
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test suite ID is required for Save action"))?;

    let scenario_id = scenario
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test scenario ID is required for Save action"))?;

    let test_date = test_suite
        .test_date
        .as_deref()
        .ok_or_else(|| anyhow!("Test suite date is required for Save action"))?;

    let test_suite_dir = output_dir
        .join(test_suite_id)
        .join("bitmaps_test")
        .join(test_date);

    create_dir_all(&test_suite_dir)?;

    // When capturing at a named viewport, include it in the filename so multiple
    // viewports for the same scenario produce distinct (non-colliding) files.
    let vp_suffix = viewport_label
        .map(|l| format!("_{}", slugify(l)))
        .unwrap_or_default();

    Ok(test_suite_dir.join(format!(
        "{}{}__{}.png",
        scenario_id,
        vp_suffix,
        screenshot_type_name(screenshot_type)
    )))
}

/// Filename of the report ordering manifest written alongside the captured
/// screenshots. It records the intended display order of screenshots (scenario
/// order, then viewport order) so the comparison report does not fall back to
/// alphabetical filename sorting.
pub const REPORT_ORDER_FILE: &str = ".report-order.json";
pub const REPORT_THRESHOLDS_FILE: &str = ".report-thresholds.json";
pub const REPORT_PIXEL_THRESHOLDS_FILE: &str = ".report-pixel-thresholds.json";
pub const REPORT_URLS_FILE: &str = ".report-urls.json";
pub(crate) const REPORT_ORDER_VERSION: u8 = 1;
pub(crate) const REPORT_URLS_VERSION: u8 = 1;

pub(crate) fn mark_diagnostics_expected(suite: &TestSuite) -> Result<()> {
    let path = run_dir(suite)?.join(".diagnostics.json");
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Diagnostic marker must be a regular file"
        );
    }
    write_file_atomically(&path, br#"{"version":1}"#)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLog {
    pub state: crate::diagnostics::journal::ArchiveState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<crate::diagnostics::runtime::ArchiveReference>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureLogLink {
    #[serde(flatten)]
    log: CaptureLog,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image_digest: Option<String>,
}

fn image_digest(path: &Path) -> Result<Option<String>> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        anyhow::ensure!(
            bytes <= 512 * 1024 * 1024,
            "Capture log image exceeds byte limit"
        );
        digest.update(&buffer[..count]);
    }
    Ok(Some(format!("{:x}", digest.finalize())))
}

fn store_capture_log(output: &Path, image: &Path, log: CaptureLog) -> Result<()> {
    let root = output.canonicalize()?;
    let parent = image
        .parent()
        .ok_or_else(|| anyhow!("Capture log has no parent"))?;
    anyhow::ensure!(
        !fs::symlink_metadata(parent)?.file_type().is_symlink()
            && parent.canonicalize()?.starts_with(&root),
        "Capture log escapes output root"
    );
    if let Ok(metadata) = fs::symlink_metadata(image) {
        anyhow::ensure!(
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && image.canonicalize()?.starts_with(&root),
            "Capture log image must be a contained regular file"
        );
    }
    let path = image.with_extension("log.json");
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Capture log must be a regular file"
        );
    }
    write_file_atomically(
        &path,
        &serde_json::to_vec(&CaptureLogLink {
            log,
            image_digest: image_digest(image)?,
        })?,
    )
}

pub(crate) fn promote_capture_log(output: &Path, source: &Path, destination: &Path) {
    let log = read_capture_log(output, source).unwrap_or(CaptureLog {
        state: crate::diagnostics::journal::ArchiveState::LegacyUnavailable,
        archive: None,
    });
    if store_capture_log(output, destination, log).is_err() {
        crate::diagnostics::install::emergency_warning(
            "Reference image was promoted but its log link could not be updated",
        );
    }
}

pub fn write_capture_log(
    output: &Path,
    image: &Path,
    archive: &crate::diagnostics::runtime::ArchiveReference,
) -> Result<()> {
    let log = CaptureLog {
        state: archive.state,
        archive: Some(archive.clone()),
    };
    store_capture_log(output, image, log)
}

pub fn read_capture_log(output: &Path, image: &Path) -> Option<CaptureLog> {
    use crate::diagnostics::journal::{ArchiveState, inspect_archive};
    let path = image.with_extension("log.json");
    if matches!(fs::symlink_metadata(&path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        let marker = image.parent()?.join(".diagnostics.json");
        if matches!(fs::symlink_metadata(&marker), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        {
            return None;
        }
        let valid = output
            .canonicalize()
            .ok()
            .and_then(|root| {
                crate::diagnostics::journal::read_json::<serde_json::Value>(&marker, &root, 128)
                    .ok()
            })
            .is_some_and(|value| value["version"] == 1);
        return Some(CaptureLog {
            state: if valid {
                ArchiveState::Missing
            } else {
                ArchiveState::LoadingError
            },
            archive: None,
        });
    }
    let loaded = (|| -> Result<CaptureLog> {
        let root = output.canonicalize()?;
        let link: CaptureLogLink = crate::diagnostics::journal::read_json(&path, &root, 64 * 1024)?;
        if let Some(expected) = link.image_digest {
            crate::diagnostics::journal::checked_file(image, &root)?;
            anyhow::ensure!(
                image_digest(image)?.as_ref() == Some(&expected),
                "Capture log no longer matches its image"
            );
        }
        let mut log = link.log;
        if let Some(reference) = &log.archive {
            log.state = inspect_archive(&root, Some(&reference.operation_id), true);
        }
        Ok(log)
    })();
    Some(loaded.unwrap_or(CaptureLog {
        state: ArchiveState::LoadingError,
        archive: None,
    }))
}

pub(crate) fn write_execution_logs(
    suite: &TestSuite,
    scenario: &TestScenario,
    viewport: Option<&str>,
    archive: &crate::diagnostics::runtime::ArchiveReference,
) -> Result<()> {
    let directory = run_dir(suite)?;
    for key in super::test_scenario::capture_progress_keys(scenario, &suite.scenarios, viewport)? {
        let image = directory.join(format!("{key}.png"));
        write_capture_log(common::REGRESSIFY_DIR.get_required()?, &image, archive)?;
    }
    Ok(())
}

/// Complete expected screenshot inventory for a capture run. Older runs stored
/// a bare array and remain readable as order-only manifests.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReportOrderManifest {
    pub version: u8,
    pub captures: Vec<String>,
}

/// Page URLs associated with one captured target. `url` is the test page URL
/// expected by the bundled report UI; `reference_url` is the raw baseline URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReportPageUrls {
    pub reference_url: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_display_name: Option<String>,
}

/// Per-capture page URLs preserved for report regeneration and standalone
/// comparisons, which otherwise only have screenshot filenames available.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReportUrlsManifest {
    pub version: u8,
    pub captures: BTreeMap<String, ReportPageUrls>,
}

fn run_dir(test_suite: &TestSuite) -> Result<PathBuf> {
    let test_suite_id = test_suite
        .id
        .as_ref()
        .ok_or_else(|| anyhow!("Test suite ID is required"))?;
    let test_date = test_suite
        .test_date
        .as_deref()
        .ok_or_else(|| anyhow!("Test suite date is required"))?;
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;
    Ok(regressify_dir
        .join(test_suite_id)
        .join("bitmaps_test")
        .join(test_date))
}

pub fn validate_state_name(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || name != name.trim()
        || name.len() > MAX_STATE_NAME_BYTES
        || name.ends_with(['.', ' '])
        || name.contains(['/', '\\', '\0', '<', '>', ':', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
        || is_windows_reserved_path_component(name)
    {
        anyhow::bail!(
            "Invalid state name '{}': use a non-empty portable filename component no longer than {} bytes",
            name,
            MAX_STATE_NAME_BYTES
        );
    }
    Ok(())
}

pub fn state_path(name: &str) -> Result<PathBuf> {
    validate_state_name(name)?;
    let regressify_dir = common::REGRESSIFY_DIR.get_required()?;
    Ok(regressify_dir.join("states").join(format!("{}.json", name)))
}

/// Load a configured persisted state file and verify the data shape generated
/// by the `persist` action before it alters browser state.
pub fn load_persisted_state(name: &str) -> Result<(PathBuf, serde_json::Value)> {
    let path = persisted_state_source(
        common::ROOT_DIR.get_required()?,
        common::REGRESSIFY_DIR.get_required()?,
        name,
    )?;
    let state = read_persisted_state_file(&path, name)?;
    Ok((path, state))
}

pub(crate) fn persisted_state_source(root: &Path, output: &Path, name: &str) -> Result<PathBuf> {
    validate_state_name(name)?;
    let current = output.join("states").join(format!("{}.json", name));
    if current.exists() {
        return Ok(current);
    }
    Ok(crate::config::resolve_optional_workspace_file(
        root,
        &format!(".states/{}.json", name),
        "legacy persisted state",
    )?
    .unwrap_or(current))
}

pub(crate) fn read_persisted_state_file(path: &Path, name: &str) -> Result<serde_json::Value> {
    let content = fs::read_to_string(path).map_err(|error| {
        anyhow!(
            "Failed to read persisted state '{}': {}",
            path.display(),
            error
        )
    })?;
    let state: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| anyhow!("Invalid persisted state '{}': {}", path.display(), error))?;
    validate_persisted_state_value(&state, name)?;
    Ok(state)
}

fn validate_persisted_state_value(state: &serde_json::Value, name: &str) -> Result<()> {
    super::storage_state::validate_origins(state)?;
    let object = state
        .as_object()
        .ok_or_else(|| anyhow!("Persisted state '{}' must be a JSON object", name))?;
    if let Some(cookies) = object.get("cookies") {
        let cookies = cookies.as_array().ok_or_else(|| {
            anyhow!(
                "Persisted state '{}' field 'cookies' must be an array",
                name
            )
        })?;
        for (index, cookie) in cookies.iter().enumerate() {
            validate_cookie_value(
                cookie,
                &format!("Persisted state '{}' cookie {}", name, index),
            )?;
        }
    }
    if let Some(local_storage) = object.get("localStorage") {
        let local_storage = local_storage.as_object().ok_or_else(|| {
            anyhow!(
                "Persisted state '{}' field 'localStorage' must be an object",
                name
            )
        })?;
        for (key, value) in local_storage {
            if !value.is_string() {
                anyhow::bail!(
                    "Persisted state '{}' localStorage key '{}' must have a string value",
                    name,
                    key
                );
            }
        }
    }
    Ok(())
}

/// Validate cookie data before it is sent through CDP. Chromium's cookie JSON
/// carries additional fields, so unknown fields remain allowed, but every field
/// this CLI consumes must be well typed and meaningful.
pub(crate) fn expand_cookie_entries(
    cookies: &[serde_json::Value],
    context: &str,
) -> Result<Vec<serde_json::Value>> {
    let mut expanded = Vec::new();
    for (index, cookie) in cookies.iter().enumerate() {
        let label = format!("{} cookie {}", context, index);
        if let Some(domains) = cookie.get("domain").and_then(serde_json::Value::as_array) {
            if domains.is_empty() {
                anyhow::bail!("{} domain array must not be empty", label);
            }
            for domain in domains {
                let mut entry = cookie.clone();
                entry["domain"] = domain.clone();
                validate_cookie_value(&entry, &label)?;
                expanded.push(entry);
            }
        } else {
            validate_cookie_value(cookie, &label)?;
            expanded.push(cookie.clone());
        }
    }
    Ok(expanded)
}

pub(crate) fn validate_cookie_value(cookie: &serde_json::Value, context: &str) -> Result<()> {
    let cookie = cookie
        .as_object()
        .ok_or_else(|| anyhow!("{} must be an object", context))?;
    let name = cookie
        .get("name")
        .and_then(|value| value.as_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("{} requires a non-empty name", context))?;
    let context = format!("{} ({:?})", context, name);
    let value = cookie
        .get("value")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("{} requires a string value", context))?;
    const MAX_COOKIE_NAME_VALUE_BYTES: usize = 4096;
    let size = name.len().saturating_add(value.len());
    if size > MAX_COOKIE_NAME_VALUE_BYTES {
        anyhow::bail!(
            "{} name + value is {} bytes, exceeding Chromium's {}-byte cookie limit; re-export this cookie from the browser. Keep chunked authentication cookies as separate entries; do not concatenate or truncate their values",
            context,
            size,
            MAX_COOKIE_NAME_VALUE_BYTES
        );
    }
    if value.contains(['\r', '\n', '\0']) {
        anyhow::bail!(
            "{} field 'value' must be a single-line string without CR, LF, or NUL characters; use a quoted value or YAML '|-' to avoid a trailing newline. Cookie values are not modified automatically",
            context
        );
    }

    let has_url = cookie
        .get("url")
        .and_then(|value| value.as_str())
        .is_some_and(|url| !url.trim().is_empty());
    let has_domain = cookie
        .get("domain")
        .and_then(|value| value.as_str())
        .is_some_and(|domain| !domain.trim_start_matches('.').trim().is_empty());
    if !has_url && !has_domain {
        anyhow::bail!("{} requires a url or domain", context);
    }

    for (field, expected) in [
        ("url", "a string"),
        ("domain", "a string"),
        ("path", "a string"),
        ("sameSite", "a string"),
    ] {
        if cookie.get(field).is_some_and(|value| !value.is_string()) {
            anyhow::bail!("{} field '{}' must be {}", context, field, expected);
        }
    }
    for field in ["secure", "httpOnly"] {
        if cookie.get(field).is_some_and(|value| !value.is_boolean()) {
            anyhow::bail!("{} field '{}' must be a boolean", context, field);
        }
    }
    if let Some(expires) = cookie.get("expires")
        && !expires.as_f64().is_some_and(f64::is_finite)
    {
        anyhow::bail!("{} field 'expires' must be a finite number", context);
    }
    if let Some(same_site) = cookie.get("sameSite").and_then(|value| value.as_str())
        && !matches!(
            same_site.to_ascii_lowercase().as_str(),
            "strict" | "lax" | "none"
        )
    {
        anyhow::bail!("{} field 'sameSite' must be Strict, Lax, or None", context);
    }
    if cookie
        .get("sameSite")
        .and_then(|value| value.as_str())
        .is_some_and(|same_site| same_site.eq_ignore_ascii_case("none"))
        && cookie.get("secure").and_then(|value| value.as_bool()) != Some(true)
    {
        anyhow::bail!("{} with SameSite=None requires secure: true", context);
    }
    Ok(())
}

/// Write the report ordering manifest for a suite run. `prefixes` is the ordered
/// list of screenshot name prefixes (`<scenario-id>[_<viewport>]`, the part of
/// each filename before the `__<step|FINAL>` suffix) in the order the scenarios
/// and their viewports are defined. The comparison report uses this to list
/// screenshots in scenario-then-viewport order instead of alphabetically.
pub fn write_report_order(test_suite: &TestSuite, prefixes: &[String]) -> Result<()> {
    let run_dir = run_dir(test_suite)?;
    create_dir_all(&run_dir)?;

    let manifest = serde_json::to_vec_pretty(&ReportOrderManifest {
        version: REPORT_ORDER_VERSION,
        captures: prefixes.to_vec(),
    })?;
    write_file_atomically(&run_dir.join(REPORT_ORDER_FILE), &manifest)?;
    Ok(())
}

pub fn write_report_thresholds(test_suite: &TestSuite, thresholds: &[(String, f64)]) -> Result<()> {
    write_threshold_manifest(&run_dir(test_suite)?, REPORT_THRESHOLDS_FILE, thresholds)
}

pub(crate) fn write_report_pixel_thresholds(
    test_suite: &TestSuite,
    thresholds: &[(String, f64)],
) -> Result<()> {
    write_threshold_manifest(
        &run_dir(test_suite)?,
        REPORT_PIXEL_THRESHOLDS_FILE,
        thresholds,
    )
}

fn write_threshold_manifest(
    run_dir: &Path,
    file_name: &str,
    thresholds: &[(String, f64)],
) -> Result<()> {
    for (key, value) in thresholds {
        anyhow::ensure!(
            !key.trim().is_empty() && value.is_finite() && (0.0..=100.0).contains(value),
            "Invalid report threshold manifest '{}': '{}' must be a finite value between 0 and 100",
            file_name,
            key
        );
    }
    create_dir_all(run_dir)?;

    let map = thresholds.iter().cloned().collect::<HashMap<String, f64>>();
    let manifest = serde_json::to_vec_pretty(&map)?;
    write_file_atomically(&run_dir.join(file_name), &manifest)?;
    Ok(())
}

pub(crate) fn write_report_urls(
    test_suite: &TestSuite,
    urls: &[(String, ReportPageUrls)],
) -> Result<()> {
    let run_dir = run_dir(test_suite)?;
    create_dir_all(&run_dir)?;

    let mut captures = BTreeMap::new();
    for (capture, page_urls) in urls {
        if capture.trim().is_empty()
            || page_urls.reference_url.trim().is_empty()
            || page_urls.url.trim().is_empty()
        {
            anyhow::bail!("Report URL manifest contains an empty capture key or page URL");
        }
        if captures
            .insert(capture.clone(), page_urls.clone())
            .is_some()
        {
            anyhow::bail!(
                "Report URL manifest contains duplicate capture key '{}'",
                capture
            );
        }
    }

    let manifest = serde_json::to_vec_pretty(&ReportUrlsManifest {
        version: REPORT_URLS_VERSION,
        captures,
    })?;
    write_file_atomically(&run_dir.join(REPORT_URLS_FILE), &manifest)?;
    Ok(())
}

pub async fn save_screenshot(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    page: &Page,
    screenshot_type: ScreenshotType,
    viewport_label: Option<&str>,
) -> Result<()> {
    use crate::diagnostics::{record::*, runtime as diagnostics};
    let key = final_screenshot_order_key(
        scenario.id.as_deref().unwrap_or_default(),
        viewport_label,
        &screenshot_type,
    );
    diagnostics::stage(
        Stage::CaptureArtifacts,
        [(
            Field::Capture,
            FieldValue::Identity(SafeIdentity::fingerprint(key)),
        )],
        save_screenshot_inner(test_suite, scenario, page, screenshot_type, viewport_label),
    )
    .await
}

async fn save_screenshot_inner(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    page: &Page,
    screenshot_type: ScreenshotType,
    viewport_label: Option<&str>,
) -> Result<()> {
    use crate::diagnostics::{record::*, runtime as diagnostics};
    let path = screenshot_path(test_suite, scenario, &screenshot_type, viewport_label)
        .map_err(|error| artifact_write_error("Failed to create screenshot output path", error))?;

    let png_bytes = capture_png(page, &screenshot_type).await?;
    let clean_bytes = strip_png_metadata(&png_bytes)?;
    let (width, height) = crate::helpers::png::read_bounded_png_dimensions_from_reader(
        std::io::Cursor::new(&clean_bytes),
        &path,
    )?;
    diagnostics::stage_sync(
        Stage::CaptureArtifacts,
        [
            (Field::Width, FieldValue::Number(width as u64)),
            (Field::Height, FieldValue::Number(height as u64)),
            (Field::Bytes, FieldValue::Number(clean_bytes.len() as u64)),
        ],
        || {
            write_file_atomically(&path, &clean_bytes)
                .map_err(|error| artifact_write_error("Failed to write screenshot", error))
        },
    )?;
    let marker_path = error_marker_path(&path);
    if marker_path.exists() {
        fs::remove_file(marker_path).map_err(|error| {
            artifact_write_error("Failed to clear stale screenshot marker", error)
        })?;
    }

    Ok(())
}

async fn capture_png(page: &Page, screenshot_type: &ScreenshotType) -> Result<Vec<u8>> {
    if let ScreenshotType::Selector(selector) = screenshot_type {
        return capture_selector_png(page, selector).await;
    }

    let params = match screenshot_type {
        ScreenshotType::Step(_) | ScreenshotType::Final => full_page_params(page).await?,
        ScreenshotType::Viewport => ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .from_surface(true)
            .omit_background(false)
            .build(),
        ScreenshotType::Selector(_) => unreachable!("selector capture is handled above"),
    };

    page.screenshot(params).await.map_err(|error| {
        anyhow!(
            "Failed to capture {} screenshot: {}",
            screenshot_type_description(screenshot_type),
            error
        )
    })
}

struct SelectorCapture {
    clip: Viewport,
    restore_x: f64,
    restore_y: f64,
}

async fn capture_selector_png(page: &Page, selector: &str) -> Result<Vec<u8>> {
    let capture = prepare_selector_capture(page, selector).await?;
    let params = ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .clip(capture.clip)
        .capture_beyond_viewport(true)
        .from_surface(true)
        .omit_background(false)
        .build();
    let screenshot_result = page.screenshot(params).await.map_err(|error| {
        anyhow!(
            "Failed to capture {} screenshot: {}",
            screenshot_type_description(&ScreenshotType::Selector(selector.to_string())),
            error
        )
    });
    let restore_result = restore_page_scroll(page, capture.restore_x, capture.restore_y).await;

    match (screenshot_result, restore_result) {
        (Ok(png), Ok(())) => Ok(png),
        (Err(screenshot_error), Ok(())) => Err(screenshot_error),
        (Ok(_), Err(restore_error)) => Err(restore_error),
        (Err(screenshot_error), Err(restore_error)) => Err(anyhow!(
            "{}; additionally failed to restore page scroll: {}",
            screenshot_error,
            restore_error
        )),
    }
}

/// Build a full-page capture that avoids resizing the emulated viewport.
/// Chromium captures the document in one compositor pass, preventing a
/// timing-sensitive relayout immediately before the screenshot.
async fn full_page_params(page: &Page) -> Result<ScreenshotParams> {
    let metrics = page.layout_metrics().await?;
    let width = metrics.css_content_size.width;
    let height = metrics.css_content_size.height;
    validate_capture_dimensions(width, height, "full page")?;

    Ok(ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .clip(Viewport {
            x: 0.,
            y: 0.,
            width,
            height,
            scale: 1.,
        })
        .capture_beyond_viewport(true)
        .from_surface(true)
        .omit_background(false)
        .build())
}

fn validate_capture_dimensions(width: f64, height: f64, target: &str) -> Result<()> {
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > f64::from(MAX_VISUAL_IMAGE_DIMENSION)
        || height > f64::from(MAX_VISUAL_IMAGE_DIMENSION)
    {
        anyhow::bail!(
            "Cannot capture {}: dimensions {}x{} must be finite, positive, and no larger than {} CSS pixels",
            target,
            width,
            height,
            MAX_VISUAL_IMAGE_DIMENSION
        );
    }
    let pixel_count = width.ceil() * height.ceil();
    if pixel_count > MAX_VISUAL_IMAGE_PIXELS as f64 {
        anyhow::bail!(
            "Cannot capture {}: dimensions {}x{} exceed the {} CSS pixel limit",
            target,
            width,
            height,
            MAX_VISUAL_IMAGE_PIXELS
        );
    }
    Ok(())
}

async fn prepare_selector_capture(page: &Page, selector: &str) -> Result<SelectorCapture> {
    let script = selector_capture_script(selector)?;
    let result = tokio::time::timeout(Duration::from_secs(5), page.evaluate(script))
        .await
        .map_err(|_| anyhow!("Timed out preparing selector '{}' for screenshot", selector))??;
    let value: serde_json::Value = result.into_value().unwrap_or(serde_json::Value::Null);

    if value.get("ok").and_then(|field| field.as_bool()) != Some(true) {
        let error = value
            .get("error")
            .and_then(|field| field.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("Cannot capture selector '{}': {}", selector, error);
    }

    let coordinate = |name: &str| -> Result<f64> {
        let coordinate = value
            .get(name)
            .and_then(|field| field.as_f64())
            .ok_or_else(|| {
                anyhow!(
                    "Selector '{}' returned no valid {} coordinate",
                    selector,
                    name
                )
            })?;
        if !coordinate.is_finite() {
            anyhow::bail!(
                "Selector '{}' returned a non-finite {} coordinate",
                selector,
                name
            );
        }
        Ok(coordinate)
    };
    let width = coordinate("width")?;
    let height = coordinate("height")?;
    validate_capture_dimensions(width, height, &format!("selector '{}'", selector))?;

    Ok(SelectorCapture {
        clip: Viewport {
            x: coordinate("x")?,
            y: coordinate("y")?,
            width,
            height,
            scale: 1.0,
        },
        restore_x: coordinate("restoreX")?,
        restore_y: coordinate("restoreY")?,
    })
}

async fn restore_page_scroll(page: &Page, x: f64, y: f64) -> Result<()> {
    let script = format!(
        "(() => {{ window.scrollTo({}, {}); return true; }})()",
        serde_json::to_string(&x)?,
        serde_json::to_string(&y)?
    );
    tokio::time::timeout(Duration::from_secs(5), page.evaluate(script))
        .await
        .map_err(|_| anyhow!("Timed out restoring page scroll after selector capture"))??;
    Ok(())
}

fn selector_capture_script(selector: &str) -> Result<String> {
    let selector = serde_json::to_string(selector)?
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    Ok(format!(
        r#"(async () => {{
            const selector = {selector};
            const find = () => {{
                try {{ return {{ element: document.querySelector(selector) }}; }}
                catch (_) {{ return {{ error: 'invalid_selector' }}; }}
            }};
            const visibleReason = (element) => {{
                const rect = element.getBoundingClientRect();
                if (rect.width <= 0 || rect.height <= 0) return 'zero_size';
                for (let current = element; current; current = current.parentElement) {{
                    const view = current.ownerDocument.defaultView;
                    if (!view) return 'no_window';
                    const style = view.getComputedStyle(current);
                    if (style.display === 'none') return 'display_none';
                    if (style.visibility === 'hidden' || style.visibility === 'collapse')
                        return 'visibility_hidden';
                    if (style.contentVisibility === 'hidden')
                        return 'content_visibility_hidden';
                    if (parseFloat(style.opacity) === 0) return 'zero_opacity';
                }}
                return null;
            }};
            let found = find();
            if (found.error) return found;
            if (!found.element) return {{ error: 'not_found' }};
            const restoreX = window.scrollX;
            const restoreY = window.scrollY;
            const restoreAndFail = (error) => {{
                window.scrollTo(restoreX, restoreY);
                return {{ error }};
            }};
            found.element.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }});
            await new Promise((resolve) => setTimeout(resolve, {settle_ms}));
            found = find();
            if (found.error) return restoreAndFail(found.error);
            if (!found.element) return restoreAndFail('detached');
            const reason = visibleReason(found.element);
            if (reason) return restoreAndFail(reason);
            const rect = found.element.getBoundingClientRect();
            return {{
                ok: true,
                x: window.scrollX + rect.left,
                y: window.scrollY + rect.top,
                width: rect.width,
                height: rect.height,
                restoreX,
                restoreY,
            }};
        }})()"#,
        settle_ms = SELECTOR_SCROLL_SETTLE.as_millis(),
    ))
}

/// Shared rounded error artwork for captures and report previews. Capture failure
/// detection uses the JSON sidecar, not the displayed pixels.
pub(crate) fn error_icon_png() -> &'static [u8] {
    include_bytes!("data/error-icon.png")
}

/// Sidecar JSON marker recording that a screenshot is a capture failure.
#[derive(Debug, Serialize, Deserialize)]
struct ErrorMarker {
    error: bool,
    #[serde(default)]
    message: String,
}

/// Path of the JSON error-marker sidecar for a screenshot file.
/// e.g. `scenario__FINAL.png` -> `scenario__FINAL.png.error.json`.
pub fn error_marker_path(png_path: &Path) -> PathBuf {
    let mut s = png_path.as_os_str().to_owned();
    s.push(".error.json");
    PathBuf::from(s)
}

/// Write error placeholders for every missing expected capture target.
pub fn write_error_screenshots(
    test_suite: &TestSuite,
    scenario: &TestScenario,
    screenshot_types: &[ScreenshotType],
    viewport_label: Option<&str>,
    message: &str,
) -> Result<()> {
    if screenshot_types.is_empty() {
        anyhow::bail!("Cannot write an error screenshot without capture targets");
    }

    let marker = ErrorMarker {
        error: true,
        message: message.to_string(),
    };
    let marker_json = serde_json::to_vec_pretty(&marker).map_err(|error| {
        artifact_write_error("Failed to serialize screenshot error marker", error)
    })?;
    for screenshot_type in screenshot_types {
        let path = screenshot_path(test_suite, scenario, screenshot_type, viewport_label).map_err(
            |error| artifact_write_error("Failed to create error screenshot output path", error),
        )?;
        write_error_screenshot_if_missing(&path, &marker_json)?;
    }
    Ok(())
}

/// Preserve a completed capture when a later action fails; otherwise create a
/// marker-backed placeholder for the still-missing target.
fn write_error_screenshot_if_missing(path: &Path, marker_json: &[u8]) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    write_file_atomically(&error_marker_path(path), marker_json)
        .map_err(|error| artifact_write_error("Failed to write screenshot error marker", error))?;
    write_file_atomically(path, error_icon_png())
        .map_err(|error| artifact_write_error("Failed to write error screenshot", error))?;
    Ok(true)
}

/// Read the error marker for a screenshot. Returns the recorded failure message
/// when the screenshot is flagged as a capture failure (`error: true`), or
/// `None` when there is no marker or the flag is false.
pub fn read_error_marker(png_path: &Path) -> Option<String> {
    let marker_path = error_marker_path(png_path);
    let bytes = match fs::read(&marker_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            return Some(format!(
                "Failed to read capture error marker '{}': {}",
                marker_path.display(),
                error
            ));
        }
    };
    match serde_json::from_slice::<ErrorMarker>(&bytes) {
        Ok(marker) => marker.error.then_some(marker.message),
        Err(error) => Some(format!(
            "Failed to parse capture error marker '{}': {}",
            marker_path.display(),
            error
        )),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reference_promotion_does_not_fabricate_log_and_rejects_stale_image_links() {
        use crate::diagnostics::{journal::*, record::*, runtime::ArchiveReference};
        let root = tempfile::tempdir().unwrap();
        let suite = root.path().join("suite");
        let captures = suite.join("bitmaps_test/run");
        std::fs::create_dir_all(&captures).unwrap();
        let source = captures.join("page__FINAL.png");
        std::fs::write(
            &source,
            lodepng::encode32(&[10_u8, 20, 30, 255], 1, 1).unwrap(),
        )
        .unwrap();
        let metadata = OperationMetadata::new(OperationKind::Capture, None);
        let mut writer =
            JournalWriter::create(root.path(), metadata.clone(), JournalOptions::default())
                .unwrap();
        writer.finalize(Outcome::Success).unwrap();
        let reference = ArchiveReference {
            operation_id: metadata.operation_id.clone(),
            execution_id: None,
            state: ArchiveState::Complete,
            missing_records: Some(0),
        };
        super::write_capture_log(root.path(), &source, &reference).unwrap();
        crate::compare::report_gen::approve_screenshots(&suite, Some("run")).unwrap();
        let destination = suite.join("bitmaps_reference/page__FINAL.png");
        assert_eq!(
            super::read_capture_log(root.path(), &destination)
                .unwrap()
                .archive,
            Some(reference)
        );
        assert_eq!(
            std::fs::read_dir(root.path().join("_diagnostics"))
                .unwrap()
                .count(),
            1
        );
        std::fs::write(
            &destination,
            lodepng::encode32(&[250_u8, 250, 250, 255], 1, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(
            super::read_capture_log(root.path(), &destination)
                .unwrap()
                .state,
            ArchiveState::LoadingError
        );
    }

    #[tokio::test]
    async fn passed_capture_with_mismatch_shows_distinct_capture_and_comparison_outcomes() {
        use crate::diagnostics::{journal::JournalReader, record::*, runtime as diagnostics};
        let root = tempfile::tempdir().unwrap();
        let suite = root.path().join("shop");
        let captures = suite.join("bitmaps_test/run");
        let references = suite.join("bitmaps_reference");
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::create_dir_all(&references).unwrap();
        let path = captures.join("home__FINAL.png");
        std::fs::write(
            references.join("home__FINAL.png"),
            lodepng::encode32(&[0_u8, 0, 0, 255], 1, 1).unwrap(),
        )
        .unwrap();
        let operation =
            diagnostics::Operation::start(root.path(), OperationKind::Capture, None, None, None);
        diagnostics::in_scope(Some(operation.context()), || {
            diagnostics::stage_sync(Stage::CaptureArtifacts, [], || {
                std::fs::write(&path, lodepng::encode32(&[255_u8, 255, 255, 255], 1, 1)?)?;
                Ok(())
            })
        })
        .unwrap();
        let reference = operation.finish(Outcome::Success).await;
        super::write_capture_log(root.path(), &path, &reference).unwrap();
        let reader = JournalReader::open(root.path(), &reference.operation_id).unwrap();
        let index =
            crate::diagnostics::journal::archive_directory(root.path(), &reference.operation_id)
                .join("index.json");
        let before = std::fs::read(&index).unwrap();
        crate::compare::report_gen::generate_comparison_report(&suite, "shop", "run", 0.0, 1)
            .unwrap();
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(captures.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["tests"][0]["pair"]["comparisonOutcome"], "mismatch");
        assert_eq!(
            report["tests"][0]["pair"]["testLog"]["archive"]["operationId"],
            reference.operation_id.as_str()
        );
        let mut finished = false;
        reader
            .visit(|record| {
                if record.event_name == DiagnosticEvent::Finished {
                    finished = true;
                    assert_eq!(record.outcome, Some(Outcome::Success));
                }
            })
            .unwrap();
        assert!(finished);
        assert_eq!(std::fs::read(index).unwrap(), before);
    }

    #[test]
    fn corrupt_capture_log_is_loading_error_not_legacy_or_capture_failure() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("home__FINAL.png");
        assert!(super::read_capture_log(root.path(), &image).is_none());
        std::fs::write(root.path().join(".diagnostics.json"), br#"{"version":1}"#).unwrap();
        assert_eq!(
            super::read_capture_log(root.path(), &image).unwrap().state,
            crate::diagnostics::journal::ArchiveState::Missing
        );
        std::fs::write(image.with_extension("log.json"), b"not JSON").unwrap();
        let log = super::read_capture_log(root.path(), &image).unwrap();
        assert_eq!(
            log.state,
            crate::diagnostics::journal::ArchiveState::LoadingError
        );
        assert!(log.archive.is_none());
    }

    use super::*;

    #[test]
    fn state_source_prefers_current_output_and_only_reads_legacy_fallback() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join(".regressify");
        std::fs::create_dir(root.path().join(".states")).unwrap();
        let legacy = root.path().join(".states/auth.json");
        std::fs::write(&legacy, "{\"cookies\":[],\"origins\":[]}").unwrap();
        assert_eq!(
            persisted_state_source(root.path(), &output, "auth").unwrap(),
            legacy.canonicalize().unwrap()
        );
        std::fs::create_dir_all(output.join("states")).unwrap();
        let current = output.join("states/auth.json");
        std::fs::write(&current, "{}").unwrap();
        assert_eq!(
            persisted_state_source(root.path(), &output, "auth").unwrap(),
            current
        );
        assert!(legacy.exists());
    }

    #[test]
    fn cookie_domains_expand_without_changing_other_attributes() {
        let cookies = serde_json::json!([
            {"name": "session", "value": "token", "domain": [".example.com", ".example.org"], "path": "/", "secure": true},
            {"name": "theme", "value": "dark", "domain": "example.net"}
        ]);
        let expanded = expand_cookie_entries(cookies.as_array().unwrap(), "fixture").unwrap();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0]["domain"], ".example.com");
        assert_eq!(expanded[1]["domain"], ".example.org");
        assert_eq!(expanded[0]["value"], expanded[1]["value"]);
        assert_eq!(expanded[1]["secure"], true);
        for domain in [
            serde_json::json!([]),
            serde_json::json!([12]),
            serde_json::json!(["valid.example", ""]),
        ] {
            let cookie = serde_json::json!({"name": "session", "value": "token", "domain": domain});
            assert!(expand_cookie_entries(&[cookie], "fixture").is_err());
        }
    }

    fn test_suite() -> TestSuite {
        serde_json::from_value(serde_json::json!({
            "id": "suite",
            "testDate": "run",
            "scenarios": []
        }))
        .unwrap()
    }

    fn test_scenario() -> TestScenario {
        serde_json::from_value(serde_json::json!({
            "id": "home",
            "url": "https://example.com"
        }))
        .unwrap()
    }

    fn png_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut chunk = Vec::with_capacity(data.len() + 12);
        chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(chunk_type);
        chunk.extend_from_slice(data);
        chunk.extend_from_slice(&[0, 0, 0, 0]);
        chunk
    }

    fn chunk_types(png: &[u8]) -> Vec<[u8; 4]> {
        let mut types = Vec::new();
        let mut pos = 8;
        while pos < png.len() {
            let data_len =
                u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]]) as usize;
            types.push([png[pos + 4], png[pos + 5], png[pos + 6], png[pos + 7]]);
            pos += data_len + 12;
        }
        types
    }

    #[test]
    fn error_icon_is_deterministic_and_valid_png() {
        const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
        let bytes = error_icon_png();
        assert!(std::ptr::eq(bytes, error_icon_png()));
        assert_eq!(
            &bytes[..8],
            &PNG_SIGNATURE,
            "error icon must be a valid PNG"
        );
        assert!(lodepng::decode32(bytes).is_ok());
        assert!(
            bytes == include_bytes!("data/error-icon.png"),
            "Capture failures must use the same rounded error artwork as the report"
        );
    }

    #[test]
    fn report_page_urls_serialize_with_report_ui_field_names() {
        let manifest = ReportUrlsManifest {
            version: REPORT_URLS_VERSION,
            captures: BTreeMap::from([(
                "home__FINAL".to_string(),
                ReportPageUrls {
                    scenario_id: None,
                    scenario_label: None,
                    scenario_display_name: Some("Home".into()),
                    reference_url: "https://www.example.com/home".to_string(),
                    url: "https://test.example.com/home".to_string(),
                },
            )]),
        };

        let value = serde_json::to_value(manifest).unwrap();

        assert_eq!(
            value["captures"]["home__FINAL"]["referenceUrl"],
            "https://www.example.com/home"
        );
        assert_eq!(
            value["captures"]["home__FINAL"]["url"],
            "https://test.example.com/home"
        );
        assert_eq!(
            value["captures"]["home__FINAL"]["scenarioDisplayName"],
            "Home"
        );
    }

    #[test]
    fn error_marker_round_trips_and_reflects_flag() {
        let temp = tempfile::tempdir().unwrap();
        let png = temp.path().join("scenario__FINAL.png");
        fs::write(&png, error_icon_png()).unwrap();

        // No marker yet.
        assert!(read_error_marker(&png).is_none());

        // error: true -> returns the message.
        let marker = ErrorMarker {
            error: true,
            message: "boom".to_string(),
        };
        fs::write(
            error_marker_path(&png),
            serde_json::to_vec(&marker).unwrap(),
        )
        .unwrap();
        assert_eq!(read_error_marker(&png).as_deref(), Some("boom"));

        // error: false -> treated as no failure.
        let ok = ErrorMarker {
            error: false,
            message: String::new(),
        };
        fs::write(error_marker_path(&png), serde_json::to_vec(&ok).unwrap()).unwrap();
        assert!(read_error_marker(&png).is_none());

        fs::write(error_marker_path(&png), b"not json").unwrap();
        assert!(
            read_error_marker(&png)
                .unwrap()
                .contains("Failed to parse capture error marker")
        );
    }

    #[test]
    fn error_placeholders_preserve_completed_screenshots() {
        let temp = tempfile::tempdir().unwrap();
        let screenshot = temp.path().join("completed.png");
        let marker_json = serde_json::to_vec(&ErrorMarker {
            error: true,
            message: "later action failed".to_string(),
        })
        .unwrap();
        fs::write(&screenshot, b"completed screenshot").unwrap();

        assert!(!write_error_screenshot_if_missing(&screenshot, &marker_json).unwrap());
        assert_eq!(fs::read(&screenshot).unwrap(), b"completed screenshot");
        assert!(!error_marker_path(&screenshot).exists());
    }

    #[test]
    fn error_placeholders_mark_missing_screenshots() {
        let temp = tempfile::tempdir().unwrap();
        let screenshot = temp.path().join("missing.png");
        let marker_json = serde_json::to_vec(&ErrorMarker {
            error: true,
            message: "capture failed".to_string(),
        })
        .unwrap();

        assert!(write_error_screenshot_if_missing(&screenshot, &marker_json).unwrap());
        assert_eq!(fs::read(&screenshot).unwrap(), error_icon_png());
        assert_eq!(
            read_error_marker(&screenshot).as_deref(),
            Some("capture failed")
        );
    }

    #[test]
    fn screenshot_path_uses_explicit_output_directory() {
        let temp = tempfile::tempdir().unwrap();
        let output_dir = temp.path().join("custom-output");
        let path = screenshot_path_in(
            &output_dir,
            &test_suite(),
            &test_scenario(),
            &ScreenshotType::Final,
            Some("Mobile Device"),
        )
        .unwrap();

        assert_eq!(
            path,
            output_dir.join("suite/bitmaps_test/run/home_mobile-device__FINAL.png")
        );
        assert!(path.parent().unwrap().exists());
    }

    #[test]
    fn metadata_normalization_removes_text_but_preserves_color_data() {
        let base = lodepng::encode32(&[10_u8, 20, 30, 255], 1, 1).unwrap();
        let iend_start = base.len() - 12;
        let mut png = base[..iend_start].to_vec();
        png.extend_from_slice(&png_chunk(b"tEXt", b"generated-at=now"));
        png.extend_from_slice(&png_chunk(b"gAMA", &[0, 0, 177, 24]));
        png.extend_from_slice(&base[iend_start..]);

        let normalized = strip_png_metadata(&png).unwrap();
        let chunk_types = chunk_types(&normalized);

        assert!(chunk_types.contains(b"gAMA"));
        assert!(!chunk_types.contains(b"tEXt"));
    }

    #[test]
    fn metadata_normalization_rejects_missing_or_trailing_iend_data() {
        let png = lodepng::encode32(&[10_u8, 20, 30, 255], 1, 1).unwrap();
        let missing_iend = png[..png.len() - 12].to_vec();
        let mut trailing = png;
        trailing.push(0);

        assert!(
            strip_png_metadata(&missing_iend)
                .unwrap_err()
                .to_string()
                .contains("missing its IEND")
        );
        assert!(
            strip_png_metadata(&trailing)
                .unwrap_err()
                .to_string()
                .contains("Invalid PNG IEND")
        );
    }

    #[test]
    fn final_capture_targets_default_to_full_page_and_validate_selectors() {
        let mut scenario = test_scenario();
        assert_eq!(
            final_screenshot_types(&scenario).unwrap(),
            vec![ScreenshotType::Final]
        );

        scenario.selectors = Some(Selectors::StringArray(vec![
            "viewport".to_string(),
            "main .hero".to_string(),
        ]));
        assert_eq!(
            final_screenshot_types(&scenario).unwrap(),
            vec![
                ScreenshotType::Viewport,
                ScreenshotType::Selector("main .hero".to_string()),
            ]
        );

        scenario.selectors = Some(Selectors::StringArray(vec![" ".to_string()]));
        assert!(
            final_screenshot_types(&scenario)
                .unwrap_err()
                .to_string()
                .contains("empty value")
        );
    }

    #[test]
    fn scenario_capture_names_reject_duplicate_or_unsafe_step_screenshots() {
        let mut scenario = test_scenario();
        scenario.actions = Some(vec![
            TestAction::Save(crate::types::SaveAction {
                save: "progress".to_string(),
                frame: None,
                viewport_names: None,
            }),
            TestAction::Save(crate::types::SaveAction {
                save: "Progress".to_string(),
                frame: None,
                viewport_names: None,
            }),
        ]);
        assert!(
            validate_scenario_capture_names(&scenario)
                .unwrap_err()
                .to_string()
                .contains("duplicate filename")
        );

        scenario.actions = Some(vec![TestAction::Save(crate::types::SaveAction {
            save: "result_diff".to_string(),
            frame: None,
            viewport_names: None,
        })]);
        assert!(
            validate_scenario_capture_names(&scenario)
                .unwrap_err()
                .to_string()
                .contains("Invalid save name")
        );
    }

    #[test]
    fn screenshot_step_names_reject_unsafe_filename_components() {
        assert!(validate_screenshot_step_name("after-login").is_ok());
        assert!(validate_screenshot_step_name("report:final").is_err());
        assert!(validate_screenshot_step_name(" progress").is_err());
        assert!(validate_screenshot_step_name("progress ").is_err());
        assert!(validate_screenshot_step_name("progress_diff").is_err());
    }

    #[test]
    fn capture_dimensions_must_be_finite_positive_and_bounded() {
        assert!(validate_capture_dimensions(1920.0, 10_000.0, "test").is_ok());
        assert!(validate_capture_dimensions(8_000.0, 8_000.0, "test").is_ok());
        assert!(
            validate_capture_dimensions(0.0, 100.0, "test")
                .unwrap_err()
                .to_string()
                .contains("finite, positive")
        );
        assert!(validate_capture_dimensions(f64::NAN, 100.0, "test").is_err());
        assert!(
            validate_capture_dimensions(f64::from(MAX_VISUAL_IMAGE_DIMENSION) + 1.0, 100.0, "test")
                .is_err()
        );
        assert!(
            validate_capture_dimensions(8_001.0, 8_000.0, "test")
                .unwrap_err()
                .to_string()
                .contains("CSS pixel limit")
        );
    }

    #[test]
    fn persisted_state_validation_rejects_missing_or_malformed_data() {
        let valid = serde_json::json!({
            "cookies": [{
                "name": "session",
                "value": "abc",
                "domain": ".example.com"
            }],
            "localStorage": { "theme": "dark" }
        });
        assert!(validate_persisted_state_value(&valid, "auth").is_ok());

        let invalid_cookies = serde_json::json!({ "cookies": {} });
        assert!(
            validate_persisted_state_value(&invalid_cookies, "auth")
                .unwrap_err()
                .to_string()
                .contains("cookies' must be an array")
        );

        let invalid_storage = serde_json::json!({ "localStorage": { "theme": 1 } });
        assert!(
            validate_persisted_state_value(&invalid_storage, "auth")
                .unwrap_err()
                .to_string()
                .contains("must have a string value")
        );

        assert!(validate_state_name("auth-staging").is_ok());
        assert!(validate_state_name("../auth").is_err());
        assert!(validate_state_name(" auth").is_err());
        assert!(validate_state_name("CON").is_err());
    }

    #[test]
    fn persisted_state_file_parser_reports_missing_and_malformed_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.json");
        assert!(
            read_persisted_state_file(&path, "auth")
                .unwrap_err()
                .to_string()
                .contains("Failed to read persisted state")
        );

        fs::write(&path, "not json").unwrap();
        assert!(
            read_persisted_state_file(&path, "auth")
                .unwrap_err()
                .to_string()
                .contains("Invalid persisted state")
        );
    }

    #[test]
    fn cookie_validation_requires_supported_cookie_fields() {
        let valid = serde_json::json!({
            "name": "session",
            "value": "abc",
            "domain": ".example.com",
            "sameSite": "Lax",
            "secure": true
        });
        assert!(validate_cookie_value(&valid, "cookie").is_ok());

        let invalid_url_type = serde_json::json!({
            "name": "session",
            "value": "abc",
            "domain": ".example.com",
            "url": 1
        });
        assert!(
            validate_cookie_value(&invalid_url_type, "cookie")
                .unwrap_err()
                .to_string()
                .contains("field 'url' must be a string")
        );

        let invalid_same_site = serde_json::json!({
            "name": "session",
            "value": "abc",
            "domain": ".example.com",
            "sameSite": "Invalid"
        });
        assert!(
            validate_cookie_value(&invalid_same_site, "cookie")
                .unwrap_err()
                .to_string()
                .contains("Strict, Lax, or None")
        );

        let insecure_none = serde_json::json!({
            "name": "session",
            "value": "abc",
            "domain": ".example.com",
            "sameSite": "None"
        });
        assert!(
            validate_cookie_value(&insecure_none, "cookie")
                .unwrap_err()
                .to_string()
                .contains("requires secure: true")
        );
    }

    #[test]
    fn cookie_validation_rejects_yaml_value_line_breaks_without_exposing_values() {
        for indicator in ["|", ">"] {
            let yaml = format!(
                "name: .AspNetCore.CookiesC1\ndomain: example.com\nvalue: {indicator}\n  private-test-token\n"
            );
            let cookie: serde_json::Value = serde_yaml::from_str(&yaml).unwrap();
            let error = validate_cookie_value(&cookie, "cookiePath 'cookies.yaml' cookie 0")
                .expect_err("YAML block scalars retain a trailing newline")
                .to_string();

            assert!(error.contains("cookiePath 'cookies.yaml' cookie 0"));
            assert!(error.contains(".AspNetCore.CookiesC1"));
            assert!(error.contains("field 'value'"));
            assert!(error.contains("single-line"));
            assert!(!error.contains("private-test-token"));
        }
    }

    #[test]
    fn cookie_validation_enforces_chromium_name_and_value_byte_limit() {
        let name = ".AspNetCore.CookiesC1";
        for (value, valid) in [
            ("x".repeat(4096 - name.len()), true),
            ("x".repeat(4097 - name.len()), false),
            ("x".repeat(7746), false),
            ("\u{e9}".repeat((4096 - name.len()) / 2), true),
            ("\u{e9}".repeat((4096 - name.len()) / 2 + 1), false),
        ] {
            let cookie = serde_json::json!({
                "name": name,
                "value": value,
                "domain": "example.com"
            });
            let result = validate_cookie_value(&cookie, "cookiePath 'cookies.yaml' cookie 1");

            if valid {
                assert!(result.is_ok());
            } else {
                let error = result.unwrap_err().to_string();
                assert!(error.contains("cookiePath 'cookies.yaml' cookie 1"));
                assert!(error.contains(name));
                assert!(error.contains(&format!("{} bytes", name.len() + value.len())));
                assert!(error.contains("4096-byte cookie limit"));
                assert!(error.contains("re-export"));
                assert!(!error.contains(&value));
            }
        }

        let oversized_name = serde_json::json!({
            "name": "x".repeat(4097),
            "value": "",
            "domain": "example.com"
        });
        assert!(validate_cookie_value(&oversized_name, "cookie").is_err());
    }

    #[test]
    fn cookie_validation_accepts_yaml_values_without_line_breaks() {
        for value in ["'private-test-token'", "|-\n  private-test-token", "''"] {
            let yaml =
                format!("name: .AspNetCore.CookiesC1\ndomain: example.com\nvalue: {value}\n");
            let cookie: serde_json::Value = serde_yaml::from_str(&yaml).unwrap();

            assert!(validate_cookie_value(&cookie, "cookie").is_ok());
        }
    }

    #[test]
    fn selector_capture_names_are_readable_and_collision_resistant() {
        let first = screenshot_type_name(&ScreenshotType::Selector("#checkout".to_string()));
        let second = screenshot_type_name(&ScreenshotType::Selector(".checkout".to_string()));

        assert!(first.starts_with("SELECTOR-checkout-"));
        assert!(second.starts_with("SELECTOR-checkout-"));
        assert_ne!(first, second);
    }

    #[test]
    fn final_capture_order_key_includes_the_target_name() {
        assert_eq!(
            final_screenshot_order_key("home", Some("Mobile Device"), &ScreenshotType::Viewport,),
            "home_mobile-device__VIEWPORT"
        );
    }

    #[test]
    fn selector_capture_script_is_valid_javascript_when_node_is_available() {
        use std::process::Command;

        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let script_path = temp.path().join("selector-capture.js");
        fs::write(&script_path, selector_capture_script("main .hero").unwrap()).unwrap();
        let output = Command::new("node")
            .arg("--check")
            .arg(&script_path)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn selector_capture_script_preserves_and_restores_scroll_on_failure() {
        use std::process::Command;

        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let script_path = temp.path().join("selector-scroll-runtime.js");
        let capture_script = selector_capture_script("#target").unwrap();
        let runtime = format!(
            r#"global.window = {{
    scrollX: 12,
    scrollY: 34,
    scrollTo(x, y) {{ this.scrollX = x; this.scrollY = y; }},
}};
const element = {{
    ownerDocument: {{ defaultView: {{ getComputedStyle: () => ({{ display: 'block', visibility: 'visible', opacity: '1' }}) }} }},
    scrollIntoView() {{ window.scrollTo(0, 500); }},
    getBoundingClientRect() {{ return {{ left: 20, top: 30, width: 40, height: 50 }}; }},
}};
let calls = 0;
global.document = {{
    querySelector() {{ return ++calls === 1 ? element : null; }},
}};
(async () => {{
    const result = await {capture_script};
    if (result.error !== 'detached' || window.scrollX !== 12 || window.scrollY !== 34)
        throw new Error(JSON.stringify({{ result, scrollX: window.scrollX, scrollY: window.scrollY }}));
}})().catch((error) => {{ console.error(error); process.exitCode = 1; }});
"#
        );
        fs::write(&script_path, runtime).unwrap();
        let output = Command::new("node").arg(&script_path).output().unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn selector_capture_script_rejects_content_hidden_targets() {
        use std::process::Command;

        if Command::new("node").arg("--version").output().is_err() {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let script_path = temp.path().join("selector-content-visibility-runtime.js");
        let capture_script = selector_capture_script("#target").unwrap();
        let runtime = format!(
            r#"global.window = {{
    scrollX: 12,
    scrollY: 34,
    scrollTo(x, y) {{ this.scrollX = x; this.scrollY = y; }},
}};
const element = {{
    parentElement: null,
    ownerDocument: {{
        defaultView: {{
            getComputedStyle: () => ({{
                display: 'block',
                visibility: 'visible',
                contentVisibility: 'hidden',
                opacity: '1',
            }}),
        }},
    }},
    scrollIntoView() {{ window.scrollTo(0, 500); }},
    getBoundingClientRect() {{ return {{ left: 20, top: 30, width: 40, height: 50 }}; }},
}};
global.document = {{ querySelector: () => element }};
(async () => {{
    const result = await {capture_script};
    if (result.error !== 'content_visibility_hidden' || window.scrollX !== 12 || window.scrollY !== 34)
        throw new Error(JSON.stringify({{ result, scrollX: window.scrollX, scrollY: window.scrollY }}));
}})().catch((error) => {{ console.error(error); process.exitCode = 1; }});
"#
        );
        fs::write(&script_path, runtime).unwrap();
        let output = Command::new("node").arg(&script_path).output().unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn artifact_write_errors_remain_identifiable_through_anyhow() {
        let error = artifact_write_error("Failed to write screenshot", "disk full");

        assert!(is_artifact_write_error(&error));
        assert_eq!(error.to_string(), "Failed to write screenshot: disk full");
    }
}
