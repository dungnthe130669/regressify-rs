//! Workspace-scoped persistence. Editor drafts never write directly to disk.

use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use super::document::{Document, DocumentKind, Format, MAX_DOCUMENT_BYTES};
use crate::helpers::{is_windows_reserved_path_component, write_file_atomically};

/// Format paths for people without changing filesystem identity or resolving relative paths.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if let Some(network) = text
        .strip_prefix("//?/UNC/")
        .or_else(|| text.strip_prefix("//?/unc/"))
    {
        format!("//{network}")
    } else {
        text.strip_prefix("//?/").unwrap_or(&text).to_string()
    }
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub root: PathBuf,
}

/// One desktop window per user. Worker processes never acquire this lease.
pub struct DesktopInstance {
    _lock: fs::File,
}

impl DesktopInstance {
    pub fn acquire(path: &Path) -> Result<Self> {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| anyhow!("Invalid instance lock path"))?,
        )?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        lock.try_lock().map_err(|_| {
            anyhow!("Regressify Desktop is already open. Switch workspaces in the existing window.")
        })?;
        Ok(Self { _lock: lock })
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: DocumentKind,
}

impl Entry {
    pub fn label(&self) -> String {
        display_path(&self.path)
    }
    pub fn suite_name(&self) -> Option<String> {
        let filename = self.path.file_name()?.to_str()?.to_ascii_lowercase();
        [".tests.yaml", ".tests.yml", ".tests.json"]
            .iter()
            .find_map(|suffix| filename.strip_suffix(suffix).map(str::to_string))
    }
}

#[derive(Debug)]
pub struct OpenDocument {
    pub entry: Entry,
    pub document: Document,
}

#[derive(Clone, Debug)]
pub struct SuiteSummary {
    pub entry: Entry,
    pub name: String,
    pub output_id: String,
    pub scenarios: Vec<ScenarioSummary>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ScenarioSummary {
    pub id: String,
    pub label: String,
    pub url: String,
}

#[derive(Clone, Debug, Default)]
pub struct SuiteSelection {
    names: BTreeSet<String>,
}

impl SuiteSelection {
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }
    pub fn len(&self) -> usize {
        self.names.len()
    }
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
    pub fn clear(&mut self) {
        self.names.clear();
    }
    pub fn toggle(&mut self, suite: &SuiteSummary, selected: bool) -> Result<()> {
        ensure!(
            suite.error.is_none(),
            "Suite '{}' has configuration errors",
            suite.name
        );
        if selected {
            self.names.insert(suite.name.clone());
        } else {
            self.names.remove(&suite.name);
        }
        Ok(())
    }
    pub fn select_all(&mut self, suites: &[SuiteSummary]) {
        self.names = suites
            .iter()
            .filter(|suite| suite.error.is_none())
            .map(|suite| suite.name.clone())
            .collect();
    }
    pub fn reconcile(&mut self, suites: &[SuiteSummary]) {
        self.names.retain(|name| {
            suites
                .iter()
                .any(|suite| &suite.name == name && suite.error.is_none())
        });
    }
    pub fn selected<'suite>(
        &self,
        suites: &'suite [SuiteSummary],
    ) -> Result<Vec<&'suite SuiteSummary>> {
        ensure!(!self.is_empty(), "Select at least one test suite");
        let selected: Vec<_> = suites
            .iter()
            .filter(|suite| self.contains(&suite.name) && suite.error.is_none())
            .collect();
        ensure!(
            selected.len() == self.len(),
            "Selected suites changed; refresh the workspace before running"
        );
        Ok(selected)
    }
    pub fn filters(&self, suites: &[SuiteSummary], output_ids: bool) -> Result<Vec<String>> {
        Ok(self
            .selected(suites)?
            .into_iter()
            .map(|suite| {
                glob::Pattern::escape(if output_ids {
                    &suite.output_id
                } else {
                    &suite.name
                })
            })
            .collect())
    }
}

