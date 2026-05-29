#![allow(dead_code)]

use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

/// A viewport definition with label and dimensions.
#[derive(Deserialize, Clone, Debug)]
pub struct ViewportDef {
    pub label: String,
    pub width: u32,
    pub height: u32,
}

/// Load viewport definitions from a YAML file.
/// The file should contain a list of viewport objects.
/// ```yaml
/// - label: phone
///   width: 320
///   height: 480
/// - label: tablet
///   width: 1024
///   height: 768
/// - label: desktop
///   width: 1920
///   height: 1080
/// ```
pub fn load_viewports(path: &Path) -> Result<Vec<ViewportDef>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let viewports: Vec<ViewportDef> = serde_yaml::from_str(&content)?;
    Ok(viewports)
}

/// Filter viewports by name (case-insensitive match on label).
/// If `viewport_names` is None or empty, returns all viewports.
pub fn resolve_viewports(
    viewport_names: Option<&[String]>,
    all_viewports: &[ViewportDef],
) -> Vec<ViewportDef> {
    match viewport_names {
        Some(names) if !names.is_empty() => all_viewports
            .iter()
            .filter(|v| names.iter().any(|n| n.eq_ignore_ascii_case(&v.label)))
            .cloned()
            .collect(),
        _ => all_viewports.to_vec(),
    }
}
