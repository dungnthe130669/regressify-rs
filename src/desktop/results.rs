//! Read existing report artifacts without treating CLI exit zero as a visual pass.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow, ensure};
use serde::Deserialize;
use walkdir::WalkDir;

use super::workspace::{Entry, SuiteSummary};

const MAX_REPORT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REPORTS: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Mismatch,
    Error,
    Unknown,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "Passed",
            Self::Mismatch => "Mismatch",
            Self::Error => "Error",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Report {
    pub id: String,
    #[serde(default)]
    pub test_suite: String,
    pub tests: Vec<TestResult>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TestResult {
    pub status: String,
    pub pair: Pair,
}

impl TestResult {
    pub fn copy_summary(&self) -> String {
        let mut lines = vec![
            format!("Scenario: {}", self.pair.display.title),
            format!("Status: {}", self.outcome().label()),
        ];
        if !self.pair.display.detail.is_empty() {
            lines.push(format!("Capture: {}", self.pair.display.detail));
        }
        if let Some(url) = &self.pair.display.page_url {
            lines.push(format!("Page: {url}"));
        }
        if let Some(diff) = &self.pair.diff {
            if !diff.mis_match_percentage.is_empty() {
                lines.push(format!("Mismatch: {}%", diff.mis_match_percentage));
            } else if let Some(percentage) = diff.raw_mis_match_percentage {
                lines.push(format!("Mismatch: {percentage:.3}%"));
            }
        }
        if let Some(threshold) = self.pair.mis_match_threshold {
            lines.push(format!("Allowed mismatch: {threshold}%"));
        }
        if let Some(error) = &self.pair.engine_error_msg {
            lines.push(format!("Error: {error}"));
        }
        lines.push(format!("Capture ID: {}", self.pair.capture_key()));
        lines.join("\n")
    }

    pub fn outcome(&self) -> Outcome {
        if self
            .pair
            .engine_error_msg
            .as_ref()
            .is_some_and(|message| !message.is_empty())
        {
            Outcome::Error
        } else {
            match self.status.as_str() {
                "pass" => Outcome::Passed,
                "fail" => Outcome::Mismatch,
                _ => Outcome::Unknown,
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    #[serde(default)]
    pub label: String,
    pub scenario_id: Option<String>,
    pub scenario_label: Option<String>,
    #[serde(default)]
    pub file_name: String,
    #[serde(skip)]
    pub display: CaptureDisplay,
    pub reference: Option<String>,
    pub test: Option<String>,
    pub diff_image: Option<String>,
    pub engine_error_msg: Option<String>,
    pub reference_url: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub selector: String,
    pub diff: Option<Difference>,
    pub mis_match_threshold: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct CaptureDisplay {
    pub title: String,
    pub detail: String,
    pub scenario_index: Option<usize>,
    pub page_url: Option<String>,
}

impl Pair {
    pub fn capture_key(&self) -> &str {
        let filename = if self.file_name.is_empty() {
            self.test
                .as_deref()
                .or(self.reference.as_deref())
                .unwrap_or(&self.label)
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&self.label)
        } else {
            &self.file_name
        };
        filename.strip_suffix(".png").unwrap_or(filename)
    }

    pub fn matches(&self, query: &str) -> bool {
        let fields: [&str; 8] = [
            &self.display.title,
            &self.display.detail,
            &self.label,
            &self.file_name,
            self.scenario_id.as_deref().unwrap_or_default(),
            self.url.as_deref().unwrap_or_default(),
            self.reference_url.as_deref().unwrap_or_default(),
            self.engine_error_msg.as_deref().unwrap_or_default(),
        ];
        fields
            .iter()
            .any(|text| text.to_lowercase().contains(query))
            || self.capture_key().to_lowercase().contains(query)
    }

    fn resolve_name(&mut self, suite: Option<&SuiteSummary>) {
        let key = self.capture_key();
        let scenarios = suite
            .map(|suite| suite.scenarios.as_slice())
            .unwrap_or_default();
        let mut matches = scenarios
            .iter()
            .enumerate()
            .filter(|(_, scenario)| {
                if let Some(id) = &self.scenario_id {
                    return scenario.id == *id;
                }
                !scenario.id.is_empty()
                    && key
                        .strip_prefix(&scenario.id)
                        .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('_'))
            })
            .collect::<Vec<_>>();
        let longest = matches.iter().map(|(_, scenario)| scenario.id.len()).max();
        matches.retain(|(_, scenario)| Some(scenario.id.len()) == longest);
        let matched = (matches.len() == 1).then(|| matches[0]);
        let id = self
            .scenario_id
            .as_deref()
            .or_else(|| matched.map(|(_, scenario)| scenario.id.as_str()));
        let (viewport, target) = id
            .and_then(|id| key.strip_prefix(id))
            .map(|suffix| {
                if let Some(target) = suffix.strip_prefix("__") {
                    ("", target)
                } else {
                    suffix
                        .strip_prefix('_')
                        .unwrap_or(suffix)
                        .split_once("__")
                        .unwrap_or(("", ""))
                }
            })
            .unwrap_or_else(|| {
                let (prefix, target) = key.split_once("__").unwrap_or((key, ""));
                let viewport = prefix
                    .split_once('_')
                    .filter(|(prefix, _)| generated_id(prefix))
                    .map_or("", |(_, viewport)| viewport);
                (viewport, target)
            });
        let generated = self.label.is_empty()
            || self.label.contains("__")
            || self.label == key.replace('_', " ")
            || self.label == key.replace("__", " ")
            || self.label
                == key
                    .replace('_', " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
        let name = self
            .scenario_label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .or_else(|| {
                matched.map(|(_, scenario)| {
                    if scenario.label == scenario.id {
                        scenario.url.as_str()
                    } else {
                        scenario.label.as_str()
                    }
                })
            })
            .or_else(|| (!generated).then_some(self.label.as_str()))
            .or(self.url.as_deref())
            .or(self.reference_url.as_deref());
        let title = name
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                let prefix = key.split_once("__").map_or(key, |(prefix, _)| prefix);
                if generated_id(prefix.split('_').next().unwrap_or(prefix)) {
                    "Unnamed scenario".into()
                } else {
                    humanize(prefix)
                }
            });
        let capture = match target {
            "FINAL" => "Full page".into(),
            "VIEWPORT" => "Viewport capture".into(),
            target if target.starts_with('$') => format!("Saved: {}", &target[1..]),
            target if target.starts_with("SELECTOR-") => "Element capture".into(),
            _ => match self.selector.as_str() {
                "document" => "Full page".into(),
                "viewport" => "Viewport capture".into(),
                selector if selector.starts_with("save:") => format!("Saved: {}", &selector[5..]),
                selector if selector.starts_with("SELECTOR-") => "Element capture".into(),
                selector => selector.to_string(),
            },
        };
        let detail = [humanize(viewport), capture]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" / ");
        self.display = CaptureDisplay {
            title,
            detail,
            scenario_index: matched.map(|(index, _)| index),
            page_url: self
                .url
                .clone()
                .or_else(|| self.reference_url.clone())
                .or_else(|| matched.map(|(_, scenario)| scenario.url.clone())),
        };
    }
}

fn generated_id(text: &str) -> bool {
    text.len() >= 16 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn humanize(text: &str) -> String {
    let text = text
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut characters = text.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
        .unwrap_or_default()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    pub raw_mis_match_percentage: Option<f64>,
    #[serde(default)]
    pub mis_match_percentage: String,
}

#[derive(Clone, Debug)]
pub struct ReportEntry {
    pub path: PathBuf,
    pub suite_directory: PathBuf,
    pub run_id: String,
    pub modified: SystemTime,
    pub report: Report,
    pub suite_name: String,
    pub source: Option<Entry>,
}

impl ReportEntry {
    pub fn label(&self) -> String {
        let generated = self
            .run_id
            .strip_prefix("desktop-")
            .is_some_and(generated_id);
        let captured = chrono::NaiveDateTime::parse_from_str(&self.run_id, "%Y%m%d_%H%M%S_%f")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(&self.run_id, "%Y%m%d_%H%M%S"))
            .ok();
        let run = if let Some(captured) = captured {
            captured
                .and_utc()
                .with_timezone(&chrono::Local)
                .format("%d %b %Y %H:%M:%S")
                .to_string()
        } else if generated {
            chrono::DateTime::<chrono::Local>::from(self.modified)
                .format("%d %b %Y %H:%M:%S")
                .to_string()
        } else {
            self.run_id.clone()
        };
        format!("{} / {run}", self.suite_name)
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        let mut passed = 0;
        let mut mismatched = 0;
        let mut errors = 0;
        for test in &self.report.tests {
            match test.outcome() {
                Outcome::Passed => passed += 1,
                Outcome::Mismatch => mismatched += 1,
                _ => errors += 1,
            }
        }
        (passed, mismatched, errors)
    }

    pub fn artifact(&self, output_root: &Path, relative: &str) -> Result<PathBuf> {
        let relative = relative.replace('\\', "/");
        ensure!(
            !relative.is_empty() && !relative.contains(':') && !relative.starts_with('/'),
            "Report artifact must be a relative file path"
        );
        let root = output_root.canonicalize()?;
        let path = self
            .suite_directory
            .join("html_report")
            .join(relative)
            .canonicalize()
            .context("Report artifact is missing")?;
        ensure!(
            path.starts_with(&root) && path.is_file(),
            "Report artifact escapes the output directory"
        );
        Ok(path)
    }

    pub fn image(&self, output_root: &Path, relative: &str) -> Result<ImageInfo> {
        let path = self.artifact(output_root, relative)?;
        ensure!(
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png")),
            "Only PNG report images are supported"
        );
        ensure!(
            fs::metadata(&path)?.len() <= 128 * 1024 * 1024,
            "Image file exceeds the preview size limit"
        );
        let (width, height) = crate::helpers::png::read_bounded_png_dimensions(&path)?;
        Ok(ImageInfo {
            path,
            width,
            height,
        })
    }

    pub fn html_report(&self, output_root: &Path) -> Result<PathBuf> {
        self.artifact(output_root, "index.html")
    }
}

#[derive(Clone, Debug)]
pub struct ImageInfo {
    pub path: PathBuf,
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Debug)]
pub struct SavedRun {
    pub run_id: String,
    pub modified: SystemTime,
    pub has_report: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SuiteArtifacts {
    pub reference_count: usize,
    pub runs: Vec<SavedRun>,
}

impl SuiteArtifacts {
    pub fn load(output_root: &Path, suite_id: &str) -> Result<Self> {
        let mut components = Path::new(suite_id).components();
        ensure!(
            matches!(components.next(), Some(std::path::Component::Normal(_)))
                && components.next().is_none(),
            "Invalid suite output ID"
        );
        if !output_root.try_exists()? {
            return Ok(Self::default());
        }
        let root = output_root.canonicalize()?;
        let suite = root.join(suite_id);
        if !artifact_directory(&suite, &root)? {
            return Ok(Self::default());
        }
        let mut artifacts = Self::default();
        let references = suite.join("bitmaps_reference");
        if artifact_directory(&references, &root)? {
            artifacts.reference_count = capture_inventory(&references, true)?.0;
        }
        let tests = suite.join("bitmaps_test");
        if !artifact_directory(&tests, &root)? {
            return Ok(artifacts);
        }
        for entry in fs::read_dir(&tests)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || !artifact_directory(&entry.path(), &root)? {
                continue;
            }
            let run_id = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("Capture run name must be UTF-8"))?;
            let (captures, mut modified) = capture_inventory(&entry.path(), false)?;
            let report_path = entry.path().join("report.json");
            let has_report = read_report(&root, &report_path, modified).is_ok_and(|report| {
                report.report.id == suite_id && !report.report.tests.is_empty()
            });
            if captures == 0 && has_report {
                modified = fs::metadata(&report_path)?.modified()?;
            }
            if captures > 0 || has_report {
                artifacts.runs.push(SavedRun {
                    run_id,
                    modified,
                    has_report,
                });
            }
        }
        artifacts.runs.sort_by(|left, right| {
            right
                .modified
                .cmp(&left.modified)
                .then_with(|| right.run_id.cmp(&left.run_id))
        });
        artifacts.runs.truncate(MAX_REPORTS);
        Ok(artifacts)
    }
}

