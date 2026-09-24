//! Editor navigation and structural commands, independent of native widgets.

use std::path::Path;

use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};

use super::{
    document::{ACTIONS, DocumentKind, Edit, action_template},
    schema::{self, Scope},
    workspace::{Entry, OpenDocument, SuiteSummary, Workspace},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    Root,
    Scenario(usize),
    Action {
        scenario: usize,
        index: usize,
    },
    Viewport(usize),
}

impl Selection {
    pub fn path(self) -> String {
        match self {
            Self::Root => "".into(),
            Self::Scenario(index) => format!("/scenarios/{index}"),
            Self::Action { scenario, index } => format!("/scenarios/{scenario}/actions/{index}"),
            Self::Viewport(index) => format!("/{index}"),
        }
    }
    pub fn scenario(self) -> Option<usize> {
        match self {
            Self::Scenario(index) => Some(index),
            Self::Action { scenario, .. } => Some(scenario),
            _ => None,
        }
    }
    fn list_position(self) -> Option<(String, usize)> {
        match self {
            Self::Root => None,
            Self::Scenario(index) => Some(("/scenarios".into(), index)),
            Self::Action { scenario, index } => {
                Some((format!("/scenarios/{scenario}/actions"), index))
            }
            Self::Viewport(index) => Some(("".into(), index)),
        }
    }
}

#[derive(Default)]
pub struct Session {
    pub workspace: Option<Workspace>,
    pub entries: Vec<Entry>,
    pub open: Option<OpenDocument>,
    pub selection: Selection,
    pub suites: Vec<SuiteSummary>,
    pub suite_error: Option<String>,
}

impl Session {
    pub fn dirty(&self) -> bool {
        self.open.as_ref().is_some_and(|open| open.document.dirty())
    }

    pub fn connect(&mut self, root: &Path) -> Result<()> {
        ensure!(!self.dirty(), "Save or discard the current document first");
        let workspace = Workspace::open(root)?;
        crate::init::init_desktop_workspace(&workspace.root)?;
        let entries = workspace.entries()?;
        self.workspace = Some(workspace);
        self.entries = entries;
        self.open = None;
        self.selection = Selection::Root;
        self.refresh_catalog();
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.entries = self
            .workspace
            .as_ref()
            .ok_or_else(|| anyhow!("Open a workspace first"))?
            .entries()?;
        self.refresh_catalog();
        Ok(())
    }

    pub fn refresh_catalog(&mut self) {
        match self
            .workspace
            .as_ref()
            .map(Workspace::suite_catalog)
            .transpose()
        {
            Ok(suites) => {
                self.suites = suites.unwrap_or_default();
                self.suite_error = None;
            }
            Err(error) => {
                self.suites.clear();
                self.suite_error = Some(format!("{error:#}"));
            }
        }
    }

    pub fn close(&mut self) -> Result<()> {
        ensure!(!self.dirty(), "Save or discard the current document first");
        *self = Self::default();
        Ok(())
    }

    pub fn load(&mut self, entry: Entry) -> Result<()> {
        ensure!(!self.dirty(), "Save or discard the current document first");
        let document = self
            .workspace
            .as_ref()
            .ok_or_else(|| anyhow!("Open a workspace first"))?
            .open_document(entry)?;
        self.open = Some(document);
        self.selection = Selection::Root;
        Ok(())
    }

    pub fn install_document(&mut self, open: OpenDocument) -> Result<()> {
        ensure!(!self.dirty(), "Save or discard the current document first");
        self.open = Some(open);
        self.selection = Selection::Root;
        self.refresh()
    }

    pub fn save(&mut self) -> Result<()> {
        self.workspace
            .as_ref()
            .ok_or_else(|| anyhow!("Open a workspace first"))?
            .save(
                self.open
                    .as_mut()
                    .ok_or_else(|| anyhow!("Open a document first"))?,
            )
    }

    pub fn discard(&mut self) -> Result<()> {
        if let Some(open) = &mut self.open {
            let saved = open.document.saved_source().to_string();
            open.document.set_source(saved)?;
        }
        Ok(())
    }

    pub fn value(&self) -> Result<Value> {
        self.open
            .as_ref()
            .ok_or_else(|| anyhow!("Open a document first"))?
            .document
            .value()
    }

