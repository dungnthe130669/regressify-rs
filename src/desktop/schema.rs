//! Derive field coverage from the CLI types instead of maintaining a second schema.

use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};

use crate::{
    config::{WorkspaceConfig, resolve_config, viewports::ViewportDef},
    types::{TestAction, TestScenario, TestSuite},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Workspace,
    Suite,
    Scenario,
    Action(String),
    Viewport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Boolean,
    Number { integer: bool },
    List { single: bool },
    Choice(Vec<String>),
    Wait,
    Structured,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub description: String,
    pub kind: FieldKind,
    pub required: bool,
}

impl Field {
    pub fn parse(&self, text: &str) -> Result<Value> {
        Ok(match &self.kind {
            FieldKind::Text => json!(text),
            FieldKind::Boolean => json!(
                text.parse::<bool>()
                    .map_err(|_| anyhow!("{} must be true or false", self.label))?
            ),
            FieldKind::Number { integer } => {
                let number: Value = serde_json::from_str(text)
                    .map_err(|_| anyhow!("{} needs a number", self.label))?;
                ensure!(
                    number.is_number() && (!integer || number.as_u64().is_some()),
                    "{} needs a non-negative {}",
                    self.label,
                    if *integer { "integer" } else { "number" }
                );
                number
            }
            FieldKind::List { single } => {
                if text.trim_start().starts_with('[') {
                    let values: Vec<String> = serde_yaml::from_str(text)?;
                    json!(values)
                } else {
                    let lines = text
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .collect::<Vec<_>>();
                    if *single && lines.len() == 1 {
                        json!(lines[0])
                    } else {
                        json!(lines)
                    }
                }
            }
            FieldKind::Choice(choices) => {
                ensure!(
                    choices.iter().any(|choice| choice == text),
                    "Unsupported {}",
                    self.label
                );
                json!(text)
            }
            FieldKind::Wait => {
                if let Ok(milliseconds) = text.parse::<u64>() {
                    json!(milliseconds)
                } else {
                    ensure!(
                        !text.is_empty() && text.parse::<f64>().is_err(),
                        "Wait needs non-negative integer milliseconds or a selector"
                    );
                    json!(text)
                }
            }
            FieldKind::Structured => serde_yaml::from_str(text)?,
        })
    }

    pub fn display(&self, value: &Value) -> String {
        match (&self.kind, value) {
            (_, Value::Null) => String::new(),
            (_, Value::String(text)) => text.clone(),
            (FieldKind::List { .. }, Value::Array(items)) => items
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n"),
            (FieldKind::Structured, _) => serde_json::to_string_pretty(value).unwrap_or_default(),
            _ => value.to_string(),
        }
    }
}

pub fn fields(scope: &Scope) -> Result<Vec<Field>> {
    let root = match scope {
        Scope::Workspace => serde_json::to_value(schemars::schema_for!(WorkspaceConfig))?,
        Scope::Suite => serde_json::to_value(schemars::schema_for!(TestSuite))?,
        Scope::Scenario => serde_json::to_value(schemars::schema_for!(TestScenario))?,
        Scope::Action(_) => serde_json::to_value(schemars::schema_for!(TestAction))?,
        Scope::Viewport => serde_json::to_value(schemars::schema_for!(ViewportDef))?,
    };
    let schema = if let Scope::Action(name) = scope {
        root["anyOf"]
            .as_array()
            .ok_or_else(|| anyhow!("Missing action schema"))?
            .iter()
            .map(|schema| resolve(schema, &root))
            .find(|schema| schema["properties"].get(name).is_some())
            .ok_or_else(|| anyhow!("Unknown action '{name}'"))?
    } else {
        &root
    };
    let properties = schema["properties"]
        .as_object()
        .ok_or_else(|| anyhow!("Missing editor schema"))?;
    let mut fields = Vec::new();
    for (key, definition) in properties {
        if matches!(key.as_str(), "scenarios" | "actions" | "testDate") {
            continue;
        }
        let required = schema["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|value| value == key));
        fields.push(Field {
            key: key.clone(),
            label: label(key),
            description: definition["description"]
                .as_str()
                .unwrap_or_default()
                .into(),
            kind: kind(key, definition, &root),
            required,
        });
    }
    fields.sort_by_key(|field| (!field.required, priority(&field.key), field.key.clone()));
    Ok(fields)
}

fn resolve<'schema>(schema: &'schema Value, root: &'schema Value) -> &'schema Value {
    if let Some(reference) = schema["$ref"]
        .as_str()
        .and_then(|reference| reference.strip_prefix('#'))
    {
        root.pointer(reference).unwrap_or(schema)
    } else {
        schema
    }
}

fn types(schema: &Value, root: &Value) -> Vec<String> {
    let schema = resolve(schema, root);
    if let Some(variants) = schema["anyOf"].as_array() {
        return variants
            .iter()
            .flat_map(|variant| types(variant, root))
            .collect();
    }
    if let Some(variants) = schema["type"].as_array() {
        return variants
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| *kind != "null")
            .map(str::to_string)
            .collect();
    }
    schema["type"]
        .as_str()
        .filter(|kind| *kind != "null")
        .map(|kind| vec![kind.to_string()])
        .unwrap_or_default()
}

