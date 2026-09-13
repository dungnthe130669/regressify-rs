use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use crate::{common, helpers::OnceLockExtensions};

const VIEWPORTS: &str = "- label: phone\n  width: 375\n  height: 667\n- label: tablet\n  width: 768\n  height: 1024\n- label: desktop\n  width: 1366\n  height: 768\n";
const COOKIES: &str = "[]\n";
const OVERRIDE_CSS: &str = "/* Add project-specific CSS overrides here. */\n";
const ON_READY_JS: &str = "// Add project-specific JavaScript that runs before capture.\n";
const REPLACEMENT_PROFILES: &str = "profiles:\n  default: []\n";
const SAMPLE_SUITE: &str = "scenarios:\n  - url: https://example.com\n    label: example-home\n";

pub fn init() -> Result<()> {
    let root_dir = common::ROOT_DIR.get_required()?;
    init_workspace(root_dir)
}

fn init_workspace(root_dir: &Path) -> Result<()> {
    write_if_missing(root_dir.join("common/_viewports.yaml"), VIEWPORTS)?;
    write_if_missing(root_dir.join("common/_cookies.yaml"), COOKIES)?;
    write_if_missing(root_dir.join("common/_override.css"), OVERRIDE_CSS)?;
    write_if_missing(root_dir.join("common/_on-ready.js"), ON_READY_JS)?;
    write_if_missing(
        root_dir.join("common/_replacement-profiles.yaml"),
        REPLACEMENT_PROFILES,
    )?;
    write_if_missing(root_dir.join("visual_tests/alloy.tests.yaml"), SAMPLE_SUITE)?;
    Ok(())
}

fn write_if_missing(path: impl AsRef<Path>, contents: &str) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            if let Err(error) = file
                .write_all(contents.as_bytes())
                .and_then(|_| file.sync_all())
            {
                let _ = fs::remove_file(path);
                return Err(error.into());
            }
            eprintln!("{} {}", "CREATE:".green(), path.display());
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            eprintln!("{} {}", "SKIP:".yellow(), path.display());
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_starter_files_without_overwriting_existing_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let existing = temp_dir.path().join("common/_override.css");
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        fs::write(&existing, "existing").unwrap();

        init_workspace(temp_dir.path()).unwrap();

        assert!(temp_dir.path().join("common/_viewports.yaml").exists());
        assert!(temp_dir.path().join("common/_cookies.yaml").exists());
        assert!(temp_dir.path().join("common/_on-ready.js").exists());
        assert!(
            temp_dir
                .path()
                .join("common/_replacement-profiles.yaml")
                .exists()
        );
        assert!(
            temp_dir
                .path()
                .join("visual_tests/alloy.tests.yaml")
                .exists()
        );
        assert_eq!(fs::read_to_string(existing).unwrap(), "existing");
        assert_eq!(
            fs::read_to_string(temp_dir.path().join("common/_cookies.yaml")).unwrap(),
            COOKIES
        );
    }
}
