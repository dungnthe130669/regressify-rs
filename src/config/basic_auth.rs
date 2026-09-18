use anyhow::{Result, bail};
use regex::Regex;

use crate::types::BasicAuth;

pub fn resolve(entries: &[BasicAuth]) -> Result<Vec<BasicAuth>> {
    resolve_with(entries, |name| std::env::var(name).ok())
}

fn resolve_with(
    entries: &[BasicAuth],
    environment: impl Fn(&str) -> Option<String>,
) -> Result<Vec<BasicAuth>> {
    let pattern = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)")?;
    let expand = |value: &str| -> Result<String> {
        let mut missing = false;
        let expanded = pattern.replace_all(value, |captures: &regex::Captures<'_>| {
            let matched = captures.get(0).expect("regex always has a full match");
            let name = captures
                .get(1)
                .or_else(|| captures.get(2))
                .expect("environment reference capture");
            if captures.get(2).is_some() && matched.start() > 0 {
                let previous = value.as_bytes()[matched.start() - 1];
                if previous.is_ascii_alphanumeric() || previous == b'_' {
                    return matched.as_str().to_string();
                }
            }
            match environment(name.as_str()) {
                Some(value) if !value.is_empty() => value,
                _ => {
                    missing = true;
                    String::new()
                }
            }
        });
        if missing {
            bail!("basicAuth references a missing or empty environment variable");
        }
        Ok(expanded.into_owned())
    };
    let mut resolved: Vec<BasicAuth> = Vec::new();
    for entry in entries {
        let origin = expand(&entry.origin)?;
        let origin = reqwest::Url::parse(&origin)
            .ok()
            .filter(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
            })
            .ok_or_else(|| {
                anyhow::anyhow!("basicAuth origin must be an HTTP(S) origin without credentials")
            })?
            .origin()
            .ascii_serialization();
        let username = expand(&entry.username)?;
        let password = expand(&entry.password)?;
        if username.is_empty() || password.is_empty() {
            bail!("basicAuth requires a non-empty username and password");
        }
        let entry = BasicAuth {
            origin,
            username,
            password,
        };
        if let Some(previous) = resolved
            .iter_mut()
            .find(|previous| previous.origin == entry.origin)
        {
            *previous = entry;
        } else {
            resolved.push(entry);
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(origin: &str, username: &str, password: &str) -> BasicAuth {
        BasicAuth {
            origin: origin.into(),
            username: username.into(),
            password: password.into(),
        }
    }

    #[test]
    fn auth_merges_by_normalized_origin_and_expands_environment_once() {
        let entries = [
            entry("https://EXAMPLE.com:443", "workspace", "secret"),
            entry("https://second.example", "other", "pa$ssword"),
            entry("${ORIGIN}", "$USERNAME", "${PASSWORD}"),
        ];
        let resolved = resolve_with(&entries, |name| match name {
            "ORIGIN" => Some("https://example.com/".into()),
            "USERNAME" => Some("scenario".into()),
            "PASSWORD" => Some("literal$NO_EXPANSION".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].username, "scenario");
        assert_eq!(resolved[0].password, "literal$NO_EXPANSION");
        assert_eq!(resolved[1].password, "pa$ssword");
    }

    #[test]
    fn auth_errors_never_include_credentials() {
        for entry in [
            entry("file:///secret", "private", "secret"),
            entry("https://example.com", "private", "$MISSING"),
        ] {
            let error = resolve_with(&[entry], |_| None).err().unwrap().to_string();
            assert!(!error.contains("private"));
            assert!(!error.contains("secret"));
        }
    }
}