fn kind(key: &str, schema: &Value, root: &Value) -> FieldKind {
    if key == "wait" {
        return FieldKind::Wait;
    }
    if key == "browser" {
        return FieldKind::Choice(vec!["chromium".into()]);
    }
    let types = types(schema, root);
    if types.iter().any(|kind| kind == "array") {
        if key == "urlReplacements" {
            return FieldKind::Structured;
        }
        return FieldKind::List {
            single: types.iter().any(|kind| kind == "string"),
        };
    }
    if types.iter().any(|kind| kind == "boolean") {
        return FieldKind::Boolean;
    }
    if types
        .iter()
        .any(|kind| kind == "integer" || kind == "number")
    {
        return FieldKind::Number {
            integer: types.iter().any(|kind| kind == "integer"),
        };
    }
    if types.iter().any(|kind| kind == "string") {
        return FieldKind::Text;
    }
    FieldKind::Structured
}

fn priority(key: &str) -> usize {
    match key {
        "label" => 0,
        "url" => 1,
        "id" => 2,
        "description" => 3,
        "viewportNames" => 4,
        "misMatchThreshold" => 5,
        _ => 10,
    }
}

fn label(key: &str) -> String {
    match key {
        "misMatchThreshold" => return "Mismatch threshold (%)".into(),
        "url" => return "URL".into(),
        "jsOnReadyPath" => return "Ready script".into(),
        "cssOverridePath" => return "CSS override file".into(),
        _ => {}
    }
    let mut label = String::new();
    for (index, character) in key.chars().enumerate() {
        if index == 0 {
            label.extend(character.to_uppercase());
        } else if character.is_uppercase() {
            label.push(' ');
            label.extend(character.to_lowercase());
        } else {
            label.push(character);
        }
    }
    if key.ends_with("Timeout") || matches!(key, "delay" | "postInteractionWait") {
        label.push_str(" (ms)");
    }
    label
}

/// Use the production resolver, including restore-state merging and timeout defaults.
pub fn effective(
    workspace: Option<&Value>,
    suite: Option<&Value>,
    scenario: Option<usize>,
) -> Result<Value> {
    let workspace: WorkspaceConfig =
        serde_json::from_value(workspace.cloned().unwrap_or_else(|| json!({})))?;
    let suite: TestSuite =
        serde_json::from_value(suite.cloned().unwrap_or_else(|| json!({"scenarios": []})))?;
    let empty: TestScenario = serde_json::from_value(json!({"url": ""}))?;
    let scenario = scenario
        .and_then(|index| suite.scenarios.get(index))
        .unwrap_or(&empty);
    Ok(serde_json::to_value(resolve_config(
        &workspace, &suite, scenario,
    ))?)
}

pub fn effective_key(key: &str) -> String {
    if matches!(
        key,
        "elementTimeout" | "actionabilityTimeout" | "settleTimeout" | "scenarioTimeout"
    ) {
        format!("{key}Ms")
    } else {
        key.to_string()
    }
}

