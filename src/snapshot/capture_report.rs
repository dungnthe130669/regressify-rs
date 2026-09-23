use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use super::site::{
    Capture, Dimensions, MAX_IMAGE_BYTES, MAX_REPORT_BYTES, SuiteReport, checked_file,
    read_bounded, store_asset, valid_segment,
};

#[derive(Deserialize)]
struct RawReport {
    id: String,
    tests: Vec<RawTest>,
}

#[derive(Deserialize)]
struct RawTest {
    status: String,
    pair: RawPair,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPair {
    #[serde(default)]
    label: String,
    #[serde(default)]
    scenario_id: String,
    #[serde(default)]
    scenario_label: String,
    #[serde(default)]
    selector: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    mis_match_threshold: f64,
    reference: Option<String>,
    test: Option<String>,
    diff_image: Option<String>,
    reference_url: Option<String>,
    url: Option<String>,
    engine_error_msg: Option<String>,
    diff: Option<RawDiff>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDiff {
    raw_mis_match_percentage: Option<f64>,
    mis_match_percentage: Option<String>,
    is_same_dimensions: bool,
    dimension_difference: RawDimensions,
}

#[derive(Deserialize)]
struct RawDimensions {
    width: i64,
    height: i64,
}

fn normalize_image(path: &str) -> String {
    path.split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path)
        .replace('\\', "/")
}

fn capture_paths(report: &RawReport) -> Result<Vec<String>> {
    report
        .tests
        .iter()
        .map(|test| {
            let path = normalize_image(
                test.pair
                    .test
                    .as_deref()
                    .context("Snapshot capture has no test image path")?,
            );
            let rest = path
                .strip_prefix("../bitmaps_test/")
                .context("Snapshot test image must be inside bitmaps_test")?;
            let components: Vec<_> = rest.split('/').collect();
            ensure!(
                components.len() == 2 && components.iter().all(|part| valid_segment(part)),
                "Invalid snapshot capture run path"
            );
            Ok(path)
        })
        .collect()
}

fn image_asset(
    root: &Path,
    output: &Path,
    suite: &str,
    path: Option<&str>,
    role: &str,
    images: &mut BTreeMap<PathBuf, String>,
) -> Result<Option<String>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let normalized = normalize_image(path);
    let reference = role == "reference";
    let prefix = if reference {
        "../bitmaps_reference/"
    } else {
        "../bitmaps_test/"
    };
    let relative = normalized
        .strip_prefix(prefix)
        .context("Snapshot image is outside its capture folder")?;
    ensure!(
        !relative.is_empty()
            && relative.split('/').all(valid_segment)
            && relative.to_ascii_lowercase().ends_with(".png"),
        "Invalid snapshot image path"
    );
    let folder = if reference {
        "bitmaps_reference"
    } else {
        "bitmaps_test"
    };
    let Some(source) = checked_file(
        root,
        &Path::new(suite).join(folder).join(relative),
        reference,
    )?
    else {
        return Ok(None);
    };
    if let Some(asset) = images.get(&source) {
        return Ok(Some(asset.clone()));
    }
    let bytes = read_bounded(&source, MAX_IMAGE_BYTES)?;
    ensure!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Snapshot capture is not a PNG image: {}",
        source.display()
    );
    let name = if reference {
        relative
    } else {
        relative
            .split_once('/')
            .context("Snapshot test image has no capture run")?
            .1
    };
    let asset = store_asset(
        output,
        &format!("_snapshot/images/{suite}/{role}/{name}"),
        &bytes,
    )?;
    images.insert(source, asset.clone());
    Ok(Some(asset))
}