fn artifact_directory(path: &Path, root: &Path) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && path.canonicalize()?.starts_with(root),
        "Artifact directory is not a real directory inside the output folder: {}",
        super::workspace::display_path(path)
    );
    Ok(true)
}

fn capture_inventory(path: &Path, successful_only: bool) -> Result<(usize, SystemTime)> {
    let mut count = 0;
    let mut modified = SystemTime::UNIX_EPOCH;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".png") || name.ends_with("_diff.png") {
            continue;
        }
        if successful_only
            && (crate::test::capture::read_error_marker(&entry.path()).is_some()
                || crate::helpers::png::read_bounded_png_dimensions(&entry.path()).is_err())
        {
            continue;
        }
        count += 1;
        modified = modified.max(entry.metadata()?.modified()?);
    }
    Ok((count, modified))
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub reports: Vec<ReportEntry>,
    pub warnings: Vec<String>,
}

impl Catalog {
    pub fn resolve_names(&mut self, suites: &[SuiteSummary]) {
        for report in &mut self.reports {
            let candidates = suites
                .iter()
                .filter(|suite| {
                    suite.error.is_none() && suite.output_id.eq_ignore_ascii_case(&report.report.id)
                })
                .collect::<Vec<_>>();
            let suite = (candidates.len() == 1).then(|| candidates[0]);
            report.suite_name = suite.map(|suite| suite.name.clone()).unwrap_or_else(|| {
                let name = if report.report.test_suite.is_empty() {
                    &report.report.id
                } else {
                    &report.report.test_suite
                };
                if generated_id(name) || uuid::Uuid::parse_str(name).is_ok() {
                    "Unmatched suite".into()
                } else {
                    name.clone()
                }
            });
            report.source = suite.map(|suite| suite.entry.clone());
            for test in &mut report.report.tests {
                test.pair.resolve_name(suite);
            }
        }
    }

