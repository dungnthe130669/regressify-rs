#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/#spoiler--check
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
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

/// Shared property: frame selector for targeting iframes.
/// Can be a single selector string or an array of selectors for nested iframes.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Frame {
    Single(String),
    Multiple(Vec<String>),
}

/// File input: a single file path or an array of file paths (relative to visual_tests).
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileInput {
    Single(String),
    Multiple(Vec<String>),
}

/// Select value: a single value or an array of values.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum SelectValue {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckAction {
    pub check: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickAction {
    pub click: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusAction {
    pub focus: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoAction {
    pub goto: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HideAction {
    pub hide: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverAction {
    pub hover: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveAction {
    pub remove: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputAction {
    pub input: String,
    pub value: Option<String>,
    pub append: Option<bool>,
    pub file: Option<FileInput>,
    pub use_file_chooser: Option<bool>,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistAction {
    pub persist: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PressAction {
    pub press: String,
    pub key: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAction {
    pub save: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollAction {
    pub scroll: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectAction {
    pub select: String,
    pub value: Option<SelectValue>,
    pub label: Option<String>,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UncheckAction {
    pub uncheck: String,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitAction {
    pub wait: Wait,
    pub url: Option<String>,
    pub frame: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Wait {
    Number(u64),

    String(String),
}