pub fn origin(
    key: &str,
    workspace: Option<&Value>,
    suite: Option<&Value>,
    scenario: Option<&Value>,
) -> &'static str {
    if key == "state"
        && scenario
            .and_then(|scenario| scenario.get("restore"))
            .is_some_and(|value| !value.is_null())
    {
        return "State + restore";
    }
    for (source, name) in [
        (scenario, "Scenario"),
        (suite, "Suite"),
        (workspace, "Workspace"),
    ] {
        if source
            .and_then(|source| source.get(key))
            .is_some_and(|value| !value.is_null())
        {
            return name;
        }
    }
    "Default"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::document::{ACTIONS, action_template};

    #[test]
    fn schema_covers_workspace_suite_scenario_and_viewports() {
        let suite: TestSuite = serde_json::from_value(json!({"scenarios": []})).unwrap();
        let scenario: TestScenario =
            serde_json::from_value(json!({"url": "https://example.com"})).unwrap();
        for (scope, serialized) in [
            (
                Scope::Workspace,
                serde_json::to_value(WorkspaceConfig::default()).unwrap(),
            ),
            (Scope::Suite, serde_json::to_value(suite).unwrap()),
            (Scope::Scenario, serde_json::to_value(scenario).unwrap()),
        ] {
            let expected = serialized
                .as_object()
                .unwrap()
                .keys()
                .filter(|key| !matches!(key.as_str(), "scenarios" | "actions" | "testDate"))
                .cloned()
                .collect::<std::collections::BTreeSet<_>>();
            let actual = fields(&scope)
                .unwrap()
                .into_iter()
                .map(|field| field.key)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(actual, expected, "{scope:?}");
        }
        let scenarios = fields(&Scope::Scenario).unwrap();
        assert!(
            scenarios
                .iter()
                .any(|field| field.key == "url" && field.required)
        );
        assert!(scenarios.iter().any(|field| field.key == "restore"
            && matches!(field.kind, FieldKind::List { single: true })));
        assert_eq!(fields(&Scope::Viewport).unwrap().len(), 3);
    }

    #[test]
    fn all_action_properties_have_guided_fields() {
        for action in ACTIONS {
            let fields = fields(&Scope::Action(action.to_string())).unwrap();
            assert!(
                fields
                    .iter()
                    .any(|field| field.key == *action && field.required),
                "{action}"
            );
            assert!(fields.iter().any(|field| field.key == "frame"), "{action}");
            let value = action_template(action).unwrap();
            for (key, value) in value.as_object().unwrap() {
                let field = fields.iter().find(|field| field.key == *key).unwrap();
                assert_eq!(
                    field.parse(&field.display(value)).unwrap(),
                    *value,
                    "{action}.{key}"
                );
            }
        }
        assert!(fields(&Scope::Action("unknown".into())).is_err());
    }

    #[test]
    fn explicit_empty_text_false_and_arrays_round_trip() {
        for (key, value) in [
            ("cookiePath", json!("")),
            ("debug", json!(false)),
            ("hideSelectors", json!([])),
            ("state", json!(["one", "two"])),
        ] {
            let field = fields(&Scope::Workspace)
                .unwrap()
                .into_iter()
                .find(|field| field.key == key)
                .unwrap();
            assert_eq!(field.parse(&field.display(&value)).unwrap(), value);
        }
        let wait = fields(&Scope::Action("wait".into()))
            .unwrap()
            .into_iter()
            .find(|field| field.key == "wait")
            .unwrap();
        assert!(wait.parse("-1").is_err());
        assert_eq!(wait.parse("#ready").unwrap(), "#ready");
    }

    #[test]
    fn effective_settings_use_the_real_cascade_and_restore_merge() {
        let workspace = json!({"bypassCsp": true, "state": "signed-in", "elementTimeout": 400});
        let suite = json!({"bypassCsp": false, "scenarios": [{"url": "https://example.com", "restore": "checkout"}]});
        let resolved = effective(Some(&workspace), Some(&suite), Some(0)).unwrap();
        assert_eq!(resolved["bypassCsp"], false);
        assert_eq!(resolved["elementTimeoutMs"], 400);
        assert_eq!(resolved["state"], json!(["signed-in", "checkout"]));
        assert_eq!(
            origin(
                "bypassCsp",
                Some(&workspace),
                Some(&suite),
                Some(&suite["scenarios"][0])
            ),
            "Suite"
        );
        assert_eq!(
            origin(
                "state",
                Some(&workspace),
                Some(&suite),
                Some(&suite["scenarios"][0])
            ),
            "State + restore"
        );
    }
}