    pub fn load(output_root: &Path) -> Result<Self> {
        if !output_root.exists() {
            return Ok(Self::default());
        }
        let root = output_root.canonicalize()?;
        ensure!(root.is_dir(), "Output path is not a directory");
        let mut catalog = Self::default();
        let mut paths = Vec::new();
        for entry in WalkDir::new(&root).max_depth(5).follow_links(false) {
            match entry {
                Ok(entry) if entry.file_type().is_file() && entry.file_name() == "report.json" => {
                    paths.push((
                        entry
                            .metadata()?
                            .modified()
                            .unwrap_or(SystemTime::UNIX_EPOCH),
                        entry.into_path(),
                    ));
                }
                Err(error) => catalog.warnings.push(error.to_string()),
                _ => {}
            }
        }
        paths.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        if paths.len() > MAX_REPORTS {
            catalog
                .warnings
                .push(format!("Showing the newest {MAX_REPORTS} reports"));
        }
        for (modified, path) in paths.into_iter().take(MAX_REPORTS) {
            match read_report(&root, &path, modified) {
                Ok(report) => catalog.reports.push(report),
                Err(error) => catalog.warnings.push(format!(
                    "{}: {error:#}",
                    super::workspace::display_path(&path)
                )),
            }
        }
        catalog.resolve_names(&[]);
        Ok(catalog)
    }
}