    pub fn scope(&self) -> Result<Option<Scope>> {
        let open = self
            .open
            .as_ref()
            .ok_or_else(|| anyhow!("Open a document first"))?;
        let value = open.document.value()?;
        let selected = value
            .pointer(&self.selection.path())
            .ok_or_else(|| anyhow!("Selection no longer exists"))?;
        Ok(match self.selection {
            Selection::Root => match open.entry.kind {
                DocumentKind::Workspace => Some(Scope::Workspace),
                DocumentKind::Suite => Some(Scope::Suite),
                _ => None,
            },
            Selection::Scenario(_) => Some(Scope::Scenario),
            Selection::Viewport(_) => Some(Scope::Viewport),
            Selection::Action { .. } => ACTIONS
                .iter()
                .find(|name| selected.get(**name).is_some())
                .map(|name| Scope::Action(name.to_string())),
        })
    }

    pub fn select(&mut self, selection: Selection) -> Result<()> {
        ensure!(
            self.value()?.pointer(&selection.path()).is_some(),
            "Selection no longer exists"
        );
        self.selection = selection;
        Ok(())
    }

    pub fn reconcile_selection(&mut self) {
        if self
            .value()
            .ok()
            .and_then(|value| value.pointer(&self.selection.path()).cloned())
            .is_none()
        {
            self.selection = Selection::Root;
        }
    }

    pub fn set_field(&mut self, key: &str, value: Option<Value>) -> Result<()> {
        let parent = self.selection.path();
        self.apply(vec![Edit::Set {
            parent,
            key: key.into(),
            value,
        }])
    }

    fn apply(&mut self, edits: Vec<Edit>) -> Result<()> {
        self.open
            .as_mut()
            .ok_or_else(|| anyhow!("Open a document first"))?
            .document
            .apply_many(edits)
    }

    pub fn add_scenario(&mut self) -> Result<()> {
        let value = self.value()?;
        let scenarios = value["scenarios"]
            .as_array()
            .ok_or_else(|| anyhow!("Open a test suite first"))?;
        let index = scenarios.len();
        let label = unique_label("Scenario", scenarios);
        self.apply(vec![Edit::Insert {
            path: "/scenarios".into(),
            index,
            value: json!({"label": label, "url": "https://example.com", "actions": []}),
        }])?;
        self.selection = Selection::Scenario(index);
        Ok(())
    }

    pub fn add_action(&mut self, action: &str) -> Result<()> {
        let scenario = self
            .selection
            .scenario()
            .ok_or_else(|| anyhow!("Select a scenario first"))?;
        let value = self.value()?;
        let parent = format!("/scenarios/{scenario}");
        let path = format!("{parent}/actions");
        let index = value
            .pointer(&path)
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let mut edits = Vec::new();
        if !value.pointer(&path).is_some_and(Value::is_array) {
            edits.push(Edit::Set {
                parent,
                key: "actions".into(),
                value: Some(json!([])),
            });
        }
        edits.push(Edit::Insert {
            path,
            index,
            value: action_template(action)?,
        });
        self.apply(edits)?;
        self.selection = Selection::Action { scenario, index };
        Ok(())
    }

    pub fn add_viewport(&mut self) -> Result<()> {
        let value = self.value()?;
        let viewports = value
            .as_array()
            .ok_or_else(|| anyhow!("Open viewport definitions first"))?;
        let index = viewports.len();
        let label = unique_label("Viewport", viewports);
        self.apply(vec![Edit::Insert {
            path: "".into(),
            index,
            value: json!({"label": label, "width": 1280, "height": 720}),
        }])?;
        self.selection = Selection::Viewport(index);
        Ok(())
    }

    pub fn duplicate_selected(&mut self) -> Result<()> {
        let (path, index) = self
            .selection
            .list_position()
            .ok_or_else(|| anyhow!("Select a scenario, action, or viewport"))?;
        let value = self.value()?;
        let items = value
            .pointer(&path)
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Missing collection"))?;
        let mut copy = items
            .get(index)
            .ok_or_else(|| anyhow!("Selection no longer exists"))?
            .clone();
        if !matches!(self.selection, Selection::Action { .. }) {
            let label = copy["label"].as_str().unwrap_or("Scenario");
            let label = unique_label(&format!("{label} copy"), items);
            if let Some(mapping) = copy.as_object_mut() {
                mapping.remove("id");
                mapping.insert("label".into(), json!(label));
            }
        }
        self.apply(vec![Edit::Insert {
            path,
            index: index + 1,
            value: copy,
        }])?;
        self.selection = match self.selection {
            Selection::Scenario(_) => Selection::Scenario(index + 1),
            Selection::Action { scenario, .. } => Selection::Action {
                scenario,
                index: index + 1,
            },
            Selection::Viewport(_) => Selection::Viewport(index + 1),
            Selection::Root => Selection::Root,
        };
        Ok(())
    }

