//! Source-preserving document edits shared by guided forms and the source editor.

use std::{collections::VecDeque, path::Path, str::FromStr};

use anyhow::{Context, Result, anyhow, ensure};
use serde_json::{Value, json};
use yaml_edit::{AsYaml, Document as YamlDocument, Sequence, SyntaxKind, YamlFile, YamlNode};

use crate::{
    config::{WorkspaceConfig, replacements::validate_replacements, viewports},
    types::{Browser, TestSuite},
};

pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_HISTORY_BYTES: usize = 16 * 1024 * 1024;
const MAX_HISTORY_ENTRIES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Workspace,
    Suite,
    Viewports,
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Yaml,
    Json,
}

impl Format {
    pub fn for_path(path: &Path) -> Self {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            Self::Json
        } else {
            Self::Yaml
        }
    }

    pub fn parse(self, source: &str) -> Result<Value> {
        ensure!(
            source.len() <= MAX_DOCUMENT_BYTES,
            "Document exceeds the 4 MiB editing limit"
        );
        match self {
            Self::Json => serde_json::from_str(source).context("Invalid JSON"),
            Self::Yaml => serde_yaml::from_str(source).context("Invalid YAML"),
        }
    }

    pub fn serialize(self, value: &Value) -> Result<String> {
        match self {
            Self::Json => Ok(format!("{}\n", serde_json::to_string_pretty(value)?)),
            Self::Yaml => Ok(serde_yaml::to_string(value)?),
        }
    }
}

/// JSON-pointer paths avoid ambiguity for selectors and keys containing dots.
#[derive(Clone, Debug)]
pub enum Edit {
    Set {
        parent: String,
        key: String,
        value: Option<Value>,
    },
    Insert {
        path: String,
        index: usize,
        value: Value,
    },
    Remove {
        path: String,
        index: usize,
    },
    Move {
        path: String,
        from: usize,
        to: usize,
    },
}

#[derive(Debug)]
pub struct Document {
    pub kind: DocumentKind,
    pub format: Format,
    source: String,
    saved: String,
    undo: VecDeque<String>,
    redo: Vec<String>,
}

