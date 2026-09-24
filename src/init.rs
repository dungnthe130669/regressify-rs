use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
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
    migrate_legacy_files(root_dir)?;
    init_runtime_files(root_dir)?;
    write_if_missing(root_dir, "visual_tests/alloy.tests.yaml", SAMPLE_SUITE)?;
    for (path, schema) in [
        (
            "common/test-schema.json",
            schemars::schema_for!(crate::types::TestSuite),
        ),
        (
            "common/regressify-schema.json",
            schemars::schema_for!(crate::config::WorkspaceConfig),
        ),
        (
            "common/replacement-profiles-schema.json",
            schemars::schema_for!(crate::config::replacements::ReplacementProfilesFile),
        ),
    ] {
        write_if_missing(root_dir, path, &serde_json::to_string_pretty(&schema)?)?;
    }
    let settings = serde_json::json!({
        "json.schemas": [{"fileMatch": ["/visual_tests/**/*.tests.json"], "url": "./common/test-schema.json"}],
        "yaml.schemas": {
            "./common/test-schema.json": "/visual_tests/**/*.tests.{yaml,yml}",
            "./common/regressify-schema.json": "/regressify.{yaml,yml}",
            "./common/replacement-profiles-schema.json": "/common/_replacement-profiles.{yaml,yml}"
        }
    });
    write_if_missing(
        root_dir,
        ".vscode/settings.json",
        &serde_json::to_string_pretty(&settings)?,
    )?;
    write_if_missing(
        root_dir,
        ".vscode/extensions.json",
        "{\n  \"recommendations\": [\"redhat.vscode-yaml\"]\n}\n",
    )?;
    Ok(())
}

#[cfg(feature = "desktop")]
pub(crate) fn init_desktop_workspace(root_dir: &Path) -> Result<()> {
    init_runtime_files(root_dir)?;
    ensure_real_workspace_directory(&root_dir.join("visual_tests"))?;
    Ok(())
}

fn init_runtime_files(root_dir: &Path) -> Result<()> {
    write_yaml_if_missing(root_dir, "regressify.yaml", "{}\n")?;
    write_if_missing(root_dir, "common/_viewports.yaml", VIEWPORTS)?;
    write_if_missing(root_dir, "common/_cookies.yaml", COOKIES)?;
    write_if_missing(root_dir, "common/_override.css", OVERRIDE_CSS)?;
    write_if_missing(root_dir, "common/_on-ready.js", ON_READY_JS)?;
    write_yaml_if_missing(
        root_dir,
        "common/_replacement-profiles.yaml",
        REPLACEMENT_PROFILES,
    )?;
    Ok(())
}

fn migrate_legacy_files(root_dir: &Path) -> Result<()> {
    let source = root_dir.join("data");
    if !source.exists() {
        return Ok(());
    }
    ensure_real_workspace_directory(&source)?;
    let common_files = [
        "_cookies.yaml",
        "_on-ready.js",
        "_override.css",
        "_replacement-profiles.yaml",
        "_signing-in.yaml",
        "_viewports.yaml",
    ];
    for entry in walkdir::WalkDir::new(&source).follow_links(false) {
        let entry = entry?;
        anyhow::ensure!(
            !entry.file_type().is_symlink(),
            "Refusing to migrate symlink '{}'",
            entry.path().display()
        );
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(&source)?;
        let common = relative.components().count() == 1
            && common_files.contains(&relative.to_string_lossy().as_ref());
        let destination = Path::new(if common { "common" } else { "visual_tests" }).join(relative);
        let path = workspace_file_path(
            root_dir,
            destination
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Legacy path must be UTF-8"))?,
        )?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut target) => {
                let copied = std::io::copy(&mut fs::File::open(entry.path())?, &mut target)
                    .and_then(|_| target.sync_all());
                if let Err(error) = copied {
                    drop(target);
                    fs::remove_file(&path)?;
                    return Err(error.into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn ensure_real_workspace_directory(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => fs::symlink_metadata(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::symlink_metadata(path)
            }
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    }
    .with_context(|| format!("Cannot initialize workspace directory '{}'", path.display()))?;
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

fn existing_starter_file(path: &Path) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Cannot inspect starter file '{}'", path.display()));
        }
    };
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "Refusing to initialize symlinked file '{}'",
        path.display()
    );
    anyhow::ensure!(
        metadata.is_file(),
        "Cannot initialize starter files: '{}' is not a regular file",
        path.display()
    );
    Ok(true)
}

fn write_yaml_if_missing(root_dir: &Path, relative_path: &str, contents: &str) -> Result<()> {
    let path = workspace_file_path(root_dir, relative_path)?;
    let primary_exists = existing_starter_file(&path)?;
    let alternate_exists = existing_starter_file(&path.with_extension("yml"))?;
    if primary_exists || alternate_exists {
        return Ok(());
    }
    write_if_missing(root_dir, relative_path, contents)
}

