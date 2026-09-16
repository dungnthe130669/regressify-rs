//! Local completions use the same schema as guided fields and parse incomplete source.

use std::{collections::BTreeSet, ops::Range};

use anyhow::Result;
use tree_sitter::{Node, Parser};

use super::{
    document::{ACTIONS, DocumentKind, Format, MAX_DOCUMENT_BYTES},
    schema::{self, Field, FieldKind, Scope},
};

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub label: String,
    pub detail: String,
    pub description: String,
    pub replacement: Range<usize>,
    pub text: String,
    pub property: bool,
}

pub struct CompletionCatalog {
    fields: Vec<(Scope, Vec<Field>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Segment {
    Property(String),
    Item,
}

struct Location {
    path: Vec<Segment>,
    properties: BTreeSet<String>,
    property: bool,
}

impl CompletionCatalog {
    pub fn new() -> Result<Self> {
        let scopes = [
            Scope::Workspace,
            Scope::Suite,
            Scope::Scenario,
            Scope::Viewport,
        ]
        .into_iter()
        .chain(ACTIONS.iter().map(|name| Scope::Action((*name).into())));
        let fields = scopes
            .map(|scope| Ok((scope.clone(), schema::fields(&scope)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { fields })
    }

    pub fn complete(
        &self,
        kind: DocumentKind,
        format: Format,
        source: &str,
        cursor: usize,
    ) -> Vec<Suggestion> {
        if kind == DocumentKind::Data
            || source.len() > MAX_DOCUMENT_BYTES
            || !source.is_char_boundary(cursor)
        {
            return Vec::new();
        }
        let start = source[..cursor]
            .rfind(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .map_or(0, |offset| {
                offset + source[offset..].chars().next().map_or(0, char::len_utf8)
            });
        let end = source[cursor..]
            .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .map_or(source.len(), |offset| cursor + offset);
        let prefix = source[start..cursor].to_ascii_lowercase();
        let quoted = start
            .checked_sub(1)
            .and_then(|offset| source.as_bytes().get(offset))
            .copied()
            .filter(|quote| matches!(quote, b'\'' | b'"'));
        let replacement = if let Some(quote) = quoted {
            start - 1..end + usize::from(source.as_bytes().get(end) == Some(&quote))
        } else {
            start..end
        };
        let colon = source[replacement.end..]
            .trim_start_matches([' ', '\t'])
            .starts_with(':');
        let mut marker = "__regressify_completion__".to_string();
        while source.contains(&marker) {
            marker.push('_');
        }
        let mut parser = Parser::new();
        let language = match format {
            Format::Yaml => tree_sitter_yaml::LANGUAGE.into(),
            Format::Json => tree_sitter_json::LANGUAGE.into(),
        };
        if parser.set_language(&language).is_err() {
            return Vec::new();
        }
        // A temporary scalar makes the cursor discoverable even before a key has its colon.
        for property in [true, false] {
            let mut token = format!(
                "\"{marker}\"{}",
                if property && !colon { ": null" } else { "" }
            );
            if format == Format::Json
                && !colon
                && source[replacement.end..].trim_start().starts_with('"')
            {
                token.push(',');
            }
            let mut probe = source.to_string();
            probe.replace_range(replacement.clone(), &token);
            let Some(mut tree) = parser.parse(&probe, None) else {
                return Vec::new();
            };
            if format == Format::Json && tree.root_node().has_error() {
                let mut closing = Vec::new();
                collect_delimiters(tree.root_node(), &mut closing, 0);
                for delimiter in closing.into_iter().rev() {
                    probe.push_str(delimiter);
                }
                if let Some(completed) = parser.parse(&probe, None) {
                    tree = completed;
                }
            }
            let Some(location) = locate(tree.root_node(), &probe, &marker, &[], 0) else {
                continue;
            };
            if location.property != property {
                continue;
            }
            let mut path = location.path.clone();
            let key = if property {
                None
            } else {
                if path.last() == Some(&Segment::Item) {
                    path.pop();
                }
                match path.pop() {
                    Some(Segment::Property(key)) => Some(key),
                    _ => None,
                }
            };
            let Some(scope) = scope(kind, &path, &location.properties) else {
                continue;
            };
            let fields = self.scope_fields(&scope, &location.properties);
            if property {
                return fields
                    .into_iter()
                    .filter(|field| {
                        field.key.to_ascii_lowercase().starts_with(&prefix)
                            && !location.properties.contains(&field.key)
                    })
                    .map(|field| {
                        let key = if format == Format::Json || quoted.is_some() {
                            format!("\"{}\"", field.key)
                        } else {
                            field.key.clone()
                        };
                        Suggestion {
                            label: field.key.clone(),
                            detail: field.label.clone(),
                            description: field.description.clone(),
                            replacement: replacement.clone(),
                            text: format!("{key}{}", if colon { "" } else { ": " }),
                            property: true,
                        }
                    })
                    .collect();
            }
            if let Some(field) = fields.iter().find(|field| Some(&field.key) == key.as_ref()) {
                let values = match &field.kind {
                    FieldKind::Boolean => vec!["false".into(), "true".into()],
                    FieldKind::Choice(choices) => choices.clone(),
                    _ => Vec::new(),
                };
                return values
                    .into_iter()
                    .filter(|value| value.to_ascii_lowercase().starts_with(&prefix))
                    .map(|value| Suggestion {
                        text: if matches!(field.kind, FieldKind::Boolean) {
                            value.clone()
                        } else {
                            format!("\"{value}\"")
                        },
                        label: value,
                        detail: field.label.clone(),
                        description: field.description.clone(),
                        replacement: replacement.clone(),
                        property: false,
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    fn scope_fields(&self, scope: &Scope, properties: &BTreeSet<String>) -> Vec<Field> {
        let mut fields = if scope == &Scope::Action(String::new()) {
            self.fields
                .iter()
                .filter(|(scope, _)| matches!(scope, Scope::Action(_)))
                .flat_map(|(scope, fields)| {
                    fields.iter().filter(
                        move |field| matches!(scope, Scope::Action(name) if name == &field.key),
                    )
                })
                .cloned()
                .collect()
        } else {
            self.fields
                .iter()
                .find(|(candidate, _)| candidate == scope)
                .map(|(_, fields)| fields.clone())
                .unwrap_or_default()
        };
        let collection = match scope {
            Scope::Suite => Some(("scenarios", "Scenarios")),
            Scope::Scenario => Some(("actions", "Actions")),
            _ => None,
        };
        if let Some((key, label)) = collection
            && !properties.contains(key)
        {
            fields.push(Field {
                key: key.into(),
                label: label.into(),
                description: String::new(),
                kind: FieldKind::Structured,
                required: false,
            });
        }
        fields
    }
}

fn scope(kind: DocumentKind, path: &[Segment], properties: &BTreeSet<String>) -> Option<Scope> {
    use Segment::{Item, Property};
    match (kind, path) {
        (DocumentKind::Workspace, []) => Some(Scope::Workspace),
        (DocumentKind::Suite, []) => Some(Scope::Suite),
        (DocumentKind::Suite, [Property(scenarios), Item]) if scenarios == "scenarios" => {
            Some(Scope::Scenario)
        }
        (DocumentKind::Suite, [Property(scenarios), Item, Property(actions), Item])
            if scenarios == "scenarios" && actions == "actions" =>
        {
            Some(Scope::Action(
                ACTIONS
                    .iter()
                    .find(|name| properties.contains(**name))
                    .copied()
                    .unwrap_or_default()
                    .into(),
            ))
        }
        (DocumentKind::Viewports, [Item]) => Some(Scope::Viewport),
        _ => None,
    }
}

fn scalar(node: Node<'_>, source: &str) -> Option<String> {
    serde_yaml::from_str::<String>(node.utf8_text(source.as_bytes()).ok()?).ok()
}

fn collect_delimiters(node: Node<'_>, closing: &mut Vec<&'static str>, depth: usize) {
    if depth > 128 || node.is_missing() {
        return;
    }
    match node.kind() {
        "{" => closing.push("}"),
        "[" => closing.push("]"),
        "}" | "]" if closing.last().copied() == Some(node.kind()) => {
            closing.pop();
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_delimiters(child, closing, depth + 1);
    }
}

fn locate(
    node: Node<'_>,
    source: &str,
    marker: &str,
    path: &[Segment],
    depth: usize,
) -> Option<Location> {
    if depth > 128 || matches!(node.kind(), "comment" | "block_scalar") {
        return None;
    }
    let mut cursor = node.walk();
    if matches!(node.kind(), "object" | "block_mapping" | "flow_mapping") {
        let pairs = node
            .named_children(&mut cursor)
            .filter(|child| matches!(child.kind(), "pair" | "block_mapping_pair" | "flow_pair"))
            .collect::<Vec<_>>();
        let properties = pairs
            .iter()
            .filter_map(|pair| scalar(pair.child_by_field_name("key")?, source))
            .filter(|key| key != marker)
            .collect::<BTreeSet<_>>();
        for pair in pairs {
            let Some(key) = pair
                .child_by_field_name("key")
                .and_then(|key| scalar(key, source))
            else {
                continue;
            };
            if key == marker {
                return Some(Location {
                    path: path.to_vec(),
                    properties,
                    property: true,
                });
            }
            if let Some(value) = pair.child_by_field_name("value") {
                let mut nested = path.to_vec();
                nested.push(Segment::Property(key));
                if let Some(mut location) = locate(value, source, marker, &nested, depth + 1) {
                    if !location.property
                        && (location.path.len() == nested.len()
                            || location.path.len() == nested.len() + 1)
                    {
                        location.properties = properties;
                    }
                    return Some(location);
                }
            }
        }
        return None;
    }
    if matches!(node.kind(), "array" | "block_sequence" | "flow_sequence") {
        let mut nested = path.to_vec();
        nested.push(Segment::Item);
        return node
            .named_children(&mut cursor)
            .find_map(|child| locate(child, source, marker, &nested, depth + 1));
    }
    if matches!(
        node.kind(),
        "string" | "double_quote_scalar" | "single_quote_scalar" | "plain_scalar"
    ) && scalar(node, source).as_deref() == Some(marker)
    {
        return Some(Location {
            path: path.to_vec(),
            properties: BTreeSet::new(),
            property: false,
        });
    }
    node.named_children(&mut cursor)
        .find_map(|child| locate(child, source, marker, path, depth + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggestions(source: &str, format: Format) -> Vec<Suggestion> {
        let cursor = source.find('|').unwrap();
        let source = source.replacen('|', "", 1);
        CompletionCatalog::new()
            .unwrap()
            .complete(DocumentKind::Suite, format, &source, cursor)
    }

    #[test]
    fn yaml_properties_follow_suite_scenario_and_action_context() {
        assert!(
            suggestions("asyncC|", Format::Yaml)
                .iter()
                .any(|item| item.label == "asyncCaptureLimit")
        );
        let scenario = suggestions("scenarios:\n  - url: /\n    del|", Format::Yaml);
        assert_eq!(scenario[0].label, "delay");
        let action = suggestions(
            "scenarios:\n  - url: /\n    actions:\n      - cli|",
            Format::Yaml,
        );
        assert_eq!(action[0].label, "click");
        let properties = suggestions(
            "scenarios:\n- url: /\n  actions:\n  - click: '#pay'\n    fr|",
            Format::Yaml,
        );
        assert_eq!(properties[0].label, "frame");
        assert!(
            !suggestions("scenarios: []\n|", Format::Yaml)
                .iter()
                .any(|item| item.label == "scenarios")
        );
    }

    #[test]
    fn json_properties_and_values_have_safe_replacement_ranges() {
        let source = "{\"scenarios\":[{\"url\":\"/\", \"del|\": 12}]}";
        let items = suggestions(source, Format::Json);
        let item = items.iter().find(|item| item.label == "delay").unwrap();
        let mut edited = source.replace('|', "");
        edited.replace_range(item.replacement.clone(), &item.text);
        let parsed: serde_json::Value = serde_json::from_str(&edited).unwrap();
        assert_eq!(parsed["scenarios"][0]["delay"], 12);
        assert_eq!(
            suggestions("{\"debug\": tr|}", Format::Json)[0].text,
            "true"
        );
        assert_eq!(
            suggestions("browser: chr|", Format::Yaml)[0].text,
            "\"chromium\""
        );
    }

    #[test]
    fn unfinished_json_and_yaml_flow_objects_keep_their_scope() {
        assert!(
            suggestions("{\n  \"deb|\"\n  \"scenarios\": []\n}", Format::Json)
                .iter()
                .any(|item| item.label == "debug")
        );
        assert_eq!(
            suggestions("{\n  \"debug\": fal|\n  \"scenarios\": []\n}", Format::Json)[0].text,
            "false"
        );
        for source in [
            "{\"scenarios\":[{\"url\":\"/\", \"del|",
            "{\"scenarios\":[{\"url\":\"/\", |}]}",
        ] {
            assert!(
                suggestions(source, Format::Json)
                    .iter()
                    .any(|item| item.label == "delay"),
                "{source}"
            );
        }
        assert!(
            suggestions("scenarios: [{url: '/', del|}]", Format::Yaml)
                .iter()
                .any(|item| item.label == "delay")
        );
        assert!(
            !suggestions("asyncCaptureLimit: 5\nasyncC|", Format::Yaml)
                .iter()
                .any(|item| item.label == "asyncCaptureLimit")
        );
    }

    #[test]
    fn workspace_and_viewport_fields_use_their_own_schema() {
        let catalog = CompletionCatalog::new().unwrap();
        assert!(
            catalog
                .complete(DocumentKind::Workspace, Format::Yaml, "asyncC", 6)
                .iter()
                .any(|item| item.label == "asyncCaptureLimit")
        );
        assert!(
            catalog
                .complete(DocumentKind::Viewports, Format::Yaml, "- wid", 5)
                .iter()
                .any(|item| item.label == "width")
        );
        assert!(
            catalog
                .complete(DocumentKind::Data, Format::Yaml, "", 0)
                .is_empty()
        );
        assert!(
            catalog
                .complete(DocumentKind::Suite, Format::Yaml, "\u{e9}", 1)
                .is_empty()
        );
    }

    #[test]
    fn comments_free_text_and_non_schema_objects_do_not_offer_properties() {
        for source in [
            "# del|",
            "scenarios:\n- url: del|",
            "scenarios:\n- url: '/'\n  label: \"del|\"",
            "unknown:\n  del|",
        ] {
            assert!(suggestions(source, Format::Yaml).is_empty(), "{source}");
        }
    }

    #[test]
    fn unicode_crlf_and_midword_edits_preserve_surrounding_source() {
        let source = "# \u{1f30d}\r\nscenarios:\r\n- url: '/'\r\n  de|lay: 12\r\n";
        let item = suggestions(source, Format::Yaml)
            .into_iter()
            .find(|item| item.label == "delay")
            .unwrap();
        let mut edited = source.replace('|', "");
        edited.replace_range(item.replacement, &item.text);
        assert_eq!(edited, source.replace('|', ""));
    }
}
