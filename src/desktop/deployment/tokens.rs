use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{DeploymentProvider, credentials::DeploymentCredential};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedToken {
    pub id: String,
    pub name: String,
    pub provider: DeploymentProvider,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub account_id: String,
}

impl NamedToken {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            uuid::Uuid::parse_str(&self.id).is_ok(),
            "Invalid named deployment token ID"
        );
        ensure!(
            !self.name.trim().is_empty()
                && self.name.len() <= 100
                && !self.name.chars().any(char::is_control),
            "Token name must contain 1-100 characters"
        );
        match self.provider {
            DeploymentProvider::Netlify => ensure!(
                self.account_id.is_empty(),
                "Netlify tokens do not use an Account ID"
            ),
            DeploymentProvider::CloudflareWorkers => ensure!(
                self.account_id.len() == 32
                    && self.account_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "Enter a valid Cloudflare Account ID (32 hexadecimal characters)"
            ),
            DeploymentProvider::AzureSwa => {
                anyhow::bail!("Azure deployment tokens belong to individual targets")
            }
        }
        Ok(())
    }

    pub fn token(&self) -> Result<Zeroizing<String>> {
        self.validate()?;
        let token = DeploymentCredential::named(&self.id)?.read()?;
        self.provider.validate_token(&token)?;
        Ok(token)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenCatalog {
    pub tokens: Vec<NamedToken>,
}

impl TokenCatalog {
    pub fn path() -> Result<PathBuf> {
        Self::path_in(&crate::desktop::workspace::Preferences::path()?)
    }

    pub fn path_in(preferences: &Path) -> Result<PathBuf> {
        Ok(preferences
            .parent()
            .context("Desktop settings directory unavailable")?
            .join("deployment-tokens.json"))
    }

    pub fn load(path: &Path) -> Result<Self> {
        match fs::File::open(path) {
            Ok(file) => {
                let bytes = super::bounded_read(file, 1024 * 1024, "Deployment token settings")?;
                let catalog: Self =
                    serde_json::from_slice(&bytes).context("Invalid deployment token settings")?;
                catalog.validate()?;
                Ok(catalog)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn find(&self, id: &str) -> Result<&NamedToken> {
        self.tokens.iter().find(|token| token.id == id).context(
            "Named deployment token no longer exists. Choose another token in deployment settings",
        )
    }

    pub fn save_token(&mut self, path: &Path, token: NamedToken, secret: &str) -> Result<()> {
        token.validate()?;
        let _lock = self.lock_current(path)?;
        let existing = self.tokens.iter().find(|entry| entry.id == token.id);
        ensure!(
            existing
                .is_none_or(|entry| entry.provider == token.provider
                    && entry.account_id == token.account_id),
            "Create a new named token to change its provider or Cloudflare account"
        );
        let secret = secret.trim();
        if !secret.is_empty() {
            token.provider.validate_token(secret)?;
        }
        let credential = DeploymentCredential::named(&token.id)?;
        let previous = credential.read_optional()?;
        ensure!(
            !secret.is_empty() || existing.is_some() && previous.is_some(),
            "Enter a deployment token"
        );
        let mut next = self.clone();
        next.tokens.retain(|entry| entry.id != token.id);
        next.tokens.push(token);
        next.validate()?;
        if !secret.is_empty() {
            credential.write(secret)?;
        }
        if let Err(error) = next.write(path) {
            if !secret.is_empty() {
                match previous.as_deref() {
                    Some(previous) => credential.write(previous),
                    None => credential.delete(),
                }
                .context("Cannot restore the secure token after saving its settings failed")?;
            }
            return Err(error).context("Cannot save named deployment token settings");
        }
        *self = next;
        Ok(())
    }

    pub fn remove(&mut self, path: &Path, id: &str) -> Result<()> {
        let _lock = self.lock_current(path)?;
        self.find(id)?;
        let credential = DeploymentCredential::named(id)?;
        let previous = credential.read_optional()?;
        let mut next = self.clone();
        next.tokens.retain(|entry| entry.id != id);
        credential.delete()?;
        if let Err(error) = next.write(path) {
            if let Some(previous) = previous {
                credential.write(&previous).context(
                    "Cannot restore the secure token after removing its settings failed",
                )?;
            }
            return Err(error).context("Cannot remove named deployment token settings");
        }
        *self = next;
        Ok(())
    }

    fn lock_current(&self, path: &Path) -> Result<fs::File> {
        fs::create_dir_all(
            path.parent()
                .context("Deployment token settings directory unavailable")?,
        )?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.lock()
            .context("Cannot lock shared deployment token settings")?;
        ensure!(
            Self::load(path)? == *self,
            "Deployment tokens changed in another window. Close and reopen token management before saving"
        );
        Ok(lock)
    }

    fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        crate::helpers::write_file_atomically(path, &serde_json::to_vec_pretty(self)?)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.tokens.len() <= 200,
            "Up to 200 named deployment tokens are supported"
        );
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for token in &self.tokens {
            token.validate()?;
            ensure!(
                ids.insert(uuid::Uuid::parse_str(&token.id)?)
                    && names.insert(token.name.trim().to_lowercase()),
                "Deployment token names and IDs must be unique"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(name: &str) -> NamedToken {
        NamedToken {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            provider: DeploymentProvider::Netlify,
            account_id: String::new(),
        }
    }

    #[test]
    fn named_tokens_create_rename_rotate_and_remove_without_serializing_secrets() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tokens.json");
        let mut catalog = TokenCatalog::load(&path).unwrap();
        let mut entry = token("Customer");
        assert!(catalog.save_token(&path, entry.clone(), " ").is_err());
        catalog
            .save_token(&path, entry.clone(), "synthetic-original-secret")
            .unwrap();
        entry.name = "Renamed".into();
        catalog.save_token(&path, entry.clone(), "").unwrap();
        assert_eq!(entry.token().unwrap().as_str(), "synthetic-original-secret");
        catalog
            .save_token(&path, entry.clone(), "synthetic-rotated-secret")
            .unwrap();
        assert_eq!(entry.token().unwrap().as_str(), "synthetic-rotated-secret");
        assert_eq!(TokenCatalog::load(&path).unwrap(), catalog);
        assert!(!fs::read_to_string(&path).unwrap().contains("secret"));
        let mut changed = entry.clone();
        changed.provider = DeploymentProvider::CloudflareWorkers;
        changed.account_id = "a".repeat(32);
        assert!(
            catalog
                .save_token(&path, changed, "synthetic-replacement")
                .is_err()
        );
        assert_eq!(entry.token().unwrap().as_str(), "synthetic-rotated-secret");
        catalog.remove(&path, &entry.id).unwrap();
        assert!(entry.token().is_err());
        assert!(TokenCatalog::load(&path).unwrap().tokens.is_empty());
    }

    #[test]
    fn stale_token_editors_cannot_overwrite_another_windows_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tokens.json");
        let mut first = TokenCatalog::default();
        let mut stale = first.clone();
        let entry = token("First");
        first
            .save_token(&path, entry.clone(), "synthetic-first")
            .unwrap();
        assert!(
            stale
                .save_token(&path, token("Second"), "synthetic-second")
                .is_err()
        );
        assert!(stale.remove(&path, &entry.id).is_err());
        assert_eq!(TokenCatalog::load(&path).unwrap(), first);
        assert_eq!(entry.token().unwrap().as_str(), "synthetic-first");
    }

    #[test]
    fn invalid_token_metadata_and_corrupt_catalogs_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tokens.json");
        let mut catalog = TokenCatalog::default();
        let entry = token("Customer");
        catalog
            .save_token(&path, entry, "synthetic-secret")
            .unwrap();
        assert!(
            catalog
                .save_token(&path, token(" CUSTOMER "), "synthetic-secret")
                .is_err()
        );
        for name in ["", " ", "line\nbreak"] {
            assert!(token(name).validate().is_err());
        }
        let mut cloudflare = token("Cloudflare");
        cloudflare.provider = DeploymentProvider::CloudflareWorkers;
        assert!(cloudflare.validate().is_err());
        cloudflare.account_id = "a".repeat(32);
        cloudflare.validate().unwrap();
        cloudflare.provider = DeploymentProvider::AzureSwa;
        assert!(cloudflare.validate().is_err());
        fs::write(&path, r#"{"tokens":[],"token":"unexpected"}"#).unwrap();
        assert!(TokenCatalog::load(&path).is_err());
        assert!(
            catalog
                .save_token(&path, token("Other"), "synthetic-secret")
                .is_err()
        );
    }
}
