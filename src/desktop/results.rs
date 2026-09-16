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
    pub tests: Vec<TestResult>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TestResult {
    pub status: String,
    pub pair: Pair,
}

impl TestResult {
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
    pub reference: Option<String>,
    pub test: Option<String>,
    pub diff_image: Option<String>,
    pub engine_error_msg: Option<String>,
    pub reference_url: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub selector: String,
    pub diff: Option<Difference>,
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
}

impl ReportEntry {
    pub fn label(&self) -> String {
        format!("{} / {}", self.report.id, self.run_id)
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

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub reports: Vec<ReportEntry>,
    pub warnings: Vec<String>,
}

impl Catalog {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
