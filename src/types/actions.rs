#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/#spoiler--check
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
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
            Self::Uncheck(action) => &action.frame,
            Self::Wait(action) => &action.frame,
        }
    }
}

/// Shared property: frame selector for targeting iframes.
/// Can be a single selector string or an array of selectors for nested iframes.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub enum Frame {
    Single(String),
    Multiple(Vec<String>),
}

/// File input: a single file path or an array of file paths (relative to visual_tests).
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub enum FileInput {
    Single(String),
    Multiple(Vec<String>),
}

/// Select value: a single value or an array of values.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub enum SelectValue {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct CheckAction {
    pub check: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct ClickAction {
    pub click: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct FocusAction {
    pub focus: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct GotoAction {
    pub goto: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct HideAction {
    pub hide: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct HoverAction {
    pub hover: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct RemoveAction {
    pub remove: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct InputAction {
    pub input: String,
    pub value: Option<String>,
    pub append: Option<bool>,
    pub file: Option<FileInput>,
    pub use_file_chooser: Option<bool>,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct PersistAction {
    pub persist: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct PressAction {
    pub press: String,
    pub key: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct SaveAction {
    pub save: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct ScrollAction {
    pub scroll: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct SelectAction {
    pub select: String,
    pub value: Option<SelectValue>,
    pub label: Option<String>,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct UncheckAction {
    pub uncheck: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub struct WaitAction {
    pub wait: Wait,
    pub url: Option<String>,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
#[cfg_attr(feature = "desktop", derive(schemars::JsonSchema))]
pub enum Wait {
    Number(u64),

    String(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_structs_reject_unknown_properties() {
        let result = serde_yaml::from_str::<ClickAction>("click: '#submit'\nfram: '#app'\n");

        assert!(matches!(result, Err(error) if error.to_string().contains("unknown field")));
    }
}
