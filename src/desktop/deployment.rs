use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod swa;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeployTarget {
    pub id: String,
    pub name: String,
    pub token_env: String,
    #[serde(default)]
    pub environment: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeploymentJob {
    pub target: DeployTarget,
    pub suite_ids: Vec<String>,
    pub catalog_ids: Vec<String>,
    pub partial_confirmed: bool,
}

impl DeploymentJob {
    pub fn validate(&self) -> Result<()> {
        self.target.validate()?;
        ensure!(
            !self.suite_ids.is_empty(),
            "Select at least one test suite to deploy"
        );
        let selected = self.suite_ids.iter().collect::<BTreeSet<_>>();
        let catalog = self.catalog_ids.iter().collect::<BTreeSet<_>>();
        ensure!(
            selected.len() == self.suite_ids.len()
                && catalog.len() == self.catalog_ids.len()
                && selected.is_subset(&catalog),
            "Deployment suite selection is invalid"
        );
        for suite in &self.catalog_ids {
            validate_suite_id(suite)?;
        }
        ensure!(
            selected == catalog || self.partial_confirmed,
            "Confirm that you intend to deploy only the selected test suites"
        );
        Ok(())
    }
}

impl DeployTarget {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 80
                && self
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
            "Invalid deployment target ID"
        );
        ensure!(
            !self.name.trim().is_empty()
                && self.name.len() <= 100
                && !self.name.chars().any(char::is_control),
            "Target name must contain 1-100 characters"
        );
        ensure!(
            !self.token_env.is_empty()
                && self.token_env.len() <= 128
                && self
                    .token_env
                    .bytes()
                    .enumerate()
                    .all(|(index, byte)| byte.is_ascii_alphabetic()
                        || byte == b'_'
                        || index > 0 && byte.is_ascii_digit()),
            "Token environment variable must be a valid environment variable name"
        );
        ensure!(
            self.environment.is_empty()
                || self.environment.len() <= 16
                    && self
                        .environment
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
            "Azure environment must contain at most 16 letters, digits, or hyphens; leave blank for production"
        );
        Ok(())
    }

    pub fn token(&self) -> Result<String> {
        self.validate()?;
        std::env::var(&self.token_env)
            .ok()
            .filter(|token| !token.trim().is_empty())
            .with_context(|| {
                format!(
                    "Deployment token environment variable '{}' is not set",
                    self.token_env
                )
            })
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeployTargets {
    #[serde(default)]
    pub targets: Vec<DeployTarget>,
    #[serde(default)]
    pub selected: Option<String>,
}

impl DeployTargets {
    pub fn path(root: &Path) -> Result<PathBuf> {
        Self::path_in(root, &super::workspace::Preferences::path()?)
    }

    pub fn path_in(root: &Path, preferences: &Path) -> Result<PathBuf> {
        let root = root.canonicalize()?;
        let mut identity = super::workspace::display_path(&root);
        if cfg!(windows) {
            identity.make_ascii_lowercase();
        }
        let key = format!("{:x}", Sha256::digest(identity.as_bytes()));
        Ok(preferences
            .parent()
            .context("Preferences directory unavailable")?
            .join("deployments")
            .join(format!("{key}.json")))
    }

    pub fn load(path: &Path) -> Result<Self> {
        match fs::File::open(path) {
            Ok(file) => {
                let bytes = bounded_read(file, 1024 * 1024, "Deployment target settings")?;
                let value: Self =
                    serde_json::from_slice(&bytes).context("Invalid deployment target settings")?;
                value.validate()?;
                Ok(value)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        fs::create_dir_all(
            path.parent()
                .context("Deployment settings directory unavailable")?,
        )?;
        crate::helpers::write_file_atomically(path, &serde_json::to_vec_pretty(self)?)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.targets.len() <= 50,
            "A workspace supports up to 50 deployment targets"
        );
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for target in &self.targets {
            target.validate()?;
            ensure!(
                ids.insert(&target.id) && names.insert(target.name.trim().to_lowercase()),
                "Deployment target names and IDs must be unique"
            );
        }
        ensure!(
            self.selected.as_ref().is_none_or(|id| ids.contains(id)),
            "Selected deployment target no longer exists"
        );
        Ok(())
    }
}

fn bounded_read(file: fs::File, limit: usize, label: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "{label} exceeds its size limit");
    Ok(bytes)
}

fn validate_suite_id(suite: &str) -> Result<()> {
    ensure!(
        !suite.is_empty()
            && suite == suite.trim()
            && suite.len() <= 200
            && Path::new(suite).components().count() == 1
            && Path::new(suite)
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            && !suite.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
            && !suite.chars().any(char::is_control)
            && !crate::helpers::is_windows_reserved_path_component(suite),
        "Invalid deployment suite ID"
    );
    Ok(())
}

fn artifact_source(
    output: &Path,
    relative: &Path,
    missing_reference: bool,
) -> Result<Option<PathBuf>> {
    ensure!(
        relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            && !relative
                .to_string_lossy()
                .contains([':', '\0', '<', '>', '"', '|', '?', '*']),
        "Report path escapes deployment output"
    );
    let mut source = output.to_path_buf();
    for component in relative.components() {
        source.push(component);
        match fs::symlink_metadata(&source) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "Deployment artifacts cannot be symlinks"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && missing_reference => {
                return Ok(None);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Deployment artifact '{}' is unavailable",
                        relative.display()
                    )
                });
            }
        }
    }
    let source = source.canonicalize()?;
    ensure!(
        source.starts_with(output) && source.is_file(),
        "Deployment artifact must be a regular file inside report output"
    );
    Ok(Some(source))
}

