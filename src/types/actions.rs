#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Documentation: https://tuyen.blog/optimizely-cms/testing/create-action/#spoiler--check
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum TestAction {
    Click(ClickAction),
    Save(SaveAction),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickAction {
    pub click: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAction {
    pub save: String,
}
