#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use crate::types::Selectors;

/// One browser action per list item. Actions run in order; a failure stops the current scenario.
/// Element selectors accept CSS plus text=, xpath=, :has-text(), and >> chains; hide, remove,
/// and screenshot selectors are standard CSS only.
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

/// Optional iframe selector, or a list of selectors from outermost to innermost frame.
/// Same-origin and cross-origin frames work; each selector must match an iframe or frame
/// element. Omit to target the main page. Quote selectors beginning with # in YAML.
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
    /// Selector of a native checkbox or radio button. Clicks only when not already checked; waits for visibility and an unobscured, enabled target. Use click instead for custom role="checkbox" or role="switch" controls or when the native input is visually hidden.
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
    /// Element selector to click. Scrolls into view and waits until visible, stable, enabled, and not covered by another element. Navigation that starts immediately is awaited; add a wait for later or client-side changes.
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
    /// Selector of the element to focus. Scrolls into view, checks that it is interactable, and fails unless it becomes the active element.
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
    /// Navigate the page (or frame) to this URL and wait until the document is interactive. Test captures apply URL replacements; reference captures use it as written.
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
    /// Standard CSS selector of elements to hide with visibility: hidden in the current document, keeping their layout space. Waits up to elementTimeout for a match and fails if none appears; use hideSelectors for optional elements.
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
    /// Element selector to hover using the real mouse pointer. Scrolls into view and waits for an interactable target.
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
    /// Standard CSS selector of elements to hide with display: none in the current document; they stay in the DOM but take no layout space. Waits up to elementTimeout for a match and fails if none appears; use removeSelectors for optional elements.
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
    /// Selector of the input, textarea, or contenteditable element. Supply exactly one of value (text) or file (upload); neither or both fails during preflight before any browser starts.
    #[schemars(example = "#email")]
    pub input: String,
    /// Literal text typed with keyboard events after the existing text is selected and deleted (unless append is true). No environment variables are substituted. Quote numbers, true, and false when they are text. Use an empty quoted string to clear the field.
    #[schemars(example = "qa@example.com", example = &"")]
    pub value: Option<String>,
    /// Append to existing text instead of clearing it first (default: false).
    pub append: Option<bool>,
    /// File path or list of file paths, relative to visual_tests/. Without useFileChooser the target must be a visible input type=file; do not put file contents here.
    pub file: Option<FileInput>,
    /// Click the target (for example a visible upload button) to open a file chooser, then supply file (default: false).
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
    /// State name without .json. Writes cookies and origin-scoped localStorage to the output directory's states/<name>.json; load it with state or restore in a later run. The file holds session secrets.
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
    /// Selector of the element that receives the key. The key property is also required.
    #[schemars(example = "#search")]
    pub press: String,
    /// Case-sensitive key name or combination with one non-modifier key: Enter, Tab, Escape, ArrowDown, Ctrl+A, Shift+Tab, or ControlOrMeta+A, for example. Unknown or wrongly cased keys fail during preflight before any browser starts.
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
    /// Checkpoint name saved as <scenario-id>[_<viewport>]__$<name>.png: at most 100 bytes, no path separators or __, and not ending in _diff. A last applicable save among the scenario's own actions replaces the automatic final screenshot, but explicit scenario selectors are still captured.
    #[schemars(example = "after-login")]
    pub save: String,
    /// Standard CSS selector to capture instead of the whole page, or "viewport" for the visible viewport. An element capture uses the first match in the selected tab's main document (or the target frame when frame is set on save), which must be visible, clipped to its box, so open menus or popups extending outside it are cut off. Independent of the scenario's final selector.
    #[schemars(
        length(min = 1),
        regex(pattern = r"\S"),
        example = "#confirmation",
        example = &"viewport"
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
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
    /// Element selector to scroll into view. Does not click or focus the element.
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
    /// Selector of a native select element. Supply exactly one of value (option values) or label (visible option text); neither or both fails during preflight before any browser starts. For custom dropdowns or comboboxes that are not native <select> elements, use click actions to open the menu and pick the option.
    #[schemars(example = "#country")]
    pub select: String,
    /// Option value, or list of values for a multiple select. Retries while options are missing or disabled, then fails after elementTimeout.
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
    /// Selector of a checked native checkbox to clear. Does nothing when already unchecked; cannot clear a selected radio button. Use click instead for custom role="checkbox" or role="switch" controls or when the native input is visually hidden.
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
    /// Optional URL or glob for a new tab. Waits for its navigation to match; * stays within one path segment and ** matches across segments. Only valid with switchTab: new; test captures apply URL replacements.
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
    /// Whole milliseconds to pause, or an element selector that must exist and become visible within elementTimeout. With url, the URL is matched first; this pause or selector wait runs afterwards.
    #[schemars(example = 1000, example = "#ready")]
    pub wait: Wait,
    /// URL or glob the page or frame URL must match within elementTimeout before the wait continues. * matches within one path segment, ** across segments, {a,b} lists alternatives, and ? is literal. Test captures apply URL replacements.
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
            json!({ "save": "after-login", "selector": "#confirmation" }),
            json!({ "select": "#country", "value": "se" }),
            json!({ "scroll": "#footer" }),
            json!({ "switchTab": "new", "url": "**/quote" }),
            json!({ "uncheck": "#newsletter" }),
            json!({ "wait": 100 }),
        ]
    }

    #[test]
    fn save_screenshot_selectors_round_trip_yaml_and_json() {
        use serde_json::{Value, json};

        for selector in [
            None,
            Some(Value::Null),
            Some(json!("[data-name=\"it's a card\"]")),
        ] {
            let mut source = json!({ "save": "detail" });
            if let Some(selector) = &selector {
                source["selector"] = selector.clone();
            }
            let yaml = serde_yaml::to_string(&source).unwrap();
            for action in [
                serde_json::from_value::<TestAction>(source.clone()).unwrap(),
                serde_yaml::from_str::<TestAction>(&yaml).unwrap(),
            ] {
                let TestAction::Save(save) = &action else {
                    panic!("Expected a save action");
                };
                assert_eq!(save.selector.as_deref(), source["selector"].as_str());
                let serialized = serde_json::to_value(&action).unwrap();
                assert_eq!(serialized["save"], "detail");
                assert_eq!(serialized["selector"], source["selector"]);
                assert_eq!(
                    serialized.get("selector").is_some(),
                    save.selector.is_some()
                );
            }
        }
        for selector in [json!(12), json!(false), json!(["#content"]), json!({})] {
            let source = json!({ "save": "detail", "selector": selector });
            assert!(serde_json::from_value::<TestAction>(source.clone()).is_err());
            assert!(
                serde_yaml::from_str::<TestAction>(&serde_yaml::to_string(&source).unwrap())
                    .is_err()
            );
        }
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