impl ScenarioSummary {
    pub fn from_scenario(scenario: &crate::types::TestScenario) -> Self {
        let id = scenario.id.clone().unwrap_or_else(|| {
            scenario
                .label
                .as_deref()
                .map(crate::helpers::slugify)
                .unwrap_or_else(|| crate::snapshot::get_asset_version(scenario.url.as_bytes(), 20))
        });
        Self {
            label: scenario.label.clone().unwrap_or_else(|| id.clone()),
            id,
            url: scenario.url.clone(),
        }
    }
}

impl Workspace {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("Cannot open workspace '{}'", display_path(root)))?;
        ensure!(root.is_dir(), "Workspace must be a directory");
        Ok(Self { root })
    }

    /// Validate every existing component, not only the final filename.
    pub fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        ensure!(
            !relative.as_os_str().is_empty(),
            "A workspace-relative filename is required"
        );
        let mut destination = self.root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(anyhow!(
                    "Path must stay inside the workspace: '{}'",
                    display_path(relative)
                ));
            };
            validate_component(
                name.to_str()
                    .ok_or_else(|| anyhow!("Filename must be UTF-8"))?,
            )?;
            destination.push(name);
            match fs::symlink_metadata(&destination) {
                Ok(_) => {
                    destination = destination
                        .canonicalize()
                        .context("Cannot resolve workspace path")?;
                    ensure!(
                        destination.starts_with(&self.root),
                        "Path escapes the workspace: '{}'",
                        display_path(relative)
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(destination)
    }

    pub fn entries(&self) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for name in ["regressify.yaml", "regressify.yml"] {
            let path = PathBuf::from(name);
            if self.resolve(&path)?.is_file() {
                entries.push(Entry {
                    path,
                    kind: DocumentKind::Workspace,
                });
            }
        }
        for folder in ["visual_tests", "common"] {
            let directory = self.resolve(Path::new(folder))?;
            if !directory.exists() {
                continue;
            }
            for item in WalkDir::new(directory).follow_links(false).max_depth(24) {
                let item = item.context("Cannot list workspace files")?;
                if !item.file_type().is_file() {
                    continue;
                }
                let path = item.path().strip_prefix(&self.root)?.to_path_buf();
                let name = item.file_name().to_string_lossy().to_ascii_lowercase();
                let suite = [".tests.yaml", ".tests.yml", ".tests.json"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix));
                let data = folder == "common"
                    && ["yaml", "yml", "json"].iter().any(|extension| {
                        path.extension().is_some_and(|actual| actual == *extension)
                    });
                if suite || data {
                    entries.push(Entry {
                        path,
                        kind: if suite {
                            DocumentKind::Suite
                        } else if name.contains("viewports") {
                            DocumentKind::Viewports
                        } else {
                            DocumentKind::Data
                        },
                    });
                }
            }
        }
        entries.sort_by_key(Entry::label);
        Ok(entries)
    }

    pub fn open_document(&self, entry: Entry) -> Result<OpenDocument> {
        let source = read_document(&self.resolve(&entry.path)?)?;
        let document = Document::new(entry.kind, Format::for_path(&entry.path), source)?;
        Ok(OpenDocument { entry, document })
    }

    pub fn viewport_choices(
        &self,
        path: &Path,
        explicit: bool,
    ) -> Result<Vec<crate::config::viewports::ViewportDef>> {
        let resolved = self.resolve(path)?;
        if !resolved.exists() && !explicit {
            return Ok(Vec::new());
        }
        ensure!(
            resolved.is_file(),
            "Viewport file '{}' does not exist",
            display_path(path)
        );
        let document = self
            .open_document(Entry {
                path: path.into(),
                kind: DocumentKind::Viewports,
            })?
            .document;
        document.validate()?;
        Ok(serde_json::from_value(document.value()?)?)
    }

    pub fn suite_catalog(&self) -> Result<Vec<SuiteSummary>> {
        let directory = self.resolve(Path::new("visual_tests"))?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        crate::test::discover_test_suites(&directory)?
            .into_iter()
            .map(|input| {
                let entry = Entry {
                    path: input.path.strip_prefix(&self.root)?.into(),
                    kind: DocumentKind::Suite,
                };
                let parsed = (|| -> Result<crate::types::TestSuite> {
                    ensure!(input.name != "all", "The suite name 'all' is reserved");
                    let document = self.open_document(entry.clone())?.document;
                    document.validate()?;
                    validate_typed_source(&document)?;
                    let suite: crate::types::TestSuite = serde_json::from_value(document.value()?)?;
                    ensure!(!suite.scenarios.is_empty(), "Suite contains no scenarios");
                    Ok(suite)
                })();
                let output_id = parsed
                    .as_ref()
                    .ok()
                    .and_then(|suite| suite.id.clone())
                    .unwrap_or_else(|| input.name.clone());
                let scenarios = parsed
                    .as_ref()
                    .map(|suite| {
                        suite
                            .scenarios
                            .iter()
                            .map(ScenarioSummary::from_scenario)
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(SuiteSummary {
                    entry,
                    name: input.name,
                    output_id,
                    scenarios,
                    error: parsed.err().map(|error| format!("{error:#}")),
                })
            })
            .collect()
    }

    /// A filename filter must resolve to the selected file, not a shadowing format.
    pub fn runnable_suite(&self, entry: &Entry) -> Result<String> {
        let name = entry
            .suite_name()
            .ok_or_else(|| anyhow!("Select a test suite first"))?;
        ensure!(
            name != "all",
            "The suite name 'all' is reserved by the CLI wildcard filter"
        );
        let suites = crate::test::discover_test_suites(&self.resolve(Path::new("visual_tests"))?)?;
        let actual = suites
            .into_iter()
            .find(|suite| suite.name == name)
            .ok_or_else(|| anyhow!("Suite '{name}' is not discovered by the CLI"))?;
        ensure!(
            actual.path.canonicalize()? == self.resolve(&entry.path)?,
            "The CLI runs '{}', not the selected '{}'. Rename or remove the shadowing suite before running this file.",
            display_path(&actual.path),
            display_path(&entry.path)
        );
        Ok(name)
    }

    pub fn reload(&self, open: &mut OpenDocument) -> Result<()> {
        *open = self.open_document(open.entry.clone())?;
        Ok(())
    }

    /// Upload paths use visual_tests; other configured files use the workspace root.
    pub fn relative_file(&self, path: &Path, upload: bool) -> Result<String> {
        let canonical = path.canonicalize()?;
        let base = if upload {
            self.resolve(Path::new("visual_tests"))?
        } else {
            self.root.clone()
        };
        ensure!(
            canonical.is_file() && canonical.starts_with(&base),
            "Select a file inside {}",
            display_path(&base)
        );
        let relative = canonical.strip_prefix(base)?;
        Ok(display_path(relative))
    }

    pub fn check_unchanged(&self, open: &OpenDocument) -> Result<PathBuf> {
        let path = self.resolve(&open.entry.path)?;
        let current = read_document(&path)
            .context("File is unavailable; reload or Save As before continuing")?;
        ensure!(
            current == open.document.saved_source(),
            "File changed outside Regressify. Reload it or Save As a new file; your draft has been retained."
        );
        Ok(path)
    }

    pub fn save(&self, open: &mut OpenDocument) -> Result<()> {
        open.document.validate()?;
        validate_typed_source(&open.document)?;
        let path = self.check_unchanged(open)?;
        write_file_atomically(&path, open.document.source().as_bytes())?;
        open.document.mark_saved();
        Ok(())
    }

    pub fn save_as(&self, open: &OpenDocument, relative: &Path) -> Result<OpenDocument> {
        open.document.validate()?;
        validate_typed_source(&open.document)?;
        let format = Format::for_path(relative);
        let source = if format == open.document.format {
            open.document.source().to_string()
        } else {
            format.serialize(&open.document.value()?)?
        };
        self.create_document(relative, open.entry.kind, &source)
    }

    pub fn create_document(
        &self,
        relative: &Path,
        kind: DocumentKind,
        source: &str,
    ) -> Result<OpenDocument> {
        let document = Document::new(kind, Format::for_path(relative), source.to_string())?;
        document.validate()?;
        validate_typed_source(&document)?;
        let path = self.resolve(relative)?;
        ensure!(
            !path.exists(),
            "A file already exists at '{}'",
            display_path(relative)
        );
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("Missing parent directory"))?;
        fs::create_dir_all(parent)?;
        // persist_noclobber closes the create/save race without replacing another file.
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(source.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist_noclobber(&path).map_err(|error| {
            anyhow!(
                "Cannot create '{}': {}",
                display_path(relative),
                error.error
            )
        })?;
        Ok(OpenDocument {
            entry: Entry {
                path: relative.into(),
                kind,
            },
            document,
        })
    }

    pub fn create_suite(&self, name: &str) -> Result<OpenDocument> {
        let path = suite_path(name)?;
        let source = Format::for_path(&path).serialize(&serde_json::json!({
            "scenarios": [{"label": "Scenario 1", "url": "https://example.com", "actions": []}]
        }))?;
        self.create_document(&path, DocumentKind::Suite, &source)
    }

    pub fn duplicate_suite(&self, open: &OpenDocument, name: &str) -> Result<OpenDocument> {
        ensure!(
            open.entry.kind == DocumentKind::Suite,
            "Only test suites can be duplicated here"
        );
        let mut copy = Document::new(
            DocumentKind::Suite,
            open.document.format,
            open.document.source().into(),
        )?;
        // An explicit suite ID would otherwise make the copy share output artifacts.
        copy.apply(super::document::Edit::Set {
            parent: "".into(),
            key: "id".into(),
            value: None,
        })?;
        let copy = OpenDocument {
            entry: open.entry.clone(),
            document: copy,
        };
        self.save_as(&copy, &suite_path(name)?)
    }

    pub fn rename_suite(&self, open: &mut OpenDocument, name: &str) -> Result<()> {
        ensure!(
            open.entry.kind == DocumentKind::Suite,
            "Only test suites can be renamed here"
        );
        ensure!(
            !open.document.dirty(),
            "Save or discard changes before renaming"
        );
        let relative = suite_path(name)?;
        ensure!(
            Format::for_path(&relative) == open.document.format,
            "Use Save As to change file format"
        );
        let source = self.check_unchanged(open)?;
        let destination = self.resolve(&relative)?;
        ensure!(!destination.exists(), "Destination already exists");
        // Same-workspace hard linking refuses an existing destination atomically.
        fs::hard_link(&source, &destination).context("Cannot rename suite")?;
        if let Err(error) = fs::remove_file(source) {
            let _ = fs::remove_file(destination);
            return Err(error).context("Cannot remove the old suite name");
        }
        open.entry.path = relative;
        Ok(())
    }

    pub fn delete_suite(&self, open: &OpenDocument) -> Result<()> {
        ensure!(
            open.entry.kind == DocumentKind::Suite,
            "Only test suite files can be deleted here"
        );
        ensure!(
            !open.document.dirty(),
            "Save or discard changes before deleting"
        );
        fs::remove_file(self.check_unchanged(open)?).context("Cannot delete suite")
    }
}

fn validate_component(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && !name.ends_with(['.', ' '])
            && !name
                .chars()
                .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
            && !is_windows_reserved_path_component(name),
        "Invalid portable filename '{name}'"
    );
    Ok(())
}