    pub fn move_selected(&mut self, direction: isize) -> Result<()> {
        let (path, index) = self
            .selection
            .list_position()
            .ok_or_else(|| anyhow!("Select an item to move"))?;
        let target = index
            .checked_add_signed(direction)
            .ok_or_else(|| anyhow!("Already at the start of the list"))?;
        self.apply(vec![Edit::Move {
            path,
            from: index,
            to: target,
        }])?;
        self.selection = match self.selection {
            Selection::Scenario(_) => Selection::Scenario(target),
            Selection::Action { scenario, .. } => Selection::Action {
                scenario,
                index: target,
            },
            Selection::Viewport(_) => Selection::Viewport(target),
            Selection::Root => Selection::Root,
        };
        Ok(())
    }

    pub fn remove_selected(&mut self) -> Result<()> {
        let (path, index) = self
            .selection
            .list_position()
            .ok_or_else(|| anyhow!("Select an item to remove"))?;
        self.apply(vec![Edit::Remove { path, index }])?;
        self.selection = match self.selection {
            Selection::Action { scenario, .. } => Selection::Scenario(scenario),
            _ => Selection::Root,
        };
        Ok(())
    }

    pub fn workspace_value(&self) -> Result<Option<Value>> {
        if let Some(open) = &self.open
            && open.entry.kind == DocumentKind::Workspace
        {
            return open.document.value().map(Some);
        }
        let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.kind == DocumentKind::Workspace)
        else {
            return Ok(None);
        };
        self.workspace
            .as_ref()
            .ok_or_else(|| anyhow!("Open a workspace first"))?
            .open_document(entry.clone())?
            .document
            .value()
            .map(Some)
    }

    pub fn effective(&self) -> Result<Value> {
        let workspace = self.workspace_value()?;
        let suite = if self
            .open
            .as_ref()
            .is_some_and(|open| open.entry.kind == DocumentKind::Suite)
        {
            Some(self.value()?)
        } else {
            None
        };
        schema::effective(
            workspace.as_ref(),
            suite.as_ref(),
            self.selection.scenario(),
        )
    }
}