pub fn stage_reports(output: &Path, suite_ids: &[String], destination: &Path) -> Result<()> {
    ensure!(
        !suite_ids.is_empty(),
        "Select at least one test suite to deploy"
    );
    let output = output
        .canonicalize()
        .context("Report output directory is unavailable")?;
    ensure!(
        !destination.exists(),
        "Deployment staging directory must be new"
    );
    fs::create_dir_all(destination)?;
    let mut copied = BTreeSet::new();
    let mut copy = |relative: &Path, missing_reference: bool| -> Result<()> {
        let Some(source) = artifact_source(&output, relative, missing_reference)? else {
            return Ok(());
        };
        if !copied.insert(relative.to_path_buf()) {
            return Ok(());
        }
        let target = destination.join(relative);
        fs::create_dir_all(target.parent().context("Staging path has no parent")?)?;
        fs::copy(source, target)?;
        Ok(())
    };
    for name in [
        "index.html",
        "styles.css",
        "script.js",
        "index_bundle.js",
        "staticwebapp.config.json",
        "diff.js",
        "diverged.js",
        "divergedWorker.js",
        "lato-bold-webfont.woff",
        "lato-bold-webfont.woff2",
        "Lato-Bold.ttf",
        "lato-regular-webfont.woff",
        "lato-regular-webfont.woff2",
        "Lato-Regular.ttf",
        "favicon.ico",
        "favicon.svg",
        "favicon-16x16.png",
        "favicon-32x32.png",
        "apple-touch-icon.png",
    ] {
        if output.join(name).is_file() {
            copy(Path::new(name), false)?;
        }
    }
    ensure!(
        destination.join("index.html").is_file(),
        "Run snapshot before deploying reports"
    );
    let mut selected = BTreeSet::new();
    for suite in suite_ids {
        validate_suite_id(suite)?;
        ensure!(selected.insert(suite), "Duplicate deployment suite ID");
        let report = Path::new(suite).join("html_report");
        copy(&report.join("index.html"), false)?;
        copy(&report.join("config.js"), false)?;
        let text = String::from_utf8(bounded_read(
            fs::File::open(destination.join(&report).join("config.js"))?,
            16 * 1024 * 1024,
            "Report configuration",
        )?)?;
        let data = text
            .trim()
            .strip_prefix("report(")
            .and_then(|value| value.strip_suffix(");"))
            .context("Invalid report configuration")?;
        let data: serde_json::Value = serde_json::from_str(data)?;
        ensure!(
            data["id"].as_str() == Some(suite),
            "Report suite identity changed"
        );
        let tests = data["tests"].as_array().context("Report has no tests")?;
        for test in tests {
            ensure!(
                test["pair"]["test"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "Report entry has no current screenshot"
            );
            for key in ["reference", "test", "diffImage"] {
                if let Some(image) = test["pair"][key].as_str().filter(|value| !value.is_empty()) {
                    let image = image
                        .split('?')
                        .next()
                        .unwrap_or_default()
                        .replace('\\', "/");
                    let image = image
                        .strip_prefix("../")
                        .context("Report image must be suite-relative")?;
                    ensure!(
                        image.starts_with(if key == "reference" {
                            "bitmaps_reference/"
                        } else {
                            "bitmaps_test/"
                        }) && Path::new(image)
                            .extension()
                            .is_some_and(|extension| extension == "png"),
                        "Report image is outside capture folders"
                    );
                    let relative = Path::new(suite).join(image);
                    copy(&relative, key == "reference")?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_requires_nonempty_selection_and_explicit_subset_confirmation() {
        let mut job = DeploymentJob {
            target: DeployTarget {
                id: "qa".into(),
                name: "QA".into(),
                token_env: "QA_TOKEN".into(),
                environment: String::new(),
            },
            suite_ids: Vec::new(),
            catalog_ids: vec!["one".into(), "two".into()],
            partial_confirmed: false,
        };
        assert!(job.validate().is_err());
        job.suite_ids = vec!["one".into()];
        assert!(job.validate().is_err());
        job.partial_confirmed = true;
        assert!(job.validate().is_ok());
        job.suite_ids.push("two".into());
        job.partial_confirmed = false;
        assert!(job.validate().is_ok());
        job.suite_ids.push("outside".into());
        assert!(job.validate().is_err());
    }

    #[test]
    fn target_settings_are_workspace_scoped_and_secret_free() {
        let directory = tempfile::tempdir().unwrap();
        let target = DeployTarget {
            id: "one".into(),
            name: "QA reports".into(),
            token_env: "QA_SWA_TOKEN".into(),
            environment: String::new(),
        };
        let settings = DeployTargets {
            targets: vec![target.clone()],
            selected: Some(target.id.clone()),
        };
        let path = directory.path().join("targets.json");
        settings.save(&path).unwrap();
        assert_eq!(
            DeployTargets::load(&path).unwrap().targets,
            vec![target.clone()]
        );
        let mut invalid = settings;
        invalid.targets.push(target);
        assert!(invalid.save(&path).is_err());
        assert_ne!(
            DeployTargets::path(directory.path()).unwrap(),
            DeployTargets::path(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
        );
    }

    #[test]
    fn staging_rejects_missing_captures_and_escaping_or_nonimage_dependencies() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        fs::create_dir_all(output.join("one/html_report")).unwrap();
        fs::write(output.join("index.html"), "one").unwrap();
        fs::write(output.join("one/html_report/index.html"), "report").unwrap();
        for (index, image) in [
            "../bitmaps_test/run/missing.png",
            "../bitmaps_test/../../outside.png",
            "../bitmaps_test/run/state.json",
            "../bitmaps_test/run/image.png:stream",
        ]
        .iter()
        .enumerate()
        {
            let report = serde_json::json!({"id": "one", "tests": [{"pair": {"test": image}}]});
            fs::write(
                output.join("one/html_report/config.js"),
                format!("report({report});"),
            )
            .unwrap();
            assert!(
                stage_reports(
                    &output,
                    &["one".into()],
                    &root.path().join(format!("stage-{index}"))
                )
                .is_err()
            );
        }
    }

    #[test]
    fn staging_includes_only_selected_report_dependencies() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("output");
        fs::create_dir_all(output.join("one/html_report")).unwrap();
        fs::create_dir_all(output.join("one/bitmaps_test/run")).unwrap();
        fs::create_dir_all(output.join("two")).unwrap();
        fs::create_dir_all(output.join("states")).unwrap();
        fs::write(output.join("index.html"), "one").unwrap();
        fs::write(output.join("one/html_report/index.html"), "report").unwrap();
        fs::write(output.join("one/html_report/config.js"), r#"report({"id":"one","tests":[{"pair":{"test":"../bitmaps_test/run/image.png?v=hash"}}]});"#).unwrap();
        fs::write(output.join("one/bitmaps_test/run/image.png"), b"png").unwrap();
        fs::write(
            output.join("one/bitmaps_test/run/private.json"),
            b"not published",
        )
        .unwrap();
        let destination = directory.path().join("publish");
        stage_reports(&output, &["one".into()], &destination).unwrap();
        assert!(destination.join("one/bitmaps_test/run/image.png").exists());
        assert!(
            !destination
                .join("one/bitmaps_test/run/private.json")
                .exists()
        );
        assert!(!destination.join("two").exists());
        assert!(!destination.join("states").exists());
        assert!(stage_reports(&output, &[], &directory.path().join("empty")).is_err());
        assert!(
            stage_reports(
                &output,
                &["../outside".into()],
                &directory.path().join("escape")
            )
            .is_err()
        );
    }
}
