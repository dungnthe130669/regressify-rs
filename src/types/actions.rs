#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/#spoiler--check
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum TestAction {
    Click(ClickAction),
    Hide(HideAction),
    Input(InputAction),
    RemoveAction(RemoveAction),
    Save(SaveAction),
    Wait(WaitAction),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickAction {
    pub click: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HideAction {
    pub hide: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveAction {
    pub remove: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputAction {
    pub input: String,
    pub value: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAction {
    pub save: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitAction {
    pub wait: Wait,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Wait {
    Number(u64),

    String(String),
}
