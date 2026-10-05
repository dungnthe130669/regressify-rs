#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use crate::types::Selectors;

/// One browser action per list item. Actions run in order; a failure stops the current scenario.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
pub enum TestAction {
    Check(CheckAction),
    Click(ClickAction),
    Focus(FocusAction),
    Goto(GotoAction),
    Hide(HideAction),
    Hover(HoverAction),
    Input(InputAction),
    Persist(PersistAction),
    Press(PressAction),
    Remove(RemoveAction),
    Save(SaveAction),
    Select(SelectAction),
    Scroll(ScrollAction),
    SwitchTab(SwitchTabAction),
    Uncheck(UncheckAction),
    Wait(WaitAction),
}

impl TestAction {
    pub(crate) fn frame(&self) -> &Option<Frame> {
        match self {
            Self::Check(action) => &action.frame,
            Self::Click(action) => &action.frame,
            Self::Focus(action) => &action.frame,
            Self::Goto(action) => &action.frame,
            Self::Hide(action) => &action.frame,
            Self::Hover(action) => &action.frame,
            Self::Input(action) => &action.frame,
            Self::Persist(action) => &action.frame,
            Self::Press(action) => &action.frame,
            Self::Remove(action) => &action.frame,
            Self::Save(action) => &action.frame,
            Self::Select(action) => &action.frame,
            Self::Scroll(action) => &action.frame,
            Self::SwitchTab(_) => &None,
            Self::Uncheck(action) => &action.frame,
            Self::Wait(action) => &action.frame,
        }
    }

    pub(crate) fn viewport_names(&self) -> Option<&[String]> {
        let names = match self {
            Self::Check(action) => &action.viewport_names,
            Self::Click(action) => &action.viewport_names,
            Self::Focus(action) => &action.viewport_names,
            Self::Goto(action) => &action.viewport_names,
            Self::Hide(action) => &action.viewport_names,
            Self::Hover(action) => &action.viewport_names,
            Self::Input(action) => &action.viewport_names,
            Self::Persist(action) => &action.viewport_names,
            Self::Press(action) => &action.viewport_names,
            Self::Remove(action) => &action.viewport_names,
            Self::Save(action) => &action.viewport_names,
            Self::Select(action) => &action.viewport_names,
            Self::Scroll(action) => &action.viewport_names,
            Self::SwitchTab(action) => &action.viewport_names,
            Self::Uncheck(action) => &action.viewport_names,
            Self::Wait(action) => &action.viewport_names,
        };
        names.as_ref().map(Selectors::values)
    }
}

/// Optional iframe CSS selector, or a list of selectors from outermost to innermost frame.
/// Omit to target the main page. Quote selectors beginning with # in YAML.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
#[schemars(example = &"iframe#checkout", example = vec!["iframe.outer", "iframe.inner"])]
pub enum Frame {
    Single(String),
    Multiple(Vec<String>),
}

/// File input: a single file path or an array of file paths (relative to visual_tests).
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
#[schemars(example = "uploads/sample.pdf", example = vec!["uploads/first.pdf", "uploads/second.pdf"])]
pub enum FileInput {
    Single(String),
    Multiple(Vec<String>),
}

