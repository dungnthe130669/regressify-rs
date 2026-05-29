#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use crate::types::UrlReplacement;

/// A file containing named replacement profiles.
/// YAML structure:
/// ```yaml
/// profiles:
///   default:
///     - ref: "https://prod.example.com"
///       test: "https://staging.example.com"
///   staging:
///     - ref: "https://prod.example.com"
///       test: "https://staging.example.com"
///       regex: true
///       flags: "i"
/// ```
#[derive(Deserialize, Clone)]
pub struct ReplacementProfilesFile {
    pub profiles: HashMap<String, Vec<UrlReplacement>>,
}

/// Load replacement profiles from a YAML file.
/// Looks for `common/_replacements.yaml` or `common/_replacements.yml` in the visual_tests dir.
pub fn load_replacement_profiles(
    visual_tests_dir: &Path,
) -> Result<Option<ReplacementProfilesFile>> {
    let yaml_path = visual_tests_dir.join("common/_replacements.yaml");
    let yml_path = visual_tests_dir.join("common/_replacements.yml");

    let config_path = if yaml_path.exists() {
        Some(yaml_path)
    } else if yml_path.exists() {
        Some(yml_path)
    } else {
        None
    };

    match config_path {
        Some(path) => {
            let content = std::fs::read_to_string(&path)?;
            let profiles: ReplacementProfilesFile = serde_yaml::from_str(&content)?;
            Ok(Some(profiles))
        }
        None => Ok(None),
    }
}

/// Determine the active replacement profile name.
/// Priority: CLI arg -> REPLACEMENT_PROFILE env var -> "default"
pub fn get_profile_name(cli_arg: Option<&str>) -> String {
    if let Some(name) = cli_arg {
        return name.to_string();
    }
    if let Ok(env_val) = std::env::var("REPLACEMENT_PROFILE") && !env_val.is_empty() {
        return env_val;
    }
    "default".to_string()
}

/// Apply a list of URL replacements to a URL string.
/// Supports both literal and regex replacements.
pub fn apply_replacements(url: &str, replacements: &[UrlReplacement]) -> String {
    let mut result = url.to_string();
    for r in replacements {
        if r.regex.unwrap_or(false) {
            let flags = r.flags.as_deref().unwrap_or("");
            let case_insensitive = flags.contains('i');
            let pattern_str = if case_insensitive {
                format!("(?i){}", r.ref_pattern)
            } else {
                r.ref_pattern.clone()
            };
            if let Ok(re) = regex::Regex::new(&pattern_str) {
                result = re.replace_all(&result, r.test.as_str()).to_string();
            } else {
                eprintln!(
                    "    WARNING: Invalid regex pattern '{}' in replacements",
                    r.ref_pattern
                );
            }
        } else {
            result = result.replace(&r.ref_pattern, &r.test);
        }
    }
    result
}
