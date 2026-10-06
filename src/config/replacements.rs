#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Result, anyhow};
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
#[derive(Deserialize, Clone, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplacementProfilesFile {
    /// Named ordered URL replacement lists. The default profile is used when no profile is selected.
    pub profiles: HashMap<String, Vec<UrlReplacement>>,
}

/// Load replacement profiles from a YAML file.
/// Looks for `common/_replacement-profiles.yaml` or `common/_replacement-profiles.yml`
/// relative to the workspace root.
pub fn load_replacement_profiles(root_dir: &Path) -> Result<Option<ReplacementProfilesFile>> {
    let config_path = super::resolve_optional_workspace_file(
        root_dir,
        "common/_replacement-profiles.yaml",
        "replacement profile file",
    )?
    .or(super::resolve_optional_workspace_file(
        root_dir,
        "common/_replacement-profiles.yml",
        "replacement profile file",
    )?);

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
    if let Ok(env_val) = std::env::var("REPLACEMENT_PROFILE")
        && !env_val.is_empty()
    {
        return env_val;
    }
    "default".to_string()
}

/// Apply a list of URL replacements to a URL string.
/// Supports both literal and regex replacements.
pub fn apply_replacements(url: &str, replacements: &[UrlReplacement]) -> Result<String> {
    let mut result = url.to_string();
    for replacement in replacements {
        if replacement.regex.unwrap_or(false) {
            let regex = compile_replacement_regex(replacement)?;
            result = regex
                .replace_all(&result, replacement.test.as_str())
                .to_string();
        } else {
            if replacement
                .flags
                .as_deref()
                .is_some_and(|flags| !flags.is_empty())
            {
                anyhow::bail!(
                    "URL replacement '{}' specifies flags but does not enable regex",
                    replacement.ref_pattern
                );
            }
            result = result.replace(&replacement.ref_pattern, &replacement.test);
        }
    }
    Ok(result)
}

fn compile_replacement_regex(replacement: &UrlReplacement) -> Result<regex::Regex> {
    let mut modifiers = String::new();
    let mut seen = HashSet::new();
    for flag in replacement.flags.as_deref().unwrap_or("").chars() {
        if !seen.insert(flag) {
            anyhow::bail!(
                "Regex URL replacement '{}' repeats flag '{}'",
                replacement.ref_pattern,
                flag
            );
        }
        match flag {
            // replace_all is global regardless of an explicit JS `g` flag;
            // Rust regex is Unicode-aware by default, matching JS `u` URLs.
            'g' | 'u' => {}
            'i' | 'm' | 's' => modifiers.push(flag),
            unsupported => anyhow::bail!(
                "Regex URL replacement '{}' has unsupported flag '{}' (supported: g, i, m, s, u)",
                replacement.ref_pattern,
                unsupported
            ),
        }
    }
    let pattern = if modifiers.is_empty() {
        replacement.ref_pattern.clone()
    } else {
        format!("(?{}){}", modifiers, replacement.ref_pattern)
    };
    regex::Regex::new(&pattern).map_err(|error| {
        anyhow!(
            "Invalid regex URL replacement '{}': {}",
            replacement.ref_pattern,
            error
        )
    })
}

/// Validate replacement rules as configuration before browser scenarios start.
pub fn validate_replacements(replacements: &[UrlReplacement], source: &str) -> Result<()> {
    for replacement in replacements {
        if replacement.ref_pattern.is_empty() {
            return Err(anyhow!(
                "URL replacement in {} has an empty `ref` pattern",
                source
            ));
        }
        if replacement.regex.unwrap_or(false) {
            compile_replacement_regex(replacement).map_err(|error| {
                anyhow!(
                    "Invalid regex URL replacement '{}' in {}: {}",
                    replacement.ref_pattern,
                    source,
                    error
                )
            })?;
        } else if replacement
            .flags
            .as_deref()
            .is_some_and(|flags| !flags.is_empty())
        {
            anyhow::bail!(
                "URL replacement '{}' in {} specifies flags but does not enable regex",
                replacement.ref_pattern,
                source
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regex_replacement(pattern: &str, test: &str, flags: &str) -> UrlReplacement {
        UrlReplacement {
            ref_pattern: pattern.to_string(),
            test: test.to_string(),
            regex: Some(true),
            flags: Some(flags.to_string()),
        }
    }

    #[test]
    fn regex_replacements_honor_supported_flags() {
        let insensitive = regex_replacement("^https://example\\.com$", "ok", "i");
        assert_eq!(
            apply_replacements("HTTPS://EXAMPLE.COM", &[insensitive]).unwrap(),
            "ok"
        );

        let dot_all = regex_replacement("a.b", "ok", "s");
        assert_eq!(apply_replacements("a\nb", &[dot_all]).unwrap(), "ok");

        let multiline = regex_replacement("^b$", "ok", "m");
        assert_eq!(
            apply_replacements("a\nb\nc", &[multiline]).unwrap(),
            "a\nok\nc"
        );

        let global = regex_replacement("a", "b", "g");
        assert_eq!(apply_replacements("aaa", &[global]).unwrap(), "bbb");
    }

    #[test]
    fn replacement_validation_rejects_invalid_flag_configurations() {
        for replacement in [
            regex_replacement("a", "b", "ii"),
            regex_replacement("a", "b", "y"),
            UrlReplacement {
                ref_pattern: "a".to_string(),
                test: "b".to_string(),
                regex: Some(false),
                flags: Some("i".to_string()),
            },
        ] {
            assert!(
                validate_replacements(&[replacement], "test")
                    .unwrap_err()
                    .to_string()
                    .contains("flag")
            );
        }
    }

    #[test]
    fn replacement_profile_file_rejects_unknown_properties() {
        let result =
            serde_yaml::from_str::<ReplacementProfilesFile>("profiles: {}\nprofilez: {}\n");

        assert!(matches!(result, Err(error) if error.to_string().contains("unknown field")));
    }

    #[cfg(unix)]
    #[test]
    fn replacement_profile_file_cannot_escape_workspace_through_symlink() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let common = workspace.path().join("common");
        let outside_profile = outside.path().join("profiles.yaml");
        std::fs::create_dir_all(&common).unwrap();
        std::fs::write(&outside_profile, "profiles: {}\n").unwrap();
        std::os::unix::fs::symlink(&outside_profile, common.join("_replacement-profiles.yaml"))
            .unwrap();

        let error = match load_replacement_profiles(workspace.path()) {
            Ok(_) => panic!("replacement profile symlink unexpectedly escaped the workspace"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("escapes workspace root"));
    }
}