/// Option value, or a list of values for a select element with the multiple attribute.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
#[schemars(example = &"option", example = vec!["first", "second"])]
pub enum SelectValue {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct CheckAction {
    /// CSS selector of a checkbox or radio button. Clicks only when not already checked; waits for visibility and an unobscured, enabled target.
    #[schemars(example = "#accept-terms")]
    pub check: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct ClickAction {
    /// CSS selector to click. Scrolls into view and waits until visible, stable, enabled, and not covered by another element.
    #[schemars(example = "#submit")]
    pub click: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct FocusAction {
    /// CSS selector of the element to focus. Scrolls into view and checks that it is interactable.
    #[schemars(example = "#email")]
    pub focus: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct GotoAction {
    /// Navigate to this URL before continuing. No element selector is needed.
    #[schemars(example = "https://example.com")]
    pub goto: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct HideAction {
    /// CSS selector of elements to hide while retaining their layout space. Useful for changing content such as timestamps.
    #[schemars(example = ".timestamp")]
    pub hide: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct HoverAction {
    /// CSS selector to hover using the real mouse pointer. Scrolls into view and waits for an interactable target.
    #[schemars(example = "#navigation-menu")]
    pub hover: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct RemoveAction {
    /// CSS selector of elements to remove from the DOM, including their layout space.
    #[schemars(example = ".advertisement")]
    pub remove: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct InputAction {
    /// CSS selector of the input, textarea, or editable element. Supply value for text or file for an upload.
    #[schemars(example = "#email")]
    pub input: String,
    /// Text to enter. Quote numbers, true, and false when they are text. Use an empty quoted string to clear the field.
    #[schemars(example = "qa@example.com", example = &"")]
    pub value: Option<String>,
    /// Append to existing text instead of clearing it first (default: false).
    pub append: Option<bool>,
    /// File path or list of file paths, relative to visual_tests/. Use with a file input; do not put file contents here.
    pub file: Option<FileInput>,
    /// Open a file chooser by clicking the target, then supply file (default: false).
    pub use_file_chooser: Option<bool>,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct PersistAction {
    /// Save cookies and localStorage under this name, without .json. Restore with state or restore in later captures.
    #[schemars(example = "signed-in")]
    pub persist: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct PressAction {
    /// CSS selector of the element that receives the key. The key property is also required.
    #[schemars(example = "#search")]
    pub press: String,
    /// Keyboard key or combination: Enter, Tab, Escape, ArrowDown, Ctrl+A, or Shift+Tab, for example.
    #[schemars(example = &"Enter", example = &"Tab", example = "Ctrl+A", example = "Shift+Tab")]
    pub key: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct SaveAction {
    /// Name of an intermediate screenshot checkpoint. The scenario continues after capture.
    #[schemars(example = "after-login")]
    pub save: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct ScrollAction {
    /// CSS selector to scroll into view. Does not click or focus the element.
    #[schemars(example = "#footer")]
    pub scroll: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct SelectAction {
    /// CSS selector of a select element. Supply value to match option values, or label to match visible option text.
    #[schemars(example = "#country")]
    pub select: String,
    /// Option value, or list of values for a multiple select. Fails when no option matches.
    pub value: Option<SelectValue>,
    /// Visible option text to select instead of matching by value.
    #[schemars(example = &"Sweden")]
    pub label: Option<String>,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct UncheckAction {
    /// CSS selector of a checked checkbox to clear. Does nothing when already unchecked; cannot clear a selected radio button.
    #[schemars(example = "#newsletter")]
    pub uncheck: String,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct SwitchTabAction {
    /// Select a new tab opened by the current page, the previously selected tab, or the scenario's original tab. Subsequent actions and screenshots use the selected tab.
    pub switch_tab: TabTarget,
    /// Optional URL or glob for a new tab. Waits for its navigation to match; * stays within one path segment and ** matches across segments. Only valid with switchTab: new.
    #[schemars(example = "**/quote")]
    pub url: Option<String>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(schemars::JsonSchema)]
pub enum TabTarget {
    New,
    Previous,
    Original,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct WaitAction {
    /// Whole milliseconds to pause, or a CSS selector to wait for. When url is supplied, a numeric wait is the URL timeout in milliseconds.
    #[schemars(example = 1000, example = "#ready")]
    pub wait: Wait,
    /// URL or glob to wait for. * matches within one path segment; ** matches across path segments.
    #[schemars(example = "**/dashboard")]
    pub url: Option<String>,
    pub frame: Option<Frame>,
    /// Restrict this action to one viewport label or a list of labels (case-insensitive).
    /// Omitted, null, or an empty list means all scenario viewports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_names: Option<Selectors>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[derive(schemars::JsonSchema)]
pub enum Wait {
    Number(u64),

    String(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_shapes() -> [serde_json::Value; 16] {
        use serde_json::json;

        [
            json!({ "check": "#accept" }),
            json!({ "click": "#submit" }),
            json!({ "focus": "#email" }),
            json!({ "goto": "https://example.com" }),
            json!({ "hide": ".timestamp" }),
            json!({ "hover": "#menu" }),
            json!({ "input": "#email", "value": "test" }),
            json!({ "persist": "signed-in" }),
            json!({ "press": "#search", "key": "Enter" }),
            json!({ "remove": ".advertisement" }),
            json!({ "save": "after-login" }),
            json!({ "select": "#country", "value": "se" }),
            json!({ "scroll": "#footer" }),
            json!({ "switchTab": "new", "url": "**/quote" }),
            json!({ "uncheck": "#newsletter" }),
            json!({ "wait": 100 }),
        ]
    }

    #[test]
    fn all_action_viewport_names_round_trip_yaml_and_json() {
        use serde_json::{Value, json};

        for shape in action_shapes() {
            for filter in [
                None,
                Some(Value::Null),
                Some(json!([])),
                Some(json!("mobile")),
                Some(json!(["Desktop", "mobile", "mobile"])),
            ] {
                let mut source = shape.clone();
                let unfiltered: TestAction = serde_json::from_value(shape.clone()).unwrap();
                let mut expected = serde_json::to_value(unfiltered).unwrap();
                if let Some(filter) = &filter {
                    source["viewportNames"] = filter.clone();
                    if !filter.is_null() {
                        expected["viewportNames"] = filter.clone();
                    }
                }
                let expected_filter: Option<Vec<String>> = match filter.unwrap_or(Value::Null) {
                    Value::String(name) => Some(vec![name]),
                    filter => serde_json::from_value(filter).unwrap(),
                };
                let yaml = serde_yaml::to_string(&source).unwrap();
                for action in [
                    serde_json::from_value::<TestAction>(source.clone()).unwrap(),
                    serde_yaml::from_str::<TestAction>(&yaml).unwrap(),
                ] {
                    assert_eq!(
                        action.viewport_names(),
                        expected_filter.as_deref(),
                        "{source}"
                    );
                    assert_eq!(serde_json::to_value(&action).unwrap(), expected, "{source}");
                    let round_trip: TestAction =
                        serde_yaml::from_str(&serde_yaml::to_string(&action).unwrap()).unwrap();
                    assert_eq!(serde_json::to_value(round_trip).unwrap(), expected);
                }
            }
        }
    }

    #[test]
    fn all_action_viewport_names_reject_malformed_and_mixed_shapes() {
        use serde_json::json;

        for shape in action_shapes() {
            for filter in [
                json!(12),
                json!(false),
                json!([12]),
                json!([null]),
                json!([false]),
                json!({ "mobile": true }),
            ] {
                let mut source = shape.clone();
                source["viewportNames"] = filter;
                assert!(serde_json::from_value::<TestAction>(source.clone()).is_err());
                let yaml = serde_yaml::to_string(&source).unwrap();
                assert!(serde_yaml::from_str::<TestAction>(&yaml).is_err(), "{yaml}");
            }
            for property in [
                "unknownProperty",
                if shape.get("click").is_some() {
                    "hide"
                } else {
                    "click"
                },
            ] {
                let mut source = shape.clone();
                source["viewportNames"] = json!(["mobile"]);
                source[property] = json!("#target");
                assert!(serde_json::from_value::<TestAction>(source.clone()).is_err());
                let yaml = serde_yaml::to_string(&source).unwrap();
                assert!(serde_yaml::from_str::<TestAction>(&yaml).is_err(), "{yaml}");
            }
        }
    }

    #[test]
    fn switch_tab_actions_use_explicit_camel_case_targets() {
        for yaml in [
            "switchTab: new\nurl: '**/quote'\n",
            "switchTab: original\n",
            "switchTab: previous\n",
        ] {
            let expected: serde_json::Value = serde_yaml::from_str(yaml).unwrap();
            let action: TestAction =
                serde_yaml::from_str(yaml).expect("valid tab selection action");
            let actual = serde_json::to_value(action).unwrap();
            assert_eq!(actual["switchTab"], expected["switchTab"]);
            assert_eq!(actual["url"], expected["url"]);
        }
    }

    #[test]
    fn switch_tab_actions_reject_unknown_targets_and_properties() {
        for yaml in [
            "switchTab: missing\n",
            "switch_tab: new\n",
            "switchTab: new\nframe: '#frame'\n",
            "switchTab: new\nclick: '#link'\n",
        ] {
            assert!(serde_yaml::from_str::<TestAction>(yaml).is_err(), "{yaml}");
        }
    }

    #[test]
    fn action_structs_reject_unknown_properties() {
        let result = serde_yaml::from_str::<ClickAction>("click: '#submit'\nfram: '#app'\n");

        assert!(matches!(result, Err(error) if error.to_string().contains("unknown field")));
    }
}
