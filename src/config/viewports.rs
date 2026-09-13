#![allow(dead_code)]

use std::path::Path;

use std::collections::HashSet;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::helpers::slugify;

const MAX_CAPTURE_NAME_BYTES: usize = 48;

/// A viewport definition with label and dimensions.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
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
    let mut capture_names = HashSet::new();
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
        let capture_name = slugify(label);
        if capture_name.is_empty() {
            return Err(anyhow!(
                "Viewport label '{}' in '{}' cannot produce a screenshot filename",
                viewport.label,
                path.display()
            ));
        }
        if capture_name.len() > MAX_CAPTURE_NAME_BYTES {
            return Err(anyhow!(
                "Viewport label '{}' in '{}' produces a screenshot name longer than {} bytes",
                viewport.label,
                path.display(),
                MAX_CAPTURE_NAME_BYTES
            ));
        }
        if !capture_names.insert(capture_name) {
            return Err(anyhow!(
                "Viewport file '{}' contains labels that produce duplicate screenshot names",
                path.display()
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

    #[test]
    fn validate_viewports_rejects_colliding_or_empty_capture_names() {
        let colliding = vec![viewport("mobile!", 375, 667), viewport("mobile?", 390, 844)];
        let error =
            validate_viewports(&colliding, Path::new("common/_viewports.yaml")).unwrap_err();
        assert!(error.to_string().contains("duplicate screenshot names"));

        let empty = vec![viewport("!!!", 375, 667)];
        let error = validate_viewports(&empty, Path::new("common/_viewports.yaml")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot produce a screenshot filename")
        );

        let long = vec![viewport(&"a".repeat(MAX_CAPTURE_NAME_BYTES + 1), 375, 667)];
        let error = validate_viewports(&long, Path::new("common/_viewports.yaml")).unwrap_err();
        assert!(error.to_string().contains("longer than"));
    }

    #[test]
    fn load_viewports_rejects_unknown_properties() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("viewports.yaml");
        std::fs::write(
            &path,
            "- label: desktop\n  width: 1280\n  height: 720\n  deviceScaleFactor: 2\n",
        )
        .unwrap();

        let error = load_viewports(&path).unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }
}