impl Document {
    pub fn new(kind: DocumentKind, format: Format, source: String) -> Result<Self> {
        ensure!(
            source.len() <= MAX_DOCUMENT_BYTES,
            "Document exceeds the 4 MiB editing limit"
        );
        Ok(Self {
            kind,
            format,
            saved: source.clone(),
            source,
            undo: VecDeque::new(),
            redo: Vec::new(),
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn saved_source(&self) -> &str {
        &self.saved
    }
    pub fn dirty(&self) -> bool {
        self.source != self.saved
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn value(&self) -> Result<Value> {
        self.format.parse(&self.source)
    }

    pub fn validate(&self) -> Result<()> {
        validate_value(self.kind, &self.value()?)
    }

    /// Invalid drafts are editable, but validation remains a mandatory save gate.
    pub fn set_source(&mut self, source: String) -> Result<()> {
        ensure!(
            source.len() <= MAX_DOCUMENT_BYTES,
            "Document exceeds the 4 MiB editing limit"
        );
        if source == self.source {
            return Ok(());
        }
        self.undo
            .push_back(std::mem::replace(&mut self.source, source));
        self.redo.clear();
        while self.undo.len() > MAX_HISTORY_ENTRIES
            || self.undo.iter().map(String::len).sum::<usize>() > MAX_HISTORY_BYTES
        {
            self.undo.pop_front();
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop_back() {
            self.redo
                .push(std::mem::replace(&mut self.source, previous));
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo
                .push_back(std::mem::replace(&mut self.source, next));
        }
    }

    pub fn mark_saved(&mut self) {
        self.saved.clone_from(&self.source);
    }

    /// An asynchronous write may finish after a newer source edit was queued.
    pub fn acknowledge_saved(&mut self, source: String) {
        self.saved = source;
    }

    pub fn apply(&mut self, edit: Edit) -> Result<()> {
        let mut expected = self.value()?;
        apply_value(&mut expected, &edit)?;
        let source = match self.format {
            Format::Json => self.format.serialize(&expected)?,
            Format::Yaml => apply_yaml(&self.source, &edit)?,
        };
        // A CST editor must never silently change the meaning of unrelated YAML.
        ensure!(
            self.format.parse(&source)? == expected,
            "This YAML construct cannot be edited losslessly. Use the Source editor instead."
        );
        self.set_source(source)
    }

    /// A user command may need several structural edits but only one undo entry.
    pub fn apply_many(&mut self, edits: Vec<Edit>) -> Result<()> {
        let mut draft = Self::new(self.kind, self.format, self.source.clone())?;
        for edit in edits {
            draft.apply(edit)?;
        }
        self.set_source(draft.source)
    }
}

fn array_mut<'value>(value: &'value mut Value, path: &str) -> Result<&'value mut Vec<Value>> {
    value
        .pointer_mut(path)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("Expected a list at '{path}'"))
}

fn apply_value(value: &mut Value, edit: &Edit) -> Result<()> {
    match edit {
        Edit::Set {
            parent,
            key,
            value: replacement,
        } => {
            let mapping = value
                .pointer_mut(parent)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| anyhow!("Expected an object at '{parent}'"))?;
            if let Some(replacement) = replacement {
                mapping.insert(key.clone(), replacement.clone());
            } else {
                mapping.remove(key);
            }
        }
        Edit::Insert {
            path,
            index,
            value: item,
        } => {
            let items = array_mut(value, path)?;
            ensure!(*index <= items.len(), "List insertion is out of bounds");
            items.insert(*index, item.clone());
        }
        Edit::Remove { path, index } => {
            let items = array_mut(value, path)?;
            ensure!(*index < items.len(), "List removal is out of bounds");
            items.remove(*index);
        }
        Edit::Move { path, from, to } => {
            let items = array_mut(value, path)?;
            ensure!(
                *from < items.len() && *to < items.len(),
                "List move is out of bounds"
            );
            let item = items.remove(*from);
            items.insert(*to, item);
        }
    }
    Ok(())
}

fn root_node(document: &YamlDocument) -> Result<YamlNode> {
    document
        .as_mapping()
        .map(YamlNode::Mapping)
        .or_else(|| document.as_sequence().map(YamlNode::Sequence))
        .or_else(|| document.as_scalar().map(YamlNode::Scalar))
        .ok_or_else(|| anyhow!("Unsupported YAML root; use the Source editor"))
}

fn yaml_node(value: &Value) -> Result<YamlNode> {
    // JSON flow nodes are valid YAML and retain their meaning at any indentation.
    // Existing CST nodes stay untouched; only newly inserted values use this form.
    root_node(&YamlDocument::from_str(&serde_json::to_string(value)?)?)
}

fn locate(root: YamlNode, path: &str) -> Result<YamlNode> {
    let mut node = root;
    for segment in path.split('/').skip(1) {
        let key = segment.replace("~1", "/").replace("~0", "~");
        node = match &node {
            YamlNode::Mapping(mapping) => mapping.get(key.as_str()),
            YamlNode::Sequence(sequence) => key.parse().ok().and_then(|index| sequence.get(index)),
            _ => None,
        }
        .ok_or_else(|| anyhow!("Cannot resolve '{path}' in YAML; use the Source editor"))?;
    }
    Ok(node)
}

fn apply_yaml(source: &str, edit: &Edit) -> Result<String> {
    // Keep the stream wrapper: leading comments can belong to it, not the document.
    let file = YamlFile::from_str(source)?;
    let document = file
        .documents()
        .next()
        .ok_or_else(|| anyhow!("Empty YAML document"))?;
    let root = root_node(&document)?;
    match edit {
        Edit::Set { parent, key, value } => {
            let node = locate(root, parent)?;
            let mapping = node
                .as_mapping()
                .ok_or_else(|| anyhow!("Expected a YAML mapping"))?;
            match value {
                Some(value) => {
                    mapping.set(key.as_str(), yaml_node(value)?);
                }
                None => {
                    mapping.remove(key.as_str());
                }
            }
        }
        Edit::Insert { path, index, value } => {
            let node = locate(root, path)?;
            let sequence = node
                .as_sequence()
                .ok_or_else(|| anyhow!("Expected a YAML sequence"))?;
            insert_yaml_item(sequence, *index, yaml_node(value)?, source)?;
        }
        Edit::Remove { path, index } => {
            let node = locate(root, path)?;
            node.as_sequence()
                .ok_or_else(|| anyhow!("Expected a YAML sequence"))?
                .remove(*index);
        }
        Edit::Move { path, from, to } => {
            let node = locate(root, path)?;
            let sequence = node
                .as_sequence()
                .ok_or_else(|| anyhow!("Expected a YAML sequence"))?;
            let mut items = sequence.values().collect::<Vec<_>>();
            let item = items.remove(*from);
            items.insert(*to, item);
            // Keep the existing separators, including indentationless YAML lists.
            for (index, item) in items.into_iter().enumerate() {
                ensure!(sequence.set(index, item), "Unable to reorder YAML item");
            }
        }
    }
    Ok(file.to_string())
}

fn insert_yaml_item(
    sequence: &Sequence,
    index: usize,
    value: YamlNode,
    source: &str,
) -> Result<()> {
    let syntax = sequence
        .as_node()
        .ok_or_else(|| anyhow!("Missing sequence syntax"))?;
    let indentationless = syntax
        .children()
        .find(|node| node.kind() == SyntaxKind::SEQUENCE_ENTRY)
        .and_then(|entry| {
            entry
                .children_with_tokens()
                .filter_map(|child| child.into_token())
                .find(|token| token.kind() == SyntaxKind::DASH)
        })
        .is_some_and(|dash| {
            let offset: usize = dash.text_range().start().into();
            offset == 0 || source.as_bytes().get(offset - 1) == Some(&b'\n')
        });
    sequence.insert(index, value);
    if indentationless {
        // yaml-edit 0.3.1 adds two spaces to inserts into column-zero lists.
        // Repair only entry separators, leaving values and comments untouched.
        let children = syntax.children_with_tokens().collect::<Vec<_>>();
        for position in (0..children.len().saturating_sub(1)).rev() {
            if children[position]
                .as_token()
                .is_some_and(|token| token.kind() == SyntaxKind::INDENT)
                && children[position + 1]
                    .as_node()
                    .is_some_and(|node| node.kind() == SyntaxKind::SEQUENCE_ENTRY)
            {
                syntax.splice_children(position..position + 1, vec![]);
            }
        }
    }
    Ok(())
}

pub fn validate_value(kind: DocumentKind, value: &Value) -> Result<()> {
    match kind {
        DocumentKind::Workspace => {
            let config: WorkspaceConfig = serde_json::from_value(value.clone())?;
            ensure!(
                config
                    .browser
                    .is_none_or(|browser| browser == Browser::Chromium),
                "Only Chromium is supported"
            );
            if let Some(rules) = config.url_replacements {
                validate_replacements(&rules, "workspace")?;
            }
        }
        DocumentKind::Suite => {
            let suite: TestSuite = serde_json::from_value(value.clone())?;
            ensure!(
                suite
                    .browser
                    .is_none_or(|browser| browser == Browser::Chromium),
                "Only Chromium is supported"
            );
            if let Some(rules) = suite.url_replacements {
                validate_replacements(&rules, "suite")?;
            }
            for (index, scenario) in suite.scenarios.iter().enumerate() {
                ensure!(
                    !scenario.url.trim().is_empty(),
                    "Scenario {} needs a URL",
                    index + 1
                );
            }
        }
        DocumentKind::Viewports => {
            let definitions = serde_json::from_value::<Vec<viewports::ViewportDef>>(value.clone())?;
            viewports::validate_viewports(&definitions, Path::new("viewport editor"))?;
        }
        DocumentKind::Data => ensure!(
            value.is_object() || value.is_array(),
            "Expected a mapping or a list"
        ),
    }
    validate_numbers(value)
}

fn validate_numbers(value: &Value) -> Result<()> {
    if let Some(mapping) = value.as_object() {
        for (key, value) in mapping {
            if !value.is_null() {
                if key == "misMatchThreshold" {
                    ensure!(
                        value
                            .as_f64()
                            .is_some_and(|number| (0.0..=100.0).contains(&number)),
                        "misMatchThreshold must be between 0 and 100"
                    );
                }
                if matches!(key.as_str(), "asyncCaptureLimit" | "asyncCompareLimit") {
                    ensure!(
                        value.as_u64().is_some_and(|number| number > 0),
                        "{key} must be a positive integer"
                    );
                }
                if matches!(key.as_str(), "delay" | "postInteractionWait") {
                    ensure!(
                        value.as_f64().is_some_and(|number| number >= 0.0),
                        "{key} cannot be negative"
                    );
                }
            }
            validate_numbers(value)?;
        }
    } else if let Some(items) = value.as_array() {
        for item in items {
            validate_numbers(item)?;
        }
    }
    Ok(())
}

pub const ACTIONS: &[&str] = &[
    "check", "click", "focus", "goto", "hide", "hover", "input", "persist", "press", "remove",
    "save", "select", "scroll", "uncheck", "wait",
];

pub fn action_template(action: &str) -> Result<Value> {
    ensure!(ACTIONS.contains(&action), "Unknown action '{action}'");
    Ok(match action {
        "wait" => json!({"wait": 1000}),
        "goto" => json!({"goto": "https://example.com"}),
        "press" => json!({"press": "#element", "key": "Enter"}),
        "input" => json!({"input": "#element", "value": ""}),
        "select" => json!({"select": "#element", "value": "option"}),
        "save" => json!({"save": "checkpoint"}),
        "persist" => json!({"persist": "session"}),
        selector => json!({selector: "#element"}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suite(source: &str) -> Document {
        Document::new(DocumentKind::Suite, Format::Yaml, source.to_string()).unwrap()
    }

    #[test]
    fn nested_edit_preserves_comments_and_untouched_formatting() {
        let source = "# QA notes\nscenarios:\n  - url: 'https://example.com' # keep URL\n    label: Home\n    actions:\n      - click: '#old' # target\n";
        let mut document = suite(source);
        document
            .apply(Edit::Set {
                parent: "/scenarios/0/actions/0".into(),
                key: "click".into(),
                value: Some(json!("#new")),
            })
            .unwrap();
        assert!(document.source().contains("# QA notes"));
        assert!(
            document
                .source()
                .contains("'https://example.com' # keep URL")
        );
        assert!(document.source().contains("# target"));
        assert_eq!(
            document.value().unwrap()["scenarios"][0]["actions"][0]["click"],
            "#new"
        );
        document.validate().unwrap();
        document.undo();
        assert_eq!(document.source(), source);
        assert!(!document.dirty());
        document.redo();
        assert!(document.dirty());
    }

    #[test]
    fn list_operations_preserve_semantics_and_bounds() {
        for format in [Format::Yaml, Format::Json] {
            let value = json!({"scenarios": [{"url": "https://one"}, {"url": "https://two"}]});
            let mut document = Document::new(
                DocumentKind::Suite,
                format,
                format.serialize(&value).unwrap(),
            )
            .unwrap();
            document
                .apply(Edit::Move {
                    path: "/scenarios".into(),
                    from: 0,
                    to: 1,
                })
                .unwrap();
            assert_eq!(
                document.value().unwrap()["scenarios"][0]["url"],
                "https://two"
            );
            document
                .apply(Edit::Insert {
                    path: "/scenarios".into(),
                    index: 1,
                    value: json!({"url": "https://three"}),
                })
                .unwrap();
            document
                .apply(Edit::Remove {
                    path: "/scenarios".into(),
                    index: 0,
                })
                .unwrap();
            assert_eq!(
                document.value().unwrap()["scenarios"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            let before = document.source().to_string();
            assert!(
                document
                    .apply(Edit::Remove {
                        path: "/scenarios".into(),
                        index: 20
                    })
                    .is_err()
            );
            assert_eq!(document.source(), before);
        }
    }

    #[test]
    fn empty_sequences_can_receive_first_item() {
        let mut document = suite("scenarios: []\n");
        document
            .apply(Edit::Insert {
                path: "/scenarios".into(),
                index: 0,
                value: json!({"url": "https://example.com"}),
            })
            .unwrap();
        document.validate().unwrap();
    }

    #[test]
    fn explicit_false_and_empty_lists_are_not_inherited() {
        let mut document = suite("debug: true\nscenarios: []\n");
        document
            .apply(Edit::Set {
                parent: "".into(),
                key: "debug".into(),
                value: Some(json!(false)),
            })
            .unwrap();
        assert_eq!(document.value().unwrap()["debug"], false);
        document
            .apply(Edit::Set {
                parent: "".into(),
                key: "hideSelectors".into(),
                value: Some(json!([])),
            })
            .unwrap();
        assert_eq!(document.value().unwrap()["hideSelectors"], json!([]));
        document
            .apply(Edit::Set {
                parent: "".into(),
                key: "debug".into(),
                value: None,
            })
            .unwrap();
        assert!(document.value().unwrap().get("debug").is_none());
    }

    #[test]
    fn invalid_source_is_retained_but_not_validated() {
        let mut document = suite("scenarios: []\n");
        document.set_source("scenarios: [".into()).unwrap();
        assert!(document.dirty());
        assert!(document.validate().is_err());
        document.undo();
        assert!(document.validate().is_ok());
    }

    #[test]
    fn every_action_template_uses_the_existing_serde_contract() {
        for action in ACTIONS {
            let template = action_template(action).unwrap();
            serde_json::from_value::<crate::types::TestAction>(template.clone()).unwrap();
            let value =
                json!({"scenarios": [{"url": "https://example.com", "actions": [template]}]});
            validate_value(DocumentKind::Suite, &value).unwrap();
        }
        assert!(action_template("unknown").is_err());
    }

    #[test]
    fn validation_rejects_unknown_properties_and_unsafe_values() {
        for value in [
            json!({"debugg": true}),
            json!({"asyncCaptureLimit": 0}),
            json!({"browser": "firefox"}),
            json!({"misMatchThreshold": 101}),
            json!({"delay": -1}),
        ] {
            assert!(
                validate_value(DocumentKind::Workspace, &value).is_err(),
                "{value}"
            );
        }
        assert!(
            validate_value(
                DocumentKind::Viewports,
                &json!([{"label": "phone", "width": 0, "height": 600}])
            )
            .is_err()
        );
    }

    #[test]
    fn history_is_bounded_and_new_edits_discard_redo() {
        let mut document = suite("scenarios: []\n");
        for index in 0..150 {
            document
                .set_source(format!("# {index}\nscenarios: []\n"))
                .unwrap();
        }
        assert_eq!(document.undo.len(), MAX_HISTORY_ENTRIES);
        document.undo();
        assert!(document.can_redo());
        document.set_source("scenarios: []\n".into()).unwrap();
        assert!(!document.can_redo());
        assert!(
            document
                .set_source(" ".repeat(MAX_DOCUMENT_BYTES + 1))
                .is_err()
        );
    }

    #[test]
    fn multiple_documents_are_not_silently_truncated() {
        let document = suite("scenarios: []\n---\nscenarios: []\n");
        assert!(document.value().is_err());
    }

    #[test]
    fn acknowledging_an_older_save_keeps_newer_edits_dirty() {
        let mut document = suite("scenarios: []\n");
        document
            .set_source("debug: true\nscenarios: []\n".into())
            .unwrap();
        let snapshot = document.source().to_string();
        document
            .set_source("debug: false\nscenarios: []\n".into())
            .unwrap();
        document.acknowledge_saved(snapshot.clone());
        assert!(document.dirty());
        assert_eq!(document.saved_source(), snapshot);
        assert!(document.source().contains("debug: false"));
    }
}