pub(super) fn load_suite(
    root: &Path,
    output: &Path,
    id: &str,
    images: &mut BTreeMap<PathBuf, String>,
) -> Result<Option<SuiteReport>> {
    ensure!(
        valid_segment(id) && id != "_snapshot",
        "Invalid snapshot suite ID"
    );
    let Some(config) = checked_file(root, &Path::new(id).join("html_report/config.js"), true)?
    else {
        return Ok(None);
    };
    let config_text = String::from_utf8(read_bounded(&config, MAX_REPORT_BYTES)?)?;
    let json = config_text
        .trim()
        .strip_prefix("report(")
        .and_then(|value| value.strip_suffix(");"))
        .context("Invalid snapshot config.js wrapper; expected report(<JSON>);")?;
    let config_report: RawReport =
        serde_json::from_str(json).context("Invalid snapshot config.js report data")?;
    ensure!(
        config_report.id == id,
        "Snapshot config suite ID does not match its output directory"
    );
    let paths = capture_paths(&config_report)?;
    let runs: BTreeSet<_> = paths
        .iter()
        .filter_map(|path| {
            path.strip_prefix("../bitmaps_test/")
                .and_then(|path| path.split('/').next())
        })
        .collect();
    ensure!(
        runs.len() <= 1,
        "Snapshot config references multiple capture runs"
    );
    let run_id = runs.first().copied().unwrap_or_default().to_owned();
    let report = if run_id.is_empty() {
        config_report
    } else {
        let path = checked_file(
            root,
            &Path::new(id)
                .join("bitmaps_test")
                .join(&run_id)
                .join("report.json"),
            false,
        )?
        .context("Capture run report is missing")?;
        let report: RawReport = serde_json::from_slice(&read_bounded(&path, MAX_REPORT_BYTES)?)
            .context("Snapshot capture run has invalid report.json")?;
        ensure!(
            report.id == id && capture_paths(&report)? == paths,
            "Snapshot config and capture run report disagree"
        );
        report
    };
    let mut tests = Vec::with_capacity(report.tests.len());
    let mut identifiers = BTreeSet::new();
    for test in report.tests {
        let pair = test.pair;
        ensure!(
            matches!(test.status.as_str(), "pass" | "fail")
                && (pair.engine_error_msg.is_none() || test.status == "fail"),
            "Invalid snapshot comparison status"
        );
        ensure!(
            (0.0..=100.0).contains(&pair.mis_match_threshold),
            "Invalid snapshot comparison threshold"
        );
        let capture_path = normalize_image(
            pair.test
                .as_deref()
                .context("Snapshot capture has no test image")?,
        );
        let capture_id = if pair.file_name.is_empty() {
            capture_path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned()
        } else {
            pair.file_name
        };
        ensure!(
            valid_segment(&capture_id) && identifiers.insert(capture_id.clone()),
            "Invalid or duplicate snapshot capture ID"
        );
        let mismatch = pair
            .diff
            .as_ref()
            .map(|diff| {
                let value = diff.raw_mis_match_percentage.map(Ok).unwrap_or_else(|| {
                    diff.mis_match_percentage
                        .as_deref()
                        .context("Comparison diff has no mismatch percentage")?
                        .parse::<f64>()
                        .context("Invalid mismatch percentage")
                })?;
                ensure!(
                    (0.0..=100.0).contains(&value),
                    "Invalid snapshot mismatch percentage"
                );
                Ok::<_, anyhow::Error>(value)
            })
            .transpose()?;
        let reference = image_asset(
            root,
            output,
            id,
            pair.reference.as_deref(),
            "reference",
            images,
        )?;
        let current = image_asset(root, output, id, pair.test.as_deref(), "test", images)?;
        let diff = image_asset(root, output, id, pair.diff_image.as_deref(), "diff", images)?;
        let label = if pair.label.is_empty() {
            capture_id.clone()
        } else {
            pair.label
        };
        tests.push(Capture {
            id: capture_id,
            label,
            scenario: if pair.scenario_label.is_empty() {
                pair.scenario_id
            } else {
                pair.scenario_label
            },
            selector: pair.selector,
            status: test.status,
            threshold: pair.mis_match_threshold,
            mismatch,
            error: pair.engine_error_msg,
            reference,
            test: current,
            diff,
            reference_url: pair.reference_url,
            test_url: pair.url,
            dimensions: pair.diff.map(|diff| Dimensions {
                same: diff.is_same_dimensions,
                width: diff.dimension_difference.width,
                height: diff.dimension_difference.height,
            }),
        });
    }
    Ok(Some(SuiteReport {
        id: id.into(),
        run_id,
        tests,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, serde_json::Value) {
        let temp = tempfile::tempdir().unwrap();
        let suite = temp.path().join("suite");
        for path in [
            "html_report",
            "bitmaps_test/current",
            "bitmaps_test/prior",
            "bitmaps_reference",
        ] {
            fs::create_dir_all(suite.join(path)).unwrap();
        }
        for path in [
            "bitmaps_test/current/home.png",
            "bitmaps_reference/home.png",
        ] {
            fs::write(suite.join(path), b"\x89PNG\r\n\x1a\npixels").unwrap();
        }
        let report = json!({ "id": "suite", "tests": [{ "status": "fail", "pair": {
            "label": "Home <desktop>", "scenarioLabel": "Home", "selector": "document", "fileName": "home.png",
            "reference": "../bitmaps_reference/home.png", "test": "../bitmaps_test/current/home.png", "misMatchThreshold": 0.1,
            "referenceUrl": "https://example.com/ref", "url": "https://example.com/test",
            "diff": { "isSameDimensions": false, "dimensionDifference": { "width": -20, "height": 40 }, "rawMisMatchPercentage": 1.25 }
        }}] });
        write_fixture(temp.path(), &report);
        (temp, report)
    }

    fn write_fixture(root: &Path, report: &serde_json::Value) {
        fs::write(
            root.join("suite/html_report/config.js"),
            format!("report({report});"),
        )
        .unwrap();
        fs::write(
            root.join("suite/bitmaps_test/current/report.json"),
            report.to_string(),
        )
        .unwrap();
    }

    fn load(root: &Path) -> Result<Option<SuiteReport>> {
        load_suite(
            &root.canonicalize()?,
            &root.canonicalize()?,
            "suite",
            &mut BTreeMap::new(),
        )
    }

    #[test]
    fn converts_comparison_metadata_and_keeps_sources_and_prior_runs() {
        let (temp, _) = fixture();
        let config = fs::read(temp.path().join("suite/html_report/config.js")).unwrap();
        let report = load(temp.path()).unwrap().unwrap();
        assert_eq!(report.run_id, "current");
        assert_eq!(report.tests[0].label, "Home <desktop>");
        assert_eq!(report.tests[0].dimensions.as_ref().unwrap().width, -20);
        assert_eq!(report.tests[0].mismatch, Some(1.25));
        assert_ne!(report.tests[0].reference, report.tests[0].test);
        assert_eq!(
            report.tests[0]
                .reference
                .as_ref()
                .unwrap()
                .split_once("?v=")
                .unwrap()
                .1,
            report.tests[0]
                .test
                .as_ref()
                .unwrap()
                .split_once("?v=")
                .unwrap()
                .1
        );
        assert!(
            report.tests[0]
                .test
                .as_ref()
                .unwrap()
                .starts_with("_snapshot/images/")
        );
        assert!(temp.path().join("suite/bitmaps_test/prior").is_dir());
        assert_eq!(
            fs::read(temp.path().join("suite/html_report/config.js")).unwrap(),
            config
        );
        assert_eq!(
            serde_json::to_value(load(temp.path()).unwrap().unwrap()).unwrap(),
            serde_json::to_value(report).unwrap()
        );
    }

    #[test]
    fn invalid_later_suite_preserves_the_previous_versioned_site() {
        let (temp, _) = fixture();
        let root = temp.path();
        let args = crate::cli::args::SnapshotArgs {
            test_suite: Vec::new(),
            title: None,
            heading: None,
            subheading: None,
        };
        let config = crate::config::WorkspaceConfig::default();
        super::super::site::generate_site(root, &["suite".into()], &args, &config).unwrap();
        let previous = super::super::site::artifact_paths(root, &["suite".into()])
            .unwrap()
            .into_iter()
            .map(|path| {
                let bytes = fs::read(root.join(&path)).unwrap();
                (path, bytes)
            })
            .collect::<BTreeMap<_, _>>();
        fs::write(
            root.join("suite/bitmaps_test/current/home.png"),
            b"\x89PNG\r\n\x1a\nchanged image",
        )
        .unwrap();
        fs::create_dir_all(root.join("z-bad/html_report")).unwrap();
        fs::write(root.join("z-bad/html_report/config.js"), "not a report").unwrap();
        assert!(
            super::super::site::generate_site(
                root,
                &["suite".into(), "z-bad".into()],
                &args,
                &config
            )
            .is_err()
        );
        for (path, bytes) in previous {
            assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
        }
        super::super::site::artifact_paths(root, &["suite".into()]).unwrap();
    }

    #[test]
    fn regeneration_keeps_file_list_stable_and_updates_all_dependent_versions() {
        let (temp, _) = fixture();
        let root = temp.path();
        let args = crate::cli::args::SnapshotArgs {
            test_suite: Vec::new(),
            title: None,
            heading: None,
            subheading: None,
        };
        let config = crate::config::WorkspaceConfig::default();
        let generate =
            || super::super::site::generate_site(root, &["suite".into()], &args, &config).unwrap();
        let read = |path: &str| {
            serde_json::from_slice::<serde_json::Value>(&fs::read(root.join(path)).unwrap())
                .unwrap()
        };
        generate();
        let first_paths = super::super::site::artifact_paths(root, &["suite".into()]).unwrap();
        let first_index = fs::read(root.join("index.html")).unwrap();
        let first_manifest = read("_snapshot/data/manifest.json");
        let first_report = read("_snapshot/data/suites/suite.json");
        fs::write(
            root.join("suite/bitmaps_test/current/home.png"),
            b"\x89PNG\r\n\x1a\nupdated pixels",
        )
        .unwrap();
        fs::write(
            root.join("_snapshot/images/old-content-hash.png"),
            b"obsolete",
        )
        .unwrap();
        generate();
        assert_eq!(
            super::super::site::artifact_paths(root, &["suite".into()]).unwrap(),
            first_paths
        );
        assert!(!root.join("_snapshot/images/old-content-hash.png").exists());
        assert_ne!(fs::read(root.join("index.html")).unwrap(), first_index);
        let next_manifest = read("_snapshot/data/manifest.json");
        let next_report = read("_snapshot/data/suites/suite.json");
        assert_ne!(
            first_manifest["suites"][0]["report"],
            next_manifest["suites"][0]["report"]
        );
        assert_eq!(
            first_report["tests"][0]["reference"],
            next_report["tests"][0]["reference"]
        );
        let old_test = first_report["tests"][0]["test"]
            .as_str()
            .unwrap()
            .split_once("?v=")
            .unwrap();
        let new_test = next_report["tests"][0]["test"]
            .as_str()
            .unwrap()
            .split_once("?v=")
            .unwrap();
        assert_eq!(old_test.0, new_test.0);
        assert_ne!(old_test.1, new_test.1);
        assert_eq!(
            new_test.1,
            super::super::get_asset_version(b"\x89PNG\r\n\x1a\nupdated pixels", 20)
        );
        assert!(root.join("suite/bitmaps_test/prior").exists());
        assert!(fs::read_dir(root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".snapshot-")
        }));
    }

    #[test]
    fn malformed_missing_and_multi_run_reports_are_errors_without_capture_deletion() {
        let (temp, mut report) = fixture();
        fs::write(
            temp.path().join("suite/bitmaps_test/current/report.json"),
            "not json",
        )
        .unwrap();
        assert!(
            load(temp.path())
                .unwrap_err()
                .to_string()
                .contains("invalid report.json")
        );
        fs::write(
            temp.path().join("suite/html_report/config.js"),
            "report(alert(1));",
        )
        .unwrap();
        assert!(load(temp.path()).is_err());
        report["tests"][0]["pair"]["test"] = json!("../bitmaps_test/missing/home.png");
        write_fixture(temp.path(), &report);
        assert!(load(temp.path()).is_err());
        let mut other = report["tests"][0].clone();
        other["pair"]["test"] = json!("../bitmaps_test/current/home.png");
        report["tests"].as_array_mut().unwrap().push(other);
        write_fixture(temp.path(), &report);
        assert!(
            load(temp.path())
                .unwrap_err()
                .to_string()
                .contains("multiple capture runs")
        );
        assert!(temp.path().join("suite/bitmaps_test/prior").is_dir());
        assert!(temp.path().join("suite/bitmaps_test/current").is_dir());
    }

    #[test]
    fn missing_reference_is_reportable_but_missing_test_and_path_escapes_fail() {
        let (temp, mut report) = fixture();
        fs::remove_file(temp.path().join("suite/bitmaps_reference/home.png")).unwrap();
        assert!(
            load(temp.path()).unwrap().unwrap().tests[0]
                .reference
                .is_none()
        );
        for path in [
            "../../private.png",
            "../bitmaps_reference/../secret.png",
            "https://example.com/a.png",
            "../bitmaps_reference/CON.png",
        ] {
            report["tests"][0]["pair"]["reference"] = json!(path);
            write_fixture(temp.path(), &report);
            assert!(load(temp.path()).is_err(), "{path}");
        }
        report["tests"][0]["pair"]["reference"] = serde_json::Value::Null;
        write_fixture(temp.path(), &report);
        fs::remove_file(temp.path().join("suite/bitmaps_test/current/home.png")).unwrap();
        assert!(load(temp.path()).is_err());
    }

    #[test]
    fn accepts_empty_suites_and_versioned_windows_paths_and_rejects_invalid_results() {
        let (temp, mut report) = fixture();
        report["tests"][0]["pair"]["test"] = json!("..\\bitmaps_test\\current\\home.png?v=old");
        write_fixture(temp.path(), &report);
        assert!(load(temp.path()).is_ok());
        report["tests"][0]["pair"]["misMatchThreshold"] = json!(101);
        write_fixture(temp.path(), &report);
        assert!(load(temp.path()).is_err());
        report["tests"][0]["pair"]["misMatchThreshold"] = json!(0.1);
        report["tests"][0]["status"] = json!("unknown");
        write_fixture(temp.path(), &report);
        assert!(load(temp.path()).is_err());
        report["tests"] = json!([]);
        write_fixture(temp.path(), &report);
        assert!(load(temp.path()).unwrap().unwrap().tests.is_empty());
    }
}
