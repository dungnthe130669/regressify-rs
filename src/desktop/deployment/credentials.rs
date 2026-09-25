use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, anyhow, ensure};
use keyring::Entry;
use zeroize::Zeroizing;

pub struct DeploymentCredential {
    entry: Arc<Entry>,
}

impl DeploymentCredential {
    pub fn new(root: &Path, id: &str) -> Result<Self> {
        Self::for_key(&credential_key(root, id)?)
    }

    pub fn named(id: &str) -> Result<Self> {
        let id = uuid::Uuid::parse_str(id).context("Invalid named deployment token ID")?;
        Self::for_key(&format!("named:{id}"))
    }

    fn for_key(key: &str) -> Result<Self> {
        ensure!(
            cfg!(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "linux",
                target_os = "freebsd",
                target_os = "openbsd"
            )),
            "Secure deployment token storage is unavailable on this platform"
        );
        let entry = credential_entry(key)?;
        Ok(Self { entry })
    }

    pub fn read(&self) -> Result<Zeroizing<String>> {
        self.read_optional()?
            .context("No saved deployment token. Edit the target and enter its deployment token")
    }

    pub fn read_optional(&self) -> Result<Option<Zeroizing<String>>> {
        match self.entry.get_password() {
            Ok(token) => Ok(Some(Zeroizing::new(token))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(anyhow!(
                "Cannot read the deployment token. Unlock your operating system credential store and try again"
            )),
        }
    }

    pub fn write(&self, token: &str) -> Result<()> {
        ensure!(!token.trim().is_empty(), "Enter a deployment token");
        #[cfg(all(windows, not(test)))]
        let result = write_local_windows_credential(&self.entry, token);
        #[cfg(any(not(windows), test))]
        let result = self.entry.set_password(token);
        result.map_err(|_| {
            anyhow!(
                "Cannot securely save the deployment token. Unlock your operating system credential store and try again"
            )
        })
    }

    pub fn delete(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(anyhow!(
                "Cannot remove the saved deployment token. Unlock your operating system credential store and try again"
            )),
        }
    }
}

#[cfg(all(windows, not(test)))]
fn write_local_windows_credential(entry: &Entry, token: &str) -> Result<()> {
    use windows_sys::Win32::Security::Credentials::{
        CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW,
        CredWriteW,
    };
    let identity = entry
        .get_credential()
        .downcast_ref::<keyring::windows::WinCredential>()
        .context("Windows credential store unavailable")?;
    let mut target = identity
        .target_name
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut username = identity
        .username
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut secret = Zeroizing::new(token.encode_utf16().collect::<Vec<_>>());
    let byte_length = secret
        .len()
        .checked_mul(2)
        .context("Deployment token is too long")?;
    ensure!(
        byte_length <= CRED_MAX_CREDENTIAL_BLOB_SIZE as usize,
        "Deployment token is too long"
    );
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        UserName: username.as_mut_ptr(),
        CredentialBlob: secret.as_mut_ptr().cast(),
        CredentialBlobSize: byte_length as u32,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..unsafe { std::mem::zeroed() }
    };
    ensure!(
        unsafe { CredWriteW(&credential, 0) } != 0,
        "Secure credential write failed"
    );
    Ok(())
}

#[cfg(not(test))]
fn credential_entry(key: &str) -> Result<Arc<Entry>> {
    Entry::new("Regressify Desktop Deployment", key)
        .map(Arc::new)
        .map_err(|_| anyhow!("Cannot access the operating system credential store"))
}

#[cfg(test)]
fn credential_entry(key: &str) -> Result<Arc<Entry>> {
    use std::{
        collections::BTreeMap,
        sync::{Mutex, OnceLock},
    };
    static ENTRIES: OnceLock<Mutex<BTreeMap<String, Arc<Entry>>>> = OnceLock::new();
    let mut entries = ENTRIES
        .get_or_init(Default::default)
        .lock()
        .expect("Test credential store lock");
    if let Some(entry) = entries.get(key) {
        return Ok(entry.clone());
    }
    let credential =
        keyring::mock::default_credential_builder().build(None, "regressify-tests", key)?;
    let entry = Arc::new(Entry::new_with_credential(credential));
    entries.insert(key.into(), entry.clone());
    Ok(entry)
}