fn unique_label(base: &str, items: &[Value]) -> String {
    for suffix in 1..=items.len() + 1 {
        let candidate = format!("{base} {suffix}");
        if !items.iter().any(|item| {
            item["label"]
                .as_str()
                .is_some_and(|label| label.eq_ignore_ascii_case(&candidate))
        }) {
            return candidate;
        }
    }
    format!("{base} {}", uuid::Uuid::new_v4().simple())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connecting_creates_missing_runtime_files_without_editor_assets() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let mut session = Session::default();

        session.connect(root).unwrap();

        for path in [
            "regressify.yaml",
            "common/_viewports.yaml",
            "common/_cookies.yaml",
            "common/_override.css",
            "common/_on-ready.js",
            "common/_replacement-profiles.yaml",
        ] {
            assert!(root.join(path).is_file(), "Missing runtime file: {path}");
        }
        assert!(root.join("visual_tests").is_dir());
        assert!(!root.join(".vscode").exists());
        for path in [
            "common/test-schema.json",
            "common/regressify-schema.json",
            "common/replacement-profiles-schema.json",
        ] {
            assert!(!root.join(path).exists(), "Unexpected editor asset: {path}");
        }
        assert!(
            session
                .entries
                .iter()
                .any(|entry| entry.kind == DocumentKind::Workspace)
        );
        assert!(session.suites.is_empty());
        for entry in &session.entries {
            session
                .workspace
                .as_ref()
                .unwrap()
                .open_document(entry.clone())
                .unwrap()
                .document
                .validate()
                .unwrap();
        }
    }

    #[test]
    fn reconnecting_restores_only_missing_runtime_files() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let mut session = Session::default();
        session.connect(root).unwrap();
        let config = root.join("regressify.yaml");
        let cookies = root.join("common/_cookies.yaml");
        let stylesheet = root.join("common/_override.css");
        std::fs::write(&config, "# custom settings\ndelay: 75\n").unwrap();
        std::fs::write(&cookies, "# unfinished draft\n[\n").unwrap();
        let modified = std::fs::metadata(&config).unwrap().modified().unwrap();
        let original_stylesheet = std::fs::read(&stylesheet).unwrap();
        std::fs::remove_file(&stylesheet).unwrap();

        session.connect(root).unwrap();
        session.connect(root).unwrap();

        assert_eq!(std::fs::read(&stylesheet).unwrap(), original_stylesheet);
        assert_eq!(
            std::fs::read_to_string(&config).unwrap(),
            "# custom settings\ndelay: 75\n"
        );
        assert_eq!(
            std::fs::metadata(&config).unwrap().modified().unwrap(),
            modified
        );
        assert_eq!(
            std::fs::read_to_string(cookies).unwrap(),
            "# unfinished draft\n[\n"
        );
    }

    #[test]
    fn failed_automatic_setup_preserves_the_current_session() {
        let directory = tempfile::tempdir().unwrap();
        let blocked = tempfile::tempdir().unwrap();
        let mut session = session(directory.path());
        session.select(Selection::Scenario(0)).unwrap();
        let original = session.open.as_ref().unwrap().document.source().to_string();
        let entries: Vec<_> = session.entries.iter().map(Entry::label).collect();
        std::fs::write(blocked.path().join("common"), "keep this file").unwrap();

        let error = session.connect(blocked.path()).unwrap_err();

        assert!(format!("{error:#}").contains("common"));
        assert_eq!(
            session.workspace.as_ref().unwrap().root,
            directory.path().canonicalize().unwrap()
        );
        assert_eq!(session.open.as_ref().unwrap().document.source(), original);
        assert_eq!(session.selection, Selection::Scenario(0));
        assert_eq!(
            session.entries.iter().map(Entry::label).collect::<Vec<_>>(),
            entries
        );
        assert_eq!(
            std::fs::read_to_string(blocked.path().join("common")).unwrap(),
            "keep this file"
        );
    }

    fn session(root: &Path) -> Session {
        let mut session = Session::default();
        session.connect(root).unwrap();
        let open = session
            .workspace
            .as_ref()
            .unwrap()
            .create_suite("home")
            .unwrap();
        session.install_document(open).unwrap();
        session
    }

    #[test]
    fn every_action_can_be_added_edited_moved_and_removed() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = session(directory.path());
        session.select(Selection::Scenario(0)).unwrap();
        for action in ACTIONS {
            session.add_action(action).unwrap();
        }
        assert_eq!(
            session.value().unwrap()["scenarios"][0]["actions"]
                .as_array()
                .unwrap()
                .len(),
            ACTIONS.len()
        );
        session.move_selected(-1).unwrap();
        session.duplicate_selected().unwrap();
        session.remove_selected().unwrap();
        assert_eq!(session.selection, Selection::Scenario(0));
        session.save().unwrap();
    }

    #[test]
    fn adding_missing_actions_is_a_single_undoable_command() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = session(directory.path());
        session
            .open
            .as_mut()
            .unwrap()
            .document
            .set_source("scenarios:\n  - url: https://example.com\n".into())
            .unwrap();
        let before = session.open.as_ref().unwrap().document.source().to_string();
        session.select(Selection::Scenario(0)).unwrap();
        session.add_action("click").unwrap();
        session.open.as_mut().unwrap().document.undo();
        session.reconcile_selection();
        assert_eq!(session.open.as_ref().unwrap().document.source(), before);
        assert_eq!(session.selection, Selection::Root);
    }

    #[test]
    fn scenario_duplicates_drop_ids_and_keep_unique_labels() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = session(directory.path());
        session.select(Selection::Scenario(0)).unwrap();
        session.set_field("id", Some(json!("original"))).unwrap();
        session.duplicate_selected().unwrap();
        session.duplicate_selected().unwrap();
        session.add_scenario().unwrap();
        let value = session.value().unwrap();
        let scenarios = value["scenarios"].as_array().unwrap();
        assert_eq!(scenarios.len(), 4);
        assert!(scenarios[1].get("id").is_none());
        let labels = scenarios
            .iter()
            .map(|scenario| scenario["label"].as_str().unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(labels.len(), scenarios.len());
    }

    #[test]
    fn unsaved_work_blocks_navigation_and_bad_edits_are_atomic() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = session(directory.path());
        session.add_scenario().unwrap();
        assert!(session.connect(directory.path()).is_err());
        let other = tempfile::tempdir().unwrap();
        assert!(session.connect(other.path()).is_err());
        assert!(std::fs::read_dir(other.path()).unwrap().next().is_none());
        let before = session.open.as_ref().unwrap().document.source().to_string();
        assert!(
            session
                .apply(vec![
                    Edit::Set {
                        parent: "".into(),
                        key: "debug".into(),
                        value: Some(json!(true))
                    },
                    Edit::Remove {
                        path: "/scenarios".into(),
                        index: 100
                    }
                ])
                .is_err()
        );
        assert_eq!(session.open.as_ref().unwrap().document.source(), before);
        session.discard().unwrap();
        assert!(!session.dirty());
        session.connect(directory.path()).unwrap();
    }
}