pub fn suite_path(name: &str) -> Result<PathBuf> {
    validate_component(name)?;
    let normalized = name.to_ascii_lowercase();
    let stem = [".tests.yaml", ".tests.yml", ".tests.json"]
        .iter()
        .find_map(|suffix| normalized.strip_suffix(suffix))
        .unwrap_or(&normalized);
    ensure!(stem != "all", "The suite name 'all' is reserved by the CLI");
    let filename = if [".tests.yaml", ".tests.yml", ".tests.json"]
        .iter()
        .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
    {
        name.to_string()
    } else {
        format!("{name}.tests.yaml")
    };
    Ok(Path::new("visual_tests").join(filename))
}

fn read_document(path: &Path) -> Result<String> {
    let file =
        fs::File::open(path).with_context(|| format!("Cannot read '{}'", display_path(path)))?;
    let mut source = String::new();
    file.take(MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_string(&mut source)?;
    ensure!(
        source.len() <= MAX_DOCUMENT_BYTES,
        "Document exceeds the 4 MiB editing limit"
    );
    Ok(source)
}

fn validate_typed_source(document: &Document) -> Result<()> {
    fn parse<T: serde::de::DeserializeOwned>(document: &Document) -> Result<()> {
        match document.format {
            Format::Yaml => {
                serde_yaml::from_str::<T>(document.source())?;
            }
            Format::Json => {
                serde_json::from_str::<T>(document.source())?;
            }
        }
        Ok(())
    }
    // Parsing structs directly also catches duplicate properties with source locations.
    match document.kind {
        DocumentKind::Workspace => parse::<crate::config::WorkspaceConfig>(document),
        DocumentKind::Suite => parse::<crate::types::TestSuite>(document),
        DocumentKind::Viewports => parse::<Vec<crate::config::viewports::ViewportDef>>(document),
        DocumentKind::Data => Ok(()),
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub recent_workspaces: Vec<PathBuf>,
    pub dark_mode: bool,
    pub navigator_hidden: bool,
    #[serde(default)]
    pub theme: Option<ThemePreference>,
    pub source_wrap: bool,
    pub source_line_numbers: bool,
    pub source_font_size: u8,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            recent_workspaces: Vec::new(),
            dark_mode: false,
            navigator_hidden: false,
            theme: Some(ThemePreference::System),
            source_wrap: true,
            source_line_numbers: true,
            source_font_size: 14,
        }
    }
}