fn credential_key(root: &Path, id: &str) -> Result<String> {
    ensure!(
        !id.is_empty()
            && id.len() <= 80
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
        "Invalid deployment credential ID"
    );
    Ok(format!("{}:{id}", super::workspace_key(root)?))
}

#[cfg(test)]
mod tests {
    use super::{DeploymentCredential, credential_key};

    #[test]
    fn deployment_credentials_can_be_saved_replaced_and_removed() {
        let credential = keyring::mock::default_credential_builder()
            .build(None, "regressify-tests", "deployment")
            .unwrap();
        let store = DeploymentCredential {
            entry: std::sync::Arc::new(keyring::Entry::new_with_credential(credential)),
        };
        assert!(store.read().is_err());
        assert!(store.write(" ").is_err());
        store.write("synthetic-token-one").unwrap();
        assert_eq!(store.read().unwrap().as_str(), "synthetic-token-one");
        store.write("synthetic-token-two").unwrap();
        assert_eq!(store.read().unwrap().as_str(), "synthetic-token-two");
        store.delete().unwrap();
        assert!(store.read().is_err());
        store.delete().unwrap();
    }

    #[test]
    fn credential_errors_never_include_secret_bytes() {
        let root = tempfile::tempdir().unwrap();
        let store = DeploymentCredential::new(root.path(), "redaction").unwrap();
        let mock = store
            .entry
            .get_credential()
            .downcast_ref::<keyring::mock::MockCredential>()
            .unwrap();
        let token = "synthetic-secret-error-payload";
        mock.set_error(keyring::Error::BadEncoding(token.as_bytes().to_vec()));
        assert!(!format!("{:#}", store.read().unwrap_err()).contains(token));
        mock.set_error(keyring::Error::BadEncoding(token.as_bytes().to_vec()));
        assert!(!format!("{:#}", store.write(token).unwrap_err()).contains(token));
        mock.set_error(keyring::Error::BadEncoding(token.as_bytes().to_vec()));
        assert!(!format!("{:#}", store.delete().unwrap_err()).contains(token));
    }

    #[test]
    fn deployment_credentials_are_scoped_to_workspace_and_target() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let key = credential_key(first.path(), "qa").unwrap();
        assert_ne!(key, credential_key(second.path(), "qa").unwrap());
        assert_ne!(key, credential_key(first.path(), "production").unwrap());
        assert_eq!(key, credential_key(&first.path().join("."), "qa").unwrap());
        assert!(credential_key(first.path(), "../qa").is_err());
        assert!(credential_key(first.path(), "").is_err());
    }

    #[test]
    fn named_credentials_are_shared_without_exposing_workspace_credentials() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let named = DeploymentCredential::named(&id).unwrap();
        named.write("synthetic-shared-token").unwrap();
        let local = DeploymentCredential::new(first.path(), &id).unwrap();
        local.write("synthetic-local-token").unwrap();
        assert_eq!(
            DeploymentCredential::named(&id)
                .unwrap()
                .read()
                .unwrap()
                .as_str(),
            "synthetic-shared-token"
        );
        assert_eq!(local.read().unwrap().as_str(), "synthetic-local-token");
        assert!(
            DeploymentCredential::new(second.path(), &id)
                .unwrap()
                .read()
                .is_err()
        );
        named.delete().unwrap();
        assert!(DeploymentCredential::named(&id).unwrap().read().is_err());
        assert_eq!(local.read().unwrap().as_str(), "synthetic-local-token");
        for invalid in ["", "../token", "not-a-uuid"] {
            assert!(DeploymentCredential::named(invalid).is_err());
        }
    }
}
