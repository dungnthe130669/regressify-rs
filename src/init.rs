use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

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
    write_if_missing(root_dir, "common/_viewports.yaml", VIEWPORTS)?;
    write_if_missing(root_dir, "common/_cookies.yaml", COOKIES)?;
    write_if_missing(root_dir, "common/_override.css", OVERRIDE_CSS)?;
    write_if_missing(root_dir, "common/_on-ready.js", ON_READY_JS)?;
    write_if_missing(
        root_dir,
        "common/_replacement-profiles.yaml",
        REPLACEMENT_PROFILES,
    )?;
    write_if_missing(root_dir, "visual_tests/alloy.tests.yaml", SAMPLE_SUITE)?;
    Ok(())
}

fn ensure_real_workspace_directory(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => fs::symlink_metadata(path)?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::symlink_metadata(path)?
            }
            Err(error) => return Err(error.into()),
        },
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "Refusing to initialize through symlinked directory '{}'",
            path.display()
        );
    }
    if !metadata.is_dir() {
        anyhow::bail!(
            "Cannot initialize starter files: '{}' is not a directory",
            path.display()
        );
    }
    Ok(())
}

fn workspace_file_path(root_dir: &Path, relative_path: &str) -> Result<PathBuf> {
    let root_dir = root_dir.canonicalize()?;
    let relative_path = Path::new(relative_path);
    if relative_path.is_absolute() {
        anyhow::bail!(
            "Starter file path '{}' must be relative",
            relative_path.display()
        );
    }

    let mut parent = root_dir.clone();
    for component in relative_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
    {
        let Component::Normal(component) = component else {
            anyhow::bail!(
                "Starter file path '{}' must not contain non-normal components",
                relative_path.display()
            );
        };
        parent.push(component);
        ensure_real_workspace_directory(&parent)?;
    }

    let file_name = relative_path.file_name().ok_or_else(|| {
        anyhow::anyhow!(
            "Starter file path '{}' has no filename",
            relative_path.display()
        )
    })?;
    Ok(parent.join(file_name))
}

fn write_if_missing(root_dir: &Path, relative_path: &str, contents: &str) -> Result<()> {
    let path = workspace_file_path(root_dir, relative_path)?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Err(error) = file
                .write_all(contents.as_bytes())
                .and_then(|_| file.sync_all())
            {
                let _ = fs::remove_file(&path);
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

    #[cfg(unix)]
    #[test]
    fn init_refuses_symlinked_workspace_directories() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), workspace.path().join("common")).unwrap();

        let error = init_workspace(workspace.path()).unwrap_err();

        assert!(error.to_string().contains("symlinked directory"));
        assert!(!outside.path().join("_viewports.yaml").exists());
    }

    #[test]
    fn workspace_directory_creation_accepts_an_existing_real_directory() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("common");
        fs::create_dir(&directory).unwrap();

        ensure_real_workspace_directory(&directory).unwrap();
    }
}