impl Preferences {
    pub fn theme_preference(&self) -> ThemePreference {
        self.theme.unwrap_or(if self.dark_mode {
            ThemePreference::Dark
        } else {
            ThemePreference::Light
        })
    }

    pub fn set_theme_preference(&mut self, preference: ThemePreference) {
        self.theme = Some(preference);
        self.dark_mode = preference == ThemePreference::Dark;
    }

    pub fn path() -> Result<PathBuf> {
        directories::ProjectDirs::from("", "Regressify", "Regressify Desktop")
            .map(|directories| directories.config_dir().join("settings.json"))
            .ok_or_else(|| anyhow!("Cannot determine the desktop settings directory"))
    }

    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(source) => {
                let mut preferences: Self =
                    serde_json::from_str(&source).context("Invalid desktop preferences")?;
                preferences.source_font_size = preferences.source_font_size.clamp(10, 24);
                Ok(preferences)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn remember(&mut self, workspace: &Workspace) {
        self.recent_workspaces
            .retain(|path| path != &workspace.root);
        self.recent_workspaces.insert(0, workspace.root.clone());
        self.recent_workspaces.truncate(10);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| anyhow!("Invalid preferences path"))?,
        )?;
        write_file_atomically(path, &serde_json::to_vec_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::document::Edit;
    use super::*;
    use serde_json::json;

    #[test]
    fn editor_display_preferences_default_safely_and_clamp_saved_font_sizes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("appearance.json");
        for (text, expected) in [
            (r#"{}"#, 14),
            (r#"{"sourceFontSize":1}"#, 10),
            (r#"{"sourceFontSize":240}"#, 24),
        ] {
            fs::write(&path, text).unwrap();
            let preferences = Preferences::load(&path).unwrap();
            assert_eq!(preferences.source_font_size, expected);
            assert!(preferences.source_wrap);
            assert!(preferences.source_line_numbers);
        }
        fs::write(
            &path,
            r#"{"sourceFontSize":18,"sourceWrap":false,"sourceLineNumbers":false}"#,
        )
        .unwrap();
        let preferences = Preferences::load(&path).unwrap();
        assert_eq!(preferences.source_font_size, 18);
        assert!(!preferences.source_wrap);
        assert!(!preferences.source_line_numbers);
    }

    #[test]
    fn theme_preferences_preserve_legacy_choices_and_follow_system_only_when_selected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("appearance.json");
        assert_eq!(
            Preferences::load(&path).unwrap().theme_preference(),
            ThemePreference::System
        );
        for (json, expected) in [
            (r#"{"darkMode":true}"#, ThemePreference::Dark),
            (r#"{"darkMode":false}"#, ThemePreference::Light),
            (
                r#"{"darkMode":true,"theme":"system"}"#,
                ThemePreference::System,
            ),
            (
                r#"{"darkMode":true,"theme":"light"}"#,
                ThemePreference::Light,
            ),
        ] {
            assert_eq!(
                serde_json::from_str::<Preferences>(json)
                    .unwrap()
                    .theme_preference(),
                expected
            );
        }
        for preference in [
            ThemePreference::System,
            ThemePreference::Light,
            ThemePreference::Dark,
        ] {
            let mut preferences = Preferences::default();
            preferences.set_theme_preference(preference);
            preferences.save(&path).unwrap();
            assert_eq!(
                Preferences::load(&path).unwrap().theme_preference(),
                preference
            );
        }
        assert!(ThemePreference::System.is_dark(true));
        assert!(!ThemePreference::System.is_dark(false));
        assert!(!ThemePreference::Light.is_dark(true));
        assert!(ThemePreference::Dark.is_dark(false));
        assert!(serde_json::from_str::<Preferences>(r#"{"theme":"unknown"}"#).is_err());
    }

    #[test]
    fn displayed_paths_remove_extended_prefixes_without_losing_network_roots() {
        for (source, expected) in [
            (r"\\?\D:\git\QA workspace", "D:/git/QA workspace"),
            (r"D:\git\QA workspace", "D:/git/QA workspace"),
            (
                r"\\?\UNC\server\share\tests.yaml",
                "//server/share/tests.yaml",
            ),
            (r"\\server\share\tests.yaml", "//server/share/tests.yaml"),
            ("//?/D:/git/tests", "D:/git/tests"),
            ("//?/UNC/server/share", "//server/share"),
            (
                r"visual_tests\checkout.tests.yaml",
                "visual_tests/checkout.tests.yaml",
            ),
            ("/home/qa/tests", "/home/qa/tests"),
        ] {
            let path = PathBuf::from(source);
            assert_eq!(display_path(&path), expected);
            assert_eq!(path, PathBuf::from(source));
        }
    }

    #[test]
    fn viewport_choices_validate_files_and_respect_workspace_boundaries() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let path = Path::new("common/_viewports.yaml");
        assert!(workspace.viewport_choices(path, false).unwrap().is_empty());
        assert!(workspace.viewport_choices(path, true).is_err());
        fs::create_dir_all(directory.path().join("common")).unwrap();
        fs::write(
            directory.path().join(path),
            "- label: Phone\n  width: 390\n  height: 844\n",
        )
        .unwrap();
        let choices = workspace.viewport_choices(path, true).unwrap();
        assert_eq!(choices[0].label, "Phone");
        assert_eq!(choices[0].width, 390);
        assert!(
            workspace
                .viewport_choices(Path::new("../outside.yaml"), true)
                .is_err()
        );
        fs::write(
            directory.path().join(path),
            "- label: Phone\n  width: 0\n  height: 844\n",
        )
        .unwrap();
        assert!(workspace.viewport_choices(path, true).is_err());
    }

    #[test]
    fn create_edit_save_and_reload_suite() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let mut open = workspace.create_suite("checkout").unwrap();
        open.document
            .apply(Edit::Set {
                parent: "/scenarios/0".into(),
                key: "label".into(),
                value: Some(json!("Checkout")),
            })
            .unwrap();
        assert!(open.document.dirty());
        workspace.save(&mut open).unwrap();
        assert!(!open.document.dirty());
        assert_eq!(workspace.entries().unwrap().len(), 1);
        workspace.reload(&mut open).unwrap();
        assert_eq!(
            open.document.value().unwrap()["scenarios"][0]["label"],
            "Checkout"
        );
        assert!(workspace.create_suite("checkout").is_err());
    }

    #[test]
    fn external_changes_and_deletion_never_get_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let mut open = workspace.create_suite("home").unwrap();
        let path = workspace.resolve(&open.entry.path).unwrap();
        fs::write(&path, "# external\nscenarios: []\n").unwrap();
        assert!(
            workspace
                .save(&mut open)
                .unwrap_err()
                .to_string()
                .contains("changed outside")
        );
        assert!(workspace.delete_suite(&open).is_err());
        assert!(workspace.rename_suite(&mut open, "other").is_err());
        assert!(fs::read_to_string(&path).unwrap().contains("external"));
        fs::remove_file(path).unwrap();
        assert!(workspace.save(&mut open).is_err());
    }

    #[test]
    fn invalid_and_duplicate_fields_cannot_be_saved() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let mut open = workspace.create_suite("home").unwrap();
        for source in [
            "scenarios: [",
            "scenarios: []\ndebug: true\ndebug: false\n",
            "scenarios: []\nunknown: 1\n",
        ] {
            open.document.set_source(source.into()).unwrap();
            assert!(workspace.save(&mut open).is_err());
        }
        let disk = workspace.open_document(open.entry.clone()).unwrap();
        assert_eq!(
            disk.document.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn suite_copy_rename_delete_preserve_data_and_avoid_output_id_collisions() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let mut original = workspace.create_suite("original").unwrap();
        original
            .document
            .apply(Edit::Set {
                parent: "".into(),
                key: "id".into(),
                value: Some(json!("original-id")),
            })
            .unwrap();
        workspace.save(&mut original).unwrap();
        let mut copy = workspace.duplicate_suite(&original, "copy").unwrap();
        assert!(copy.document.value().unwrap().get("id").is_none());
        workspace.rename_suite(&mut copy, "renamed").unwrap();
        assert!(
            !workspace
                .resolve(Path::new("visual_tests/copy.tests.yaml"))
                .unwrap()
                .exists()
        );
        assert!(workspace.rename_suite(&mut copy, "original").is_err());
        workspace.delete_suite(&copy).unwrap();
        assert_eq!(workspace.entries().unwrap().len(), 1);
    }

    #[test]
    fn paths_are_portable_and_workspace_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        for path in [
            "../escape",
            "C:\\outside",
            "CON.yaml",
            "common/file:stream",
            "file.",
            "file ",
            "",
            "visual_tests/../../outside",
        ] {
            assert!(workspace.resolve(Path::new(path)).is_err(), "{path}");
        }
        for name in ["../escape", "CON", "all", "a/b", "*.tests.yaml"] {
            assert!(suite_path(name).is_err(), "{name}");
        }
        assert!(
            workspace
                .resolve(Path::new("common/my settings.yaml"))
                .is_ok()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escapes_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        std::os::unix::fs::symlink(external.path(), directory.path().join("outside")).unwrap();
        assert!(workspace.resolve(Path::new("outside/file.yaml")).is_err());
    }

    #[test]
    fn preferences_round_trip_and_bound_recent_workspaces() {
        let directory = tempfile::tempdir().unwrap();
        let mut preferences = Preferences::default();
        for index in 0..15 {
            preferences.remember(&Workspace {
                root: PathBuf::from(format!("workspace-{index}")),
            });
        }
        let workspace = Workspace {
            root: PathBuf::from("workspace-10"),
        };
        preferences.remember(&workspace);
        assert_eq!(preferences.recent_workspaces.len(), 10);
        assert_eq!(preferences.recent_workspaces[0], workspace.root);
        let path = directory.path().join("settings.json");
        preferences.navigator_hidden = true;
        preferences.save(&path).unwrap();
        assert!(Preferences::load(&path).unwrap().navigator_hidden);
        let legacy: Preferences = serde_json::from_str(r#"{"darkMode":true}"#).unwrap();
        assert!(legacy.dark_mode);
        assert!(!legacy.navigator_hidden);
        assert_eq!(
            Preferences::load(&path).unwrap().recent_workspaces,
            preferences.recent_workspaces
        );
        fs::write(&path, "invalid").unwrap();
        assert!(Preferences::load(&path).is_err());
    }

    #[test]
    fn picked_files_use_the_correct_base_directory() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        fs::create_dir_all(directory.path().join("visual_tests/uploads")).unwrap();
        let upload = directory.path().join("visual_tests/uploads/sample.txt");
        fs::write(&upload, "sample").unwrap();
        assert_eq!(
            workspace.relative_file(&upload, true).unwrap(),
            "uploads/sample.txt"
        );
        assert_eq!(
            workspace.relative_file(&upload, false).unwrap(),
            "visual_tests/uploads/sample.txt"
        );
        let other = outside.path().join("other.txt");
        fs::write(&other, "outside").unwrap();
        assert!(workspace.relative_file(&other, false).is_err());
        let root_file = directory.path().join("root.txt");
        fs::write(&root_file, "root").unwrap();
        assert!(workspace.relative_file(&root_file, true).is_err());
    }

    #[test]
    fn selected_suite_must_match_cli_format_precedence() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let json = workspace.create_suite("Shop.TESTS.JSON").unwrap();
        assert_eq!(json.document.format, Format::Json);
        assert_eq!(workspace.runnable_suite(&json.entry).unwrap(), "shop");
        let yaml = workspace.create_suite("shop.tests.yaml").unwrap();
        assert!(
            workspace
                .runnable_suite(&json.entry)
                .unwrap_err()
                .to_string()
                .contains("not the selected")
        );
        assert_eq!(workspace.runnable_suite(&yaml.entry).unwrap(), "shop");
        assert_eq!(workspace.entries().unwrap().len(), 2);
        assert!(suite_path("ALL").is_err());
        assert!(suite_path("All.TESTS.JSON").is_err());
        assert!(suite_path("all.tests.yaml").is_err());
    }

    #[test]
    fn multiple_suite_selection_uses_exact_names_and_output_ids() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        workspace.create_suite("shop").unwrap();
        let mut account = workspace.create_suite("account").unwrap();
        account
            .document
            .apply(Edit::Set {
                parent: String::new(),
                key: "id".into(),
                value: Some(json!("account-output")),
            })
            .unwrap();
        workspace.save(&mut account).unwrap();
        let suites = workspace.suite_catalog().unwrap();
        let mut selected = SuiteSelection::default();
        assert!(selected.filters(&suites, false).is_err());
        selected.select_all(&suites);
        assert_eq!(selected.len(), 2);
        assert_eq!(
            selected.filters(&suites, false).unwrap(),
            vec!["account", "shop"]
        );
        assert_eq!(
            selected.filters(&suites, true).unwrap(),
            vec!["account-output", "shop"]
        );
        selected.toggle(&suites[0], false).unwrap();
        assert_eq!(selected.filters(&suites, false).unwrap(), vec!["shop"]);
        selected.reconcile(&[]);
        assert!(selected.is_empty());
    }

    #[test]
    fn invalid_suites_are_visible_but_not_silently_selected() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let bad = workspace.create_suite("bad").unwrap();
        workspace.create_suite("good").unwrap();
        fs::write(workspace.resolve(&bad.entry.path).unwrap(), "scenarios: [").unwrap();
        let suites = workspace.suite_catalog().unwrap();
        assert_eq!(suites.len(), 2);
        assert!(suites[0].error.is_some());
        let mut selected = SuiteSelection::default();
        selected.select_all(&suites);
        assert_eq!(selected.len(), 1);
        assert!(selected.toggle(&suites[0], true).is_err());
        assert_eq!(selected.filters(&suites, false).unwrap(), vec!["good"]);
    }

    #[test]
    fn only_one_desktop_instance_holds_the_workspace_window_lease() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("desktop.lock");
        let instance = DesktopInstance::acquire(&path).unwrap();
        assert!(DesktopInstance::acquire(&path).is_err());
        drop(instance);
        assert!(DesktopInstance::acquire(&path).is_ok());
    }
}