fn read_report(root: &Path, path: &Path, modified: SystemTime) -> Result<ReportEntry> {
    let canonical = path.canonicalize()?;
    ensure!(
        canonical.starts_with(root),
        "Report escapes the output directory"
    );
    let run_directory = path
        .parent()
        .ok_or_else(|| anyhow!("Missing report run directory"))?;
    let bitmaps = run_directory
        .parent()
        .ok_or_else(|| anyhow!("Missing capture directory"))?;
    ensure!(
        bitmaps
            .file_name()
            .is_some_and(|name| name == "bitmaps_test"),
        "Unexpected report directory layout"
    );
    let suite_directory = bitmaps
        .parent()
        .ok_or_else(|| anyhow!("Missing suite output directory"))?
        .to_path_buf();
    let mut source = String::new();
    fs::File::open(&canonical)?
        .take(MAX_REPORT_BYTES + 1)
        .read_to_string(&mut source)?;
    ensure!(
        source.len() as u64 <= MAX_REPORT_BYTES,
        "Report exceeds the 16 MiB limit"
    );
    let report: Report = serde_json::from_str(&source).context("Invalid comparison report")?;
    ensure!(
        !report.id.trim().is_empty(),
        "Report is missing its suite ID"
    );
    let run_id = run_directory
        .file_name()
        .ok_or_else(|| anyhow!("Missing run ID"))?
        .to_string_lossy()
        .into_owned();
    Ok(ReportEntry {
        path: canonical,
        suite_directory,
        run_id,
        modified,
        report,
        suite_name: String::new(),
        source: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn suite() -> SuiteSummary {
        use super::super::{document::DocumentKind, workspace::ScenarioSummary};
        SuiteSummary {
            entry: Entry {
                path: "visual_tests/checkout.tests.yaml".into(),
                kind: DocumentKind::Suite,
            },
            name: "checkout".into(),
            output_id: "shop".into(),
            error: None,
            scenarios: vec![
                ScenarioSummary {
                    id: "8ac900fea19642cc0000".into(),
                    label: "Payment confirmation".into(),
                    url: "https://example.com/receipt".into(),
                },
                ScenarioSummary {
                    id: "checkout".into(),
                    label: "Order summary".into(),
                    url: "/order".into(),
                },
                ScenarioSummary {
                    id: "checkout_extra".into(),
                    label: "Extra details".into(),
                    url: "/extras".into(),
                },
            ],
        }
    }

    #[test]
    fn artifact_inventory_ignores_empty_folders_diffs_and_failed_baselines() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        assert_eq!(
            SuiteArtifacts::load(&root.join("missing"), "shop")
                .unwrap()
                .reference_count,
            0
        );
        let references = root.join("shop/bitmaps_reference");
        let run = root.join("shop/bitmaps_test/captured-run");
        fs::create_dir_all(&references).unwrap();
        fs::create_dir_all(&run).unwrap();
        assert!(SuiteArtifacts::load(root, "shop").unwrap().runs.is_empty());
        let png = lodepng::encode32(&[255_u8; 4], 1, 1).unwrap();
        fs::write(references.join("home_diff.png"), &png).unwrap();
        fs::write(references.join("failed.png"), &png).unwrap();
        fs::write(
            references.join("failed.png.error.json"),
            r#"{"error":true,"message":"capture failed"}"#,
        )
        .unwrap();
        fs::write(references.join("invalid.png"), "not a PNG").unwrap();
        fs::write(run.join("home_diff.png"), &png).unwrap();
        let artifacts = SuiteArtifacts::load(root, "shop").unwrap();
        assert_eq!(artifacts.reference_count, 0);
        assert!(artifacts.runs.is_empty());
        fs::write(references.join("home.png"), &png).unwrap();
        fs::write(run.join("home.png"), &png).unwrap();
        let artifacts = SuiteArtifacts::load(root, "shop").unwrap();
        assert_eq!(artifacts.reference_count, 1);
        assert_eq!(artifacts.runs.len(), 1);
        assert_eq!(artifacts.runs[0].run_id, "captured-run");
        assert!(!artifacts.runs[0].has_report);
        assert_eq!(
            SuiteArtifacts::load(root, "different-output")
                .unwrap()
                .reference_count,
            0
        );
        assert!(SuiteArtifacts::load(root, "../shop").is_err());
    }

    #[test]
    fn artifact_inventory_selects_latest_capture_not_lexically_largest_run_id() {
        let directory = tempfile::tempdir().unwrap();
        for (name, seconds) in [("zz-older-run", 10), ("aa-newer-run", 20)] {
            let run = directory.path().join("shop/bitmaps_test").join(name);
            fs::create_dir_all(&run).unwrap();
            let png = run.join("home.png");
            fs::write(&png, lodepng::encode32(&[255_u8; 4], 1, 1).unwrap()).unwrap();
            fs::File::options()
                .write(true)
                .open(png)
                .unwrap()
                .set_times(
                    fs::FileTimes::new().set_modified(
                        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
                    ),
                )
                .unwrap();
        }
        let artifacts = SuiteArtifacts::load(directory.path(), "shop").unwrap();
        assert_eq!(artifacts.runs[0].run_id, "aa-newer-run");
    }

    #[test]
    fn artifact_inventory_detects_existing_reports_and_requires_matching_suite_identity() {
        let directory = tempfile::tempdir().unwrap();
        let run = directory.path().join("shop/bitmaps_test/existing-run");
        fs::create_dir_all(&run).unwrap();
        let png = lodepng::encode32(&[255_u8; 4], 1, 1).unwrap();
        fs::write(run.join("home.png"), png).unwrap();
        for (id, expected) in [("shop", true), ("other", false)] {
            fs::write(run.join("report.json"), serde_json::to_vec(&json!({"id": id, "tests": [
                {"status":"pass","pair":{"label":"Home","test":"../bitmaps_test/existing-run/home.png"}}
            ]})).unwrap()).unwrap();
            let artifacts = SuiteArtifacts::load(directory.path(), "shop").unwrap();
            assert_eq!(artifacts.runs[0].has_report, expected);
        }
        fs::write(run.join("report.json"), "invalid json").unwrap();
        assert!(!SuiteArtifacts::load(directory.path(), "shop").unwrap().runs[0].has_report);
    }

    #[test]
    fn result_names_prefer_captured_labels_without_changing_capture_keys() {
        let mut pair: Pair = serde_json::from_value(json!({
            "label": "8ac900fea19642cc0000 desktop FINAL",
            "fileName": "8ac900fea19642cc0000_desktop__FINAL.png",
            "scenarioId": "8ac900fea19642cc0000", "scenarioLabel": "Name at capture time"
        }))
        .unwrap();
        pair.resolve_name(Some(&suite()));
        assert_eq!(pair.display.title, "Name at capture time");
        assert_eq!(pair.display.detail, "Desktop / Full page");
        assert_eq!(pair.display.scenario_index, Some(0));
        assert_eq!(pair.capture_key(), "8ac900fea19642cc0000_desktop__FINAL");
        assert!(pair.matches("name at capture"));
        assert!(pair.matches("8ac900"));
        pair.resolve_name(None);
        assert_eq!(pair.display.title, "Name at capture time");
        assert!(pair.display.scenario_index.is_none());
    }

    #[test]
    fn legacy_names_match_whole_ids_and_keep_capture_context_separate() {
        let mut pair: Pair = serde_json::from_value(json!({
            "label": "8ac900fea19642cc0000 phone $after-login", "test": "../bitmaps_test/run/8ac900fea19642cc0000_phone__$after-login.png"
        })).unwrap();
        pair.resolve_name(Some(&suite()));
        assert_eq!(pair.display.title, "Payment confirmation");
        assert_eq!(pair.display.detail, "Phone / Saved: after-login");
        pair.file_name = "checkout_extra__VIEWPORT.png".into();
        pair.resolve_name(Some(&suite()));
        assert_eq!(pair.display.title, "Extra details");
        assert_eq!(pair.display.scenario_index, Some(2));
        pair.file_name = "checkouts__FINAL.png".into();
        pair.resolve_name(Some(&suite()));
        assert!(pair.display.scenario_index.is_none());
    }

    #[test]
    fn unknown_generated_names_use_urls_and_ambiguous_suites_do_not_guess() {
        let mut pair: Pair = serde_json::from_value(json!({"label": "8ac900fea19642cc0000 FINAL", "fileName": "8ac900fea19642cc0000__FINAL.png", "url": "https://example.com/receipt"})).unwrap();
        pair.resolve_name(None);
        assert_eq!(pair.display.title, "https://example.com/receipt");
        pair.url = None;
        pair.resolve_name(None);
        assert_eq!(pair.display.title, "Unnamed scenario");
        let directory = tempfile::tempdir().unwrap();
        fixture(directory.path(), "desktop-0123456789abcdef0123456789abcdef");
        let mut catalog = Catalog::load(directory.path()).unwrap();
        catalog.resolve_names(&[suite()]);
        assert!(catalog.reports[0].label().starts_with("checkout / "));
        assert!(!catalog.reports[0].label().contains("desktop-"));
        assert!(catalog.reports[0].source.is_some());
        catalog.resolve_names(&[suite(), suite()]);
        assert!(catalog.reports[0].source.is_none());
        catalog.reports[0].report.id = "0123456789abcdef".into();
        catalog.resolve_names(&[]);
        assert_eq!(catalog.reports[0].suite_name, "Unmatched suite");
    }

    fn fixture(root: &Path, run: &str) -> PathBuf {
        let run_directory = root.join("shop/bitmaps_test").join(run);
        fs::create_dir_all(&run_directory).unwrap();
        fs::create_dir_all(root.join("shop/html_report")).unwrap();
        fs::write(root.join("shop/html_report/index.html"), "report").unwrap();
        let report = json!({"id": "shop", "tests": [
            {"status": "pass", "pair": {"label": "home desktop", "test": format!("../bitmaps_test/{run}/home.png")}},
            {"status": "fail", "pair": {"label": "cart", "diff": {"rawMisMatchPercentage": 3.5}}},
            {"status": "fail", "pair": {"label": "login", "engineErrorMsg": "Missing element #submit"}},
            {"status": "future-status", "pair": {"label": "unknown"}}
        ]});
        fs::write(
            run_directory.join("report.json"),
            serde_json::to_vec(&report).unwrap(),
        )
        .unwrap();
        run_directory
    }

    #[test]
    fn report_counts_keep_visual_and_engine_failures_distinct() {
        let directory = tempfile::tempdir().unwrap();
        fixture(directory.path(), "run-1");
        let catalog = Catalog::load(directory.path()).unwrap();
        assert!(catalog.warnings.is_empty());
        let report = &catalog.reports[0];
        assert_eq!(report.run_id, "run-1");
        assert_eq!(report.counts(), (1, 1, 2));
        assert_eq!(report.report.tests[2].outcome(), Outcome::Error);
        assert_eq!(report.report.tests[3].outcome(), Outcome::Unknown);
        assert!(report.html_report(directory.path()).is_ok());
    }

    #[test]
    fn malformed_reports_do_not_hide_good_reports() {
        let directory = tempfile::tempdir().unwrap();
        fixture(directory.path(), "good");
        let bad = fixture(directory.path(), "bad");
        fs::write(bad.join("report.json"), "not JSON").unwrap();
        let catalog = Catalog::load(directory.path()).unwrap();
        assert_eq!(catalog.reports.len(), 1);
        assert_eq!(catalog.warnings.len(), 1);
        assert!(
            Catalog::load(&directory.path().join("missing"))
                .unwrap()
                .reports
                .is_empty()
        );
    }

    #[test]
    fn image_paths_are_scoped_and_png_dimensions_are_checked() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("output");
        let run = fixture(&root, "run-1");
        let rgba = vec![255_u8, 0, 0, 255];
        lodepng::encode32_file(run.join("home.png"), &rgba, 1, 1).unwrap();
        fs::write(directory.path().join("outside.png"), "outside").unwrap();
        let catalog = Catalog::load(&root).unwrap();
        let report = &catalog.reports[0];
        let image = report
            .image(&root, "../bitmaps_test/run-1/home.png")
            .unwrap();
        assert_eq!((image.width, image.height), (1, 1));
        for path in [
            "../../../outside.png",
            "C:/outside.png",
            "https://example.com/image.png",
            "../missing.png",
        ] {
            assert!(report.image(&root, path).is_err(), "{path}");
        }
        fs::write(run.join("bad.png"), "not a PNG").unwrap();
        assert!(
            report
                .image(&root, "../bitmaps_test/run-1/bad.png")
                .is_err()
        );
    }
}
