use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod budget;
pub mod cloudflare;
pub mod credentials;
mod diagnostics;
pub mod netlify;
mod package;
pub mod swa;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum DeploymentProvider {
    #[default]
    AzureSwa,
    Netlify,
    CloudflareWorkers,
}

impl DeploymentProvider {
    pub fn label(self) -> &'static str {
        match self {
            Self::AzureSwa => "Azure Static Web Apps",
            Self::Netlify => "Netlify",
            Self::CloudflareWorkers => "Cloudflare Workers",
        }
    }

    pub fn validate_token(self, token: &str) -> Result<()> {
        match self {
            Self::AzureSwa => swa::validate_token(token),
            Self::CloudflareWorkers => cloudflare::validate_token(token),
            Self::Netlify => {
                ensure!(
                    !token.is_empty()
                        && token.len() <= 1024
                        && token.bytes().all(|byte| byte.is_ascii_graphic()),
                    "Enter a valid Netlify personal access token"
                );
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeployTarget {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub provider: DeploymentProvider,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub site_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudflare: Option<cloudflare::CloudflareTarget>,
}

impl<'de> Deserialize<'de> for DeployTarget {
    fn deserialize<Deserializer>(deserializer: Deserializer) -> Result<Self, Deserializer::Error>
    where
        Deserializer: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Fields {
            id: String,
            name: String,
            #[serde(default)]
            credential_id: Option<String>,
            #[serde(default)]
            environment: String,
            #[serde(default)]
            provider: DeploymentProvider,
            #[serde(default)]
            site_id: String,
            #[serde(default)]
            cloudflare: Option<cloudflare::CloudflareTarget>,
            #[serde(default, rename = "tokenEnv")]
            _legacy_token_env: Option<String>,
        }
        let fields = Fields::deserialize(deserializer)?;
        Ok(Self {
            id: fields.id,
            name: fields.name,
            credential_id: fields.credential_id,
            environment: fields.environment,
            provider: fields.provider,
            site_id: fields.site_id,
            cloudflare: fields.cloudflare,
        })
    }
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
            self.credential_id
                .as_ref()
                .is_none_or(|id| uuid::Uuid::parse_str(id).is_ok()),
            "Invalid deployment credential ID"
        );
        ensure!(
            self.provider == DeploymentProvider::CloudflareWorkers || self.cloudflare.is_none(),
            "Only Cloudflare targets use Cloudflare settings"
        );
        match self.provider {
            DeploymentProvider::AzureSwa => {
                ensure!(
                    self.site_id.is_empty(),
                    "Azure targets do not use a Netlify Site ID"
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
            }
            DeploymentProvider::Netlify => {
                ensure!(
                    self.environment.is_empty(),
                    "Netlify targets do not use an Azure environment"
                );
                ensure!(
                    self.site_id.len() == 36 && uuid::Uuid::parse_str(&self.site_id).is_ok(),
                    "Enter the Netlify Site ID (Project ID) from project configuration"
                );
            }
            DeploymentProvider::CloudflareWorkers => {
                ensure!(
                    self.environment.is_empty() && self.site_id.is_empty(),
                    "Cloudflare targets do not use an Azure environment or Netlify Site ID"
                );
                self.cloudflare
                    .as_ref()
                    .context("Enter the Cloudflare Account ID and Worker name")?
                    .validate()?;
            }
        }
        Ok(())
    }

    pub fn token(&self, root: &Path) -> Result<zeroize::Zeroizing<String>> {
        self.validate()?;
        let id = self
            .credential_id
            .as_deref()
            .context("No saved deployment token. Edit the target and enter its deployment token")?;
        credentials::DeploymentCredential::new(root, id)?.read()
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
        let key = workspace_key(root)?;
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

    pub fn save_target(
        &mut self,
        root: &Path,
        path: &Path,
        mut target: DeployTarget,
        token: &str,
    ) -> Result<()> {
        let token = token.trim();
        let existing = self
            .targets
            .iter()
            .find(|previous| previous.id == target.id);
        let existing_id = existing.and_then(|previous| previous.credential_id.clone());
        let provider_changed =
            existing.is_some_and(|previous| previous.provider != target.provider);
        ensure!(
            !token.is_empty() || !provider_changed,
            "Enter a new deployment token when changing providers"
        );
        ensure!(
            !token.is_empty() || existing_id.is_some(),
            "Enter a deployment token"
        );
        if !token.is_empty() {
            target.provider.validate_token(token)?;
        }
        let credential_id = existing_id
            .clone()
            .filter(|_| !provider_changed)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        target.credential_id = Some(credential_id.clone());
        let mut next = self.clone();
        next.targets.retain(|previous| previous.id != target.id);
        next.selected = Some(target.id.clone());
        next.targets.push(target);
        next.validate()?;
        let credential = credentials::DeploymentCredential::new(root, &credential_id)?;
        let previous = if existing_id.is_some() && !provider_changed {
            credential.read_optional()?
        } else {
            None
        };
        let retired = existing_id
            .as_deref()
            .filter(|_| provider_changed)
            .map(|id| credentials::DeploymentCredential::new(root, id))
            .transpose()?;
        let retired_token = retired
            .as_ref()
            .map(|credential| credential.read_optional())
            .transpose()?
            .flatten();
        ensure!(
            !token.is_empty() || previous.is_some(),
            "No saved deployment token. Enter the deployment token again"
        );
        if !token.is_empty() {
            credential.write(token)?;
        }
        let saved = (|| -> Result<()> {
            if let Some(retired) = &retired {
                retired.delete()?;
            }
            next.save(path)
        })();
        if let Err(error) = saved {
            if let (Some(retired), Some(retired_token)) = (&retired, &retired_token) {
                retired
                    .write(retired_token)
                    .context("Cannot restore the previous provider's credential")?;
            }
            if !token.is_empty() {
                match previous.as_deref() {
                    Some(previous) => credential.write(previous),
                    None => credential.delete(),
                }
                .context(
                    "Cannot restore the secure credential after saving target settings failed",
                )?;
            }
            return Err(error).context("Cannot save deployment target settings");
        }
        *self = next;
        Ok(())
    }

    pub fn remove_target(&mut self, root: &Path, path: &Path, id: &str) -> Result<()> {
        let target = self
            .targets
            .iter()
            .find(|target| target.id == id)
            .context("Deployment target no longer exists")?;
        let credential = target
            .credential_id
            .as_deref()
            .map(|id| credentials::DeploymentCredential::new(root, id))
            .transpose()?;
        let previous = credential
            .as_ref()
            .map(|credential| credential.read_optional())
            .transpose()?
            .flatten();
        let mut next = self.clone();
        next.targets.retain(|target| target.id != id);
        if next.selected.as_deref() == Some(id) {
            next.selected = next.targets.first().map(|target| target.id.clone());
        }
        next.validate()?;
        if let Some(credential) = &credential {
            credential.delete()?;
        }
        if let Err(error) = next.save(path) {
            if let (Some(credential), Some(previous)) = (&credential, &previous) {
                credential.write(previous).context(
                    "Cannot restore the secure credential after removing target settings failed",
                )?;
            }
            return Err(error).context("Cannot remove deployment target settings");
        }
        *self = next;
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.targets.len() <= 50,
            "A workspace supports up to 50 deployment targets"
        );
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut credentials = BTreeSet::new();
        for target in &self.targets {
            target.validate()?;
            ensure!(
                ids.insert(&target.id) && names.insert(target.name.trim().to_lowercase()),
                "Deployment target names and IDs must be unique"
            );
            ensure!(
                target
                    .credential_id
                    .as_ref()
                    .is_none_or(|id| credentials.insert(id)),
                "Each deployment target must have its own saved credential"
            );
        }
        ensure!(
            self.selected.as_ref().is_none_or(|id| ids.contains(id)),
            "Selected deployment target no longer exists"
        );
        Ok(())
    }
}

pub enum DeploymentClient {
    Azure(swa::SwaClient),
    Netlify(netlify::NetlifyClient),
    Cloudflare(cloudflare::CloudflareClient),
}

impl DeploymentClient {
    pub fn new(target: &DeployTarget, token: &str) -> Result<Self> {
        target.validate()?;
        match target.provider {
            DeploymentProvider::AzureSwa => Ok(Self::Azure(swa::SwaClient::new(token)?)),
            DeploymentProvider::Netlify => Ok(Self::Netlify(netlify::NetlifyClient::new(token)?)),
            DeploymentProvider::CloudflareWorkers => {
                Ok(Self::Cloudflare(cloudflare::CloudflareClient::new(token)?))
            }
        }
    }

    pub async fn deploy(&self, root: &Path, target: &DeployTarget) -> Result<String> {
        target.validate()?;
        match self {
            Self::Azure(client) => client.deploy(root, target).await,
            Self::Netlify(client) => client.deploy(root, target).await,
            Self::Cloudflare(client) => {
                ensure!(
                    target.provider == DeploymentProvider::CloudflareWorkers,
                    "Cloudflare client requires a Cloudflare target"
                );
                client
                    .deploy(
                        root,
                        target
                            .cloudflare
                            .as_ref()
                            .context("Cloudflare target settings are missing")?,
                    )
                    .await
            }
        }
    }
}

fn validate_static_configuration(root: &Path, provider: DeploymentProvider) -> Result<()> {
    let root = root.canonicalize()?;
    let Some(path) = artifact_source(&root, Path::new("staticwebapp.config.json"), true)? else {
        return Ok(());
    };
    let config: serde_json::Value = serde_json::from_slice(&bounded_read(
        fs::File::open(path).context("Cannot read Azure static site configuration")?,
        1024 * 1024,
        "Static site configuration",
    )?)
    .map_err(|_| anyhow::anyhow!("Invalid Azure static site configuration"))?;
    let restricted = config.get("auth").is_some()
        || config.get("networking").is_some()
        || config["routes"].as_array().is_some_and(|routes| {
            routes.iter().any(|route| {
                route.get("allowedRoles").is_some_and(|roles| {
                    !roles
                        .as_array()
                        .is_some_and(|roles| roles.iter().any(|role| role == "anonymous"))
                })
            })
        });
    let protection = if provider == DeploymentProvider::CloudflareWorkers {
        "Cloudflare Access"
    } else {
        provider.label()
    };
    ensure!(
        !restricted,
        "Azure access restrictions cannot be enforced by {}; configure {protection} access protection before deploying these reports",
        provider.label()
    );
    Ok(())
}

fn phase(name: &str) {
    eprintln!("{name}");
    crate::progress::emit(crate::progress::Event::Phase { name: name.into() });
}

pub(super) async fn with_heartbeat<Work: std::future::Future>(
    label: &str,
    work: Work,
) -> Work::Output {
    monitor(work, std::time::Duration::from_secs(5), |elapsed| {
        eprintln!("{label}: still working ({}s elapsed)", elapsed.as_secs());
    })
    .await
}

async fn monitor<Work: std::future::Future>(
    work: Work,
    interval: std::time::Duration,
    mut notify: impl FnMut(std::time::Duration),
) -> Work::Output {
    let started = tokio::time::Instant::now();
    let mut timer = tokio::time::interval_at(started + interval, interval);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    tokio::pin!(work);
    loop {
        tokio::select! {
            output = &mut work => return output,
            _ = timer.tick() => notify(started.elapsed()),
        }
    }
}

pub(super) fn workspace_key(root: &Path) -> Result<String> {
    let root = root.canonicalize()?;
    let mut identity = super::workspace::display_path(&root);
    if cfg!(windows) {
        identity.make_ascii_lowercase();
    }
    Ok(format!("{:x}", Sha256::digest(identity.as_bytes())))
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

fn read_report_configuration(path: &Path) -> Result<serde_json::Value> {
    let text = String::from_utf8(bounded_read(
        fs::File::open(path)?,
        16 * 1024 * 1024,
        "Report configuration",
    )?)?;
    let data = text
        .trim()
        .strip_prefix("report(")
        .and_then(|value| value.strip_suffix(");"))
        .context("Invalid report configuration")?;
    serde_json::from_str(data).context("Invalid report configuration JSON")
}

fn report_image_relative(suite: &str, key: &str, image: &str) -> Result<PathBuf> {
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
            .is_some_and(|extension| extension == "png")
            && Path::new(image)
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "Report image is outside capture folders"
    );
    Ok(Path::new(suite).join(image))
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
    let dependencies = crate::snapshot::artifact_paths(&output, suite_ids)?;
    fs::create_dir_all(destination)?;
    phase("Staging selected report dependencies");
    let mut copied = BTreeSet::new();
    let mut copied_bytes = 0_u64;
    let mut last_update = std::time::Instant::now();
    let mut copy = |relative: &Path, missing_reference: bool| -> Result<()> {
        let Some(source) = artifact_source(&output, relative, missing_reference)? else {
            return Ok(());
        };
        if !copied.insert(relative.to_path_buf()) {
            return Ok(());
        }
        let target = destination.join(relative);
        fs::create_dir_all(target.parent().context("Staging path has no parent")?)?;
        let expected_bytes = fs::metadata(&source)?.len();
        let bytes = fs::copy(source, target)?;
        ensure!(
            bytes == expected_bytes,
            "Report changed during deployment staging"
        );
        copied_bytes += bytes;
        if copied.len() == 1
            || copied.len().is_multiple_of(50)
            || last_update.elapsed().as_secs() >= 1
        {
            phase(&format!(
                "Staged {} report files ({:.2} MiB)",
                copied.len(),
                copied_bytes as f64 / (1024.0 * 1024.0)
            ));
            last_update = std::time::Instant::now();
        }
        Ok(())
    };
    let staged = (|| -> Result<()> {
        for relative in dependencies {
            copy(&relative, false)?;
        }
        crate::snapshot::artifact_paths(destination, suite_ids)?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    phase(&format!(
        "Staging complete: {} files for {} suites",
        copied.len(),
        suite_ids.len()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn heartbeat_reports_waiting_and_preserves_results() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let mut sender = Some(sender);
        let mut updates = 0;
        let output = monitor(receiver, std::time::Duration::from_millis(1), |_| {
            updates += 1;
            if let Some(sender) = sender.take() {
                sender.send("finished").unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(output, "finished");
        assert!(updates >= 1);
        let error =
            with_heartbeat("Test deployment", async { Err::<(), _>("provider detail") }).await;
        assert_eq!(error, Err("provider detail"));
    }

    #[test]
    fn deployment_requires_nonempty_selection_and_explicit_subset_confirmation() {
        let mut job = DeploymentJob {
            target: DeployTarget {
                id: "qa".into(),
                name: "QA".into(),
                credential_id: None,
                environment: String::new(),
                provider: DeploymentProvider::AzureSwa,
                site_id: String::new(),
                cloudflare: None,
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
            credential_id: None,
            environment: String::new(),
            provider: DeploymentProvider::AzureSwa,
            site_id: String::new(),
            cloudflare: None,
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
    fn legacy_targets_keep_metadata_but_require_a_secure_token() {
        let target: DeployTarget = serde_json::from_value(serde_json::json!({
            "id": "qa", "name": "QA", "tokenEnv": "QA_SWA_TOKEN", "environment": "preview"
        }))
        .unwrap();
        assert_eq!(target.environment, "preview");
        assert_eq!(target.provider, DeploymentProvider::AzureSwa);
        assert!(target.site_id.is_empty());
        assert!(target.credential_id.is_none());
        assert!(!serde_json::to_string(&target).unwrap().contains("tokenEnv"));
        assert!(
            target
                .token(Path::new("."))
                .unwrap_err()
                .to_string()
                .contains("enter its deployment token")
        );
    }

    #[test]
    fn target_tokens_are_securely_saved_preserved_replaced_and_removed() {
        let root = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let path = config.path().join("targets.json");
        let target = DeployTarget {
            id: "qa".into(),
            name: "QA reports".into(),
            credential_id: None,
            environment: String::new(),
            provider: DeploymentProvider::AzureSwa,
            site_id: String::new(),
            cloudflare: None,
        };
        let token = format!("{}-{}000", "a".repeat(64), "b".repeat(36));
        let replacement = format!("{}-{}000", "c".repeat(64), "d".repeat(36));
        let mut settings = DeployTargets::default();
        assert!(
            settings
                .save_target(root.path(), &path, target.clone(), "")
                .is_err()
        );
        assert!(
            settings
                .save_target(root.path(), &path, target.clone(), "invalid")
                .is_err()
        );
        assert!(!path.exists());
        settings
            .save_target(root.path(), &path, target, &token)
            .unwrap();
        let saved = DeployTargets::load(&path).unwrap().targets.remove(0);
        let credential = credentials::DeploymentCredential::new(
            root.path(),
            saved.credential_id.as_deref().unwrap(),
        )
        .unwrap();
        assert_eq!(saved.token(root.path()).unwrap().as_str(), token);
        assert!(saved.token(config.path()).is_err());
        assert!(!fs::read_to_string(&path).unwrap().contains(&token));
        assert!(!format!("{saved:?}").contains(&token));
        let mut aliased = settings.clone();
        let mut duplicate = saved.clone();
        duplicate.id = "other".into();
        duplicate.name = "Other target".into();
        aliased.targets.push(duplicate);
        assert!(aliased.save(&path).is_err());
        let mut renamed = saved.clone();
        renamed.name = "Renamed QA".into();
        settings
            .save_target(root.path(), &path, renamed.clone(), "")
            .unwrap();
        assert_eq!(settings.targets[0].name, "Renamed QA");
        assert_eq!(saved.token(root.path()).unwrap().as_str(), token);
        assert!(
            settings
                .save_target(root.path(), config.path(), renamed.clone(), &replacement)
                .is_err()
        );
        assert_eq!(saved.token(root.path()).unwrap().as_str(), token);
        settings
            .save_target(root.path(), &path, renamed, &replacement)
            .unwrap();
        assert_eq!(saved.token(root.path()).unwrap().as_str(), replacement);
        assert!(
            settings
                .remove_target(root.path(), config.path(), "qa")
                .is_err()
        );
        assert_eq!(saved.token(root.path()).unwrap().as_str(), replacement);
        settings.remove_target(root.path(), &path, "qa").unwrap();
        assert!(DeployTargets::load(&path).unwrap().targets.is_empty());
        assert!(credential.read_optional().unwrap().is_none());
    }

    #[test]
    fn netlify_targets_validate_provider_fields_and_require_new_credentials_on_switch() {
        let root = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let path = config.path().join("targets.json");
        let mut target: DeployTarget = serde_json::from_value(serde_json::json!({
            "id": "qa", "name": "QA", "provider": "netlify", "siteId": uuid::Uuid::new_v4().to_string()
        })).unwrap();
        assert!(target.validate().is_ok());
        let encoded = serde_json::to_string(&target).unwrap();
        assert_eq!(
            serde_json::from_str::<DeployTarget>(&encoded).unwrap(),
            target
        );
        for invalid in ["", "../other", "https://example.com", "invalid"] {
            let mut invalid_target = target.clone();
            invalid_target.site_id = invalid.into();
            assert!(invalid_target.validate().is_err());
        }
        target.environment = "preview".into();
        assert!(target.validate().is_err());
        target.environment.clear();
        let mut settings = DeployTargets::default();
        settings
            .save_target(
                root.path(),
                &path,
                target.clone(),
                "synthetic-netlify-token",
            )
            .unwrap();
        assert_eq!(
            settings.targets[0].token(root.path()).unwrap().as_str(),
            "synthetic-netlify-token"
        );
        target.provider = DeploymentProvider::AzureSwa;
        target.site_id.clear();
        assert!(
            settings
                .save_target(root.path(), &path, target.clone(), "")
                .is_err()
        );
        assert_eq!(settings.targets[0].provider, DeploymentProvider::Netlify);
        let previous_target = settings.targets[0].clone();
        let azure_token = format!("{}-{}000", "a".repeat(64), "b".repeat(36));
        assert!(
            settings
                .save_target(root.path(), config.path(), target.clone(), &azure_token)
                .is_err()
        );
        assert_eq!(
            previous_target.token(root.path()).unwrap().as_str(),
            "synthetic-netlify-token"
        );
        settings
            .save_target(root.path(), &path, target.clone(), &azure_token)
            .unwrap();
        assert_ne!(
            previous_target.credential_id,
            settings.targets[0].credential_id
        );
        assert!(previous_target.token(root.path()).is_err());
        target.provider = DeploymentProvider::Netlify;
        target.site_id = uuid::Uuid::new_v4().to_string();
        assert!(
            settings
                .save_target(root.path(), &path, target, "")
                .is_err()
        );
        assert_eq!(
            settings.targets[0].token(root.path()).unwrap().as_str(),
            azure_token
        );
        settings.remove_target(root.path(), &path, "qa").unwrap();
        for invalid in [
            "",
            "token\nAuthorization: extra",
            "token with spaces",
            "\u{00e9}",
        ] {
            assert!(DeploymentProvider::Netlify.validate_token(invalid).is_err());
        }
        assert!(
            serde_json::from_value::<DeployTarget>(serde_json::json!({
                "id": "qa", "name": "QA", "provider": "unknown"
            }))
            .is_err()
        );
    }

    #[test]
    fn cloudflare_targets_round_trip_and_keep_credentials_on_failed_switch() -> Result<()> {
        let root = tempfile::tempdir()?;
        let config = tempfile::tempdir()?;
        let path = config.path().join("targets.json");
        let target: DeployTarget = serde_json::from_value(serde_json::json!({
            "id": "qa", "name": "Cloudflare QA", "provider": "cloudflareWorkers",
            "cloudflare": {"accountId": "0123456789abcdef0123456789abcdef", "workerName": "reports"}
        }))?;
        target.validate()?;
        assert_eq!(
            serde_json::from_str::<DeployTarget>(&serde_json::to_string(&target)?)?,
            target
        );
        let mut invalid = target.clone();
        invalid.cloudflare = None;
        assert!(invalid.validate().is_err());
        invalid = target.clone();
        invalid.site_id = uuid::Uuid::new_v4().to_string();
        assert!(invalid.validate().is_err());
        invalid = target.clone();
        invalid.environment = "preview".into();
        assert!(invalid.validate().is_err());
        invalid = target.clone();
        invalid.provider = DeploymentProvider::AzureSwa;
        assert!(invalid.validate().is_err());
        let mut settings = DeployTargets::default();
        settings.save_target(
            root.path(),
            &path,
            target.clone(),
            "synthetic-cloudflare-token",
        )?;
        let saved = settings.targets[0].clone();
        assert_eq!(
            saved.token(root.path())?.as_str(),
            "synthetic-cloudflare-token"
        );
        assert!(!fs::read_to_string(&path)?.contains("synthetic-cloudflare-token"));
        settings.save_target(root.path(), &path, target.clone(), "")?;
        assert_eq!(settings.targets[0].credential_id, saved.credential_id);
        invalid.cloudflare = None;
        assert!(
            settings
                .save_target(root.path(), &path, invalid.clone(), "")
                .is_err()
        );
        let azure_token = format!("{}-{}000", "a".repeat(64), "b".repeat(36));
        assert!(
            settings
                .save_target(root.path(), config.path(), invalid.clone(), &azure_token)
                .is_err()
        );
        assert_eq!(
            saved.token(root.path())?.as_str(),
            "synthetic-cloudflare-token"
        );
        settings.save_target(root.path(), &path, invalid, &azure_token)?;
        assert!(saved.token(root.path()).is_err());
        assert!(
            settings
                .save_target(root.path(), &path, target, "")
                .is_err()
        );
        assert_eq!(settings.targets[0].provider, DeploymentProvider::AzureSwa);
        settings.remove_target(root.path(), &path, "qa")?;
        Ok(())
    }

    fn snapshot_fixture(output: &Path) -> BTreeSet<String> {
        for path in [
            "one/html_report",
            "one/bitmaps_reference",
            "one/bitmaps_test/older",
        ] {
            fs::create_dir_all(output.join(path)).unwrap();
        }
        let mut expected: BTreeSet<String> = crate::snapshot::FRONTEND_FILES
            .iter()
            .filter(|(name, _)| *name != "gitignore")
            .map(|(name, _)| (*name).to_owned())
            .collect();
        expected.insert("index.html".into());
        for (path, pixels) in [
            ("one/bitmaps_reference/home.png", "reference"),
            ("one/bitmaps_test/older/home.png", "test"),
            ("one/bitmaps_test/older/home_diff.png", "diff"),
        ] {
            let bytes = [b"\x89PNG\r\n\x1a\n".as_slice(), pixels.as_bytes()].concat();
            fs::write(output.join(path), &bytes).unwrap();
            expected.insert(format!(
                "_snapshot/images/one/{pixels}/{}",
                Path::new(path).file_name().unwrap().to_str().unwrap()
            ));
        }
        let report = serde_json::json!({ "id": "one", "tests": [{ "status": "fail", "pair": {
            "label": "Home", "test": "../bitmaps_test/older/home.png", "reference": "../bitmaps_reference/home.png", "diffImage": "../bitmaps_test/older/home_diff.png"
        }}] });
        fs::write(
            output.join("one/html_report/config.js"),
            format!("report({report});"),
        )
        .unwrap();
        fs::write(
            output.join("one/bitmaps_test/older/report.json"),
            report.to_string(),
        )
        .unwrap();
        crate::snapshot::generate_site(
            output,
            &["one".into()],
            &crate::cli::args::SnapshotArgs {
                test_suite: Vec::new(),
                title: None,
                heading: None,
                subheading: None,
            },
            &crate::config::WorkspaceConfig::default(),
        )
        .unwrap();
        expected.extend([
            "_snapshot/data/manifest.json".into(),
            "_snapshot/data/suites/one.json".into(),
        ]);
        expected
    }

    #[test]
    fn staging_rejects_missing_images_tampering_and_wrong_suite_selection() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        let expected = snapshot_fixture(&output);
        let image = expected
            .iter()
            .find(|path| path.starts_with("_snapshot/images/"))
            .unwrap();
        let original = fs::read(output.join(image)).unwrap();
        fs::remove_file(output.join(image)).unwrap();
        assert!(stage_reports(&output, &["one".into()], &root.path().join("missing")).is_err());
        fs::write(output.join(image), b"tampered").unwrap();
        assert!(stage_reports(&output, &["one".into()], &root.path().join("changed")).is_err());
        fs::write(output.join(image), original).unwrap();
        for selection in [
            vec![],
            vec!["../outside".into()],
            vec!["two".into()],
            vec!["one".into(), "one".into()],
        ] {
            assert!(stage_reports(&output, &selection, &root.path().join("wrong")).is_err());
        }
        assert!(!root.path().join("missing").exists());
    }

    #[test]
    fn staging_includes_only_selected_report_dependencies() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("output");
        let expected = snapshot_fixture(&output);
        let skipped = [
            "Lato-Bold.ttf",
            "Lato-Regular.ttf",
            ".gitignore",
            "unused.js",
            "index_bundle.js.map",
            "old-report.html",
            "one/bitmaps_reference/unused.png",
            "one/bitmaps_test/run/unused.png",
            "one/bitmaps_test/run/unused.diff.png",
            "one/bitmaps_test/run/private.json",
            "one/bitmaps_test/older/unused.png",
            "one/bitmaps_test/older/unused.diff.png",
            "one/bitmaps_test/oldest/unused.png",
            "one/html_report/unused.js",
            "two/html_report/index.html",
            "two/bitmaps_test/run/image.png",
            "states/private.json",
        ];
        for relative in skipped {
            let path = output.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"fixture").unwrap();
        }
        let destination = directory.path().join("publish");
        stage_reports(&output, &["one".into()], &destination).unwrap();
        for relative in skipped {
            assert!(!destination.join(relative).exists(), "{relative}");
            assert_eq!(fs::read(output.join(relative)).unwrap(), b"fixture");
        }
        let package = package::Package::create(&destination, expected.len() as u64).unwrap();
        let archive = zip::ZipArchive::new(package.file.reopen().unwrap()).unwrap();
        assert_eq!(
            archive
                .file_names()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>(),
            expected
        );
        for relative in &expected {
            assert_eq!(
                fs::read(destination.join(relative)).unwrap(),
                fs::read(output.join(relative)).unwrap()
            );
        }
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