fn write_if_missing(root_dir: &Path, relative_path: &str, contents: &str) -> Result<()> {
    let path = workspace_file_path(root_dir, relative_path)?;
    if existing_starter_file(&path)? {
        eprintln!("{} {}", "SKIP:".yellow(), path.display());
        return Ok(());
    }
    let parent = path
        .parent()
        .context("Starter file must have a parent directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("Cannot create starter file '{}'", path.display()))?;
    temporary
        .write_all(contents.as_bytes())
        .and_then(|_| temporary.as_file().sync_all())
        .with_context(|| format!("Cannot write starter file '{}'", path.display()))?;
    // Publish a complete file without replacing a concurrent edit or another opener's file.
    match temporary.persist_noclobber(&path) {
        Ok(_) => {
            eprintln!("{} {}", "CREATE:".green(), path.display());
            Ok(())
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            anyhow::ensure!(
                existing_starter_file(&path)?,
                "Starter file '{}' disappeared during initialization",
                path.display()
            );
            eprintln!("{} {}", "SKIP:".yellow(), path.display());
            Ok(())
        }
        Err(error) => Err(error.error)
            .with_context(|| format!("Cannot create starter file '{}'", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_preserves_editor_settings_and_migrates_only_missing_legacy_files() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        fs::create_dir_all(root.join("data/nested")).unwrap();
        fs::write(root.join("data/_cookies.yaml"), "[]\n").unwrap();
        fs::write(root.join("data/nested/shop.tests.yaml"), SAMPLE_SUITE).unwrap();
        fs::create_dir_all(root.join(".vscode")).unwrap();
        fs::write(
            root.join(".vscode/settings.json"),
            "{ // user settings\n}\n",
        )
        .unwrap();
        init_workspace(root).unwrap();
        init_workspace(root).unwrap();
        assert!(root.join("data/nested/shop.tests.yaml").exists());
        assert_eq!(
            fs::read_to_string(root.join("visual_tests/nested/shop.tests.yaml")).unwrap(),
            SAMPLE_SUITE
        );
        assert_eq!(
            fs::read_to_string(root.join(".vscode/settings.json")).unwrap(),
            "{ // user settings\n}\n"
        );
        let schema: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.join("common/test-schema.json")).unwrap(),
        )
        .unwrap();
        assert!(
            schema["$defs"]["TestScenario"]["properties"]
                .get("readySelector")
                .is_some()
        );
        let auth = &schema["$defs"]["BasicAuth"];
        assert!(
            auth["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("origin"))
        );
        assert_eq!(
            auth["properties"]["origin"]["examples"][0],
            "https://example.com"
        );
        assert!(
            auth["properties"]["password"]["description"]
                .as_str()
                .unwrap()
                .contains("${BASIC_AUTH_PASSWORD}")
        );
        assert_eq!(schema["properties"]["misMatchThreshold"]["maximum"], 100.0);
        assert_eq!(
            schema["$defs"]["Browser"]["enum"],
            serde_json::json!(["chromium"])
        );
        fs::write(root.join("common/test-schema.json"), "custom-schema\n").unwrap();
        init_workspace(root).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("common/test-schema.json")).unwrap(),
            "custom-schema\n"
        );
    }

    #[test]
    fn init_creates_starter_files_without_overwriting_existing_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let existing = temp_dir.path().join("common/_override.css");
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        fs::write(&existing, "existing").unwrap();

        init_workspace(temp_dir.path()).unwrap();

        assert_eq!(
            fs::read_to_string(temp_dir.path().join("regressify.yaml")).unwrap(),
            "{}\n"
        );
        assert!(crate::config::load_workspace_config(temp_dir.path()).is_ok());
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

    #[test]
    fn init_preserves_existing_workspace_config_in_either_yaml_format() {
        for name in ["regressify.yaml", "regressify.yml"] {
            let workspace = tempfile::tempdir().unwrap();
            let root = workspace.path();
            fs::write(root.join(name), "# project settings\ndelay: 75\n").unwrap();

            init_workspace(root).unwrap();
            init_workspace(root).unwrap();

            assert_eq!(
                fs::read_to_string(root.join(name)).unwrap(),
                "# project settings\ndelay: 75\n"
            );
            assert_eq!(
                crate::config::load_workspace_config(root).unwrap().delay,
                Some(75.0)
            );
            if name == "regressify.yml" {
                assert!(!root.join("regressify.yaml").exists());
            }
        }
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_init_preserves_alternate_yaml_files_and_project_data() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        fs::create_dir(root.join("common")).unwrap();
        fs::create_dir(root.join("visual_tests")).unwrap();
        fs::create_dir(root.join(".vscode")).unwrap();
        let existing = [
            ("regressify.yml", "delay: 75\n"),
            (
                "common/_replacement-profiles.yml",
                "profiles:\n  staging:\n    - ref: https://example.com\n      test: https://staging.example.com\n",
            ),
            (
                "common/_viewports.yaml",
                "- label: custom\n  width: 800\n  height: 600\n",
            ),
            ("common/_cookies.yaml", "[]\n"),
            ("common/_override.css", "body { color: red; }\n"),
            ("common/_on-ready.js", "window.projectReady = true;\n"),
            ("common/test-schema.json", "{\"custom\": true}\n"),
            ("visual_tests/shop.tests.yml", SAMPLE_SUITE),
            (".vscode/settings.json", "{ // user settings\n}\n"),
        ];
        for (path, contents) in existing {
            fs::write(root.join(path), contents).unwrap();
        }

        init_desktop_workspace(root).unwrap();
        init_desktop_workspace(root).unwrap();

        for (path, contents) in existing {
            assert_eq!(fs::read_to_string(root.join(path)).unwrap(), contents);
        }
        assert!(!root.join("regressify.yaml").exists());
        assert!(!root.join("common/_replacement-profiles.yaml").exists());
        assert!(!root.join("visual_tests/alloy.tests.yaml").exists());
        assert_eq!(
            crate::config::load_workspace_config(root).unwrap().delay,
            Some(75.0)
        );
        assert!(
            crate::config::replacements::load_replacement_profiles(root)
                .unwrap()
                .unwrap()
                .profiles
                .contains_key("staging")
        );
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn concurrent_desktop_init_creates_only_complete_runtime_files() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        std::thread::scope(|scope| {
            let threads: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| init_desktop_workspace(root)))
                .collect();
            for thread in threads {
                thread.join().unwrap().unwrap();
            }
        });

        let expected = [
            ("regressify.yaml", "{}\n"),
            ("common/_viewports.yaml", VIEWPORTS),
            ("common/_cookies.yaml", COOKIES),
            ("common/_override.css", OVERRIDE_CSS),
            ("common/_on-ready.js", ON_READY_JS),
            ("common/_replacement-profiles.yaml", REPLACEMENT_PROFILES),
        ];
        for (path, contents) in expected {
            assert_eq!(fs::read_to_string(root.join(path)).unwrap(), contents);
        }
        assert_eq!(
            walkdir::WalkDir::new(root)
                .into_iter()
                .map(Result::unwrap)
                .filter(|entry| entry.file_type().is_file())
                .count(),
            expected.len()
        );
        assert!(root.join("visual_tests").is_dir());
    }

    #[test]
    fn init_rejects_non_file_starter_targets() {
        for relative in [
            "regressify.yaml",
            "regressify.yml",
            "common/_viewports.yaml",
            "common/_cookies.yaml",
            "common/_override.css",
            "common/_on-ready.js",
            "common/_replacement-profiles.yaml",
            ".vscode/settings.json",
        ] {
            let workspace = tempfile::tempdir().unwrap();
            let blocked = workspace.path().join(relative);
            fs::create_dir_all(&blocked).unwrap();
            fs::write(blocked.join("keep.txt"), "existing").unwrap();

            let error = init_workspace(workspace.path()).unwrap_err();

            assert!(
                format!("{error:#}")
                    .contains(&blocked.canonicalize().unwrap().display().to_string()),
                "Missing blocked path in error: {error:#}"
            );
            assert_eq!(
                fs::read_to_string(blocked.join("keep.txt")).unwrap(),
                "existing"
            );
        }
    }

    #[test]
    fn init_preserves_read_only_existing_files() {
        let workspace = tempfile::tempdir().unwrap();
        fs::create_dir(workspace.path().join("common")).unwrap();
        let existing = workspace.path().join("common/_cookies.yaml");
        fs::write(&existing, "# project cookies\n[]\n").unwrap();
        let permissions = fs::metadata(&existing).unwrap().permissions();
        let mut read_only = permissions.clone();
        read_only.set_readonly(true);
        fs::set_permissions(&existing, read_only).unwrap();

        let result = init_workspace(workspace.path());

        fs::set_permissions(&existing, permissions).unwrap();
        result.unwrap();
        assert_eq!(
            fs::read_to_string(existing).unwrap(),
            "# project cookies\n[]\n"
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

    #[cfg(unix)]
    #[test]
    fn init_refuses_existing_and_dangling_symlinked_starter_files() {
        for relative in [
            "regressify.yaml",
            "regressify.yml",
            "common/_cookies.yaml",
            "common/_replacement-profiles.yml",
        ] {
            for dangling in [false, true] {
                let workspace = tempfile::tempdir().unwrap();
                let outside = tempfile::tempdir().unwrap();
                fs::create_dir(workspace.path().join("common")).unwrap();
                let target = outside.path().join("existing.yaml");
                if !dangling {
                    fs::write(&target, "existing").unwrap();
                }
                std::os::unix::fs::symlink(&target, workspace.path().join(relative)).unwrap();

                let error = init_workspace(workspace.path()).unwrap_err();

                assert!(error.to_string().contains("symlinked file"));
                if dangling {
                    assert!(!target.exists());
                } else {
                    assert_eq!(fs::read_to_string(&target).unwrap(), "existing");
                }
            }
        }
    }

    #[test]
    fn workspace_directory_creation_accepts_an_existing_real_directory() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("common");
        fs::create_dir(&directory).unwrap();

        ensure_real_workspace_directory(&directory).unwrap();
    }
}
