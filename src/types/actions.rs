#![allow(dead_code)]
use serde::{Deserialize, Serialize};

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
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct ClickAction {
    /// CSS selector to click. Scrolls into view and waits until visible, stable, enabled, and not covered by another element.
    #[schemars(example = "#submit")]
    pub click: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct FocusAction {
    /// CSS selector of the element to focus. Scrolls into view and checks that it is interactable.
    #[schemars(example = "#email")]
    pub focus: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct GotoAction {
    /// Navigate to this URL before continuing. No element selector is needed.
    #[schemars(example = "https://example.com")]
    pub goto: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct HideAction {
    /// CSS selector of elements to hide while retaining their layout space. Useful for changing content such as timestamps.
    #[schemars(example = ".timestamp")]
    pub hide: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct HoverAction {
    /// CSS selector to hover using the real mouse pointer. Scrolls into view and waits for an interactable target.
    #[schemars(example = "#navigation-menu")]
    pub hover: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct RemoveAction {
    /// CSS selector of elements to remove from the DOM, including their layout space.
    #[schemars(example = ".advertisement")]
    pub remove: String,
    pub frame: Option<Frame>,
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
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct PersistAction {
    /// Save cookies and localStorage under this name, without .json. Restore with state or restore in later captures.
    #[schemars(example = "signed-in")]
    pub persist: String,
    pub frame: Option<Frame>,
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
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct SaveAction {
    /// Name of an intermediate screenshot checkpoint. The scenario continues after capture.
    #[schemars(example = "after-login")]
    pub save: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct ScrollAction {
    /// CSS selector to scroll into view. Does not click or focus the element.
    #[schemars(example = "#footer")]
    pub scroll: String,
    pub frame: Option<Frame>,
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
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(schemars::JsonSchema)]
pub struct UncheckAction {
    /// CSS selector of a checked checkbox to clear. Does nothing when already unchecked; cannot clear a selected radio button.
    #[schemars(example = "#newsletter")]
    pub uncheck: String,
    pub frame: Option<Frame>,
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
