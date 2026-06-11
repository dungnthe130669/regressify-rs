#![allow(dead_code)]

use std::path::Path;

use std::collections::HashSet;

use anyhow::{Result, anyhow};
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
    validate_viewports(&viewports, path)?;
    Ok(viewports)
}

fn validate_viewports(viewports: &[ViewportDef], path: &Path) -> Result<()> {
    let mut labels = HashSet::new();
    for viewport in viewports {
        let label = viewport.label.trim();
        if label.is_empty() {
            return Err(anyhow!(
                "Viewport file '{}' contains a viewport with an empty label",
                path.display()
            ));
        }
        if viewport.width == 0 || viewport.height == 0 {
            return Err(anyhow!(
                "Viewport '{}' in '{}' must have non-zero width and height",
                viewport.label,
                path.display()
            ));
        }
        let key = label.to_ascii_lowercase();
        if !labels.insert(key) {
            return Err(anyhow!(
                "Viewport file '{}' contains duplicate label '{}' (case-insensitive)",
                path.display(),
                viewport.label
            ));
        }
    }
    Ok(())
}

/// Filter viewports by name (case-insensitive match on label).
/// If `viewport_names` is None or empty, returns all viewports.
pub fn resolve_viewports(
    viewport_names: Option<&[String]>,
    all_viewports: &[ViewportDef],
) -> Result<Vec<ViewportDef>> {
    match viewport_names {
        Some(names) if !names.is_empty() => {
            let mut resolved = Vec::new();
            let mut missing = Vec::new();
            for name in names {
                match all_viewports
                    .iter()
                    .find(|v| name.eq_ignore_ascii_case(&v.label))
                {
                    Some(viewport) => resolved.push(viewport.clone()),
                    None => missing.push(name.clone()),
                }
            }
            if !missing.is_empty() {
                let available = all_viewports
                    .iter()
                    .map(|v| v.label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(anyhow!(
                    "Unknown viewport name(s): {}. Available viewport(s): {}",
                    missing.join(", "),
                    available
                ));
            }
            Ok(resolved)
        }
        _ => Ok(all_viewports.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport(label: &str, width: u32, height: u32) -> ViewportDef {
        ViewportDef {
            label: label.to_string(),
            width,
            height,
        }
    }

    #[test]
    fn resolve_viewports_errors_for_missing_names() {
        let all = vec![viewport("desktop", 1280, 720)];
        let names = vec!["mobile".to_string()];
        let err = resolve_viewports(Some(&names), &all).unwrap_err();
        assert!(err.to_string().contains("Unknown viewport name"));
    }

    #[test]
    fn validate_viewports_rejects_duplicate_labels() {
        let all = vec![
            viewport("desktop", 1280, 720),
            viewport("Desktop", 1440, 900),
        ];
        let err = validate_viewports(&all, Path::new("common/_viewports.yaml")).unwrap_err();
        assert!(err.to_string().contains("duplicate label"));
    }
}
