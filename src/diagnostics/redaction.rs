use std::sync::LazyLock;

use regex::{Captures, Regex};
use reqwest::{Url, header::HeaderValue};

const MAX_DETAIL_CHARS: usize = 2048;
pub(crate) const MAX_SOURCE_BYTES: usize = 2048;

pub(crate) fn sanitize_source(value: &str) -> String {
    static URLS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)https?://[^\s<>"']+"#).expect("valid URL pattern"));
    static PARAMETERS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"([?&#])([^=&\s<>"']+)=([^&#\s<>"']*)"#)
            .expect("valid source parameter pattern")
    });
    let value = URLS.replace_all(value, |capture: &Captures<'_>| {
        let Ok(mut url) = Url::parse(&capture[0]) else {
            return "[redacted URL]".to_owned();
        };
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.to_string()
    });
    let value = PARAMETERS.replace_all(&value, |capture: &Captures<'_>| {
        let mut parsed = Url::parse("https://diagnostic.invalid/").expect("valid static URL");
        parsed.set_query(Some(&capture[0][1..]));
        let sensitive = parsed.query_pairs().next().is_some_and(|(key, _)| {
            matches!(
                key.to_ascii_lowercase().replace(['_', '-'], "").as_str(),
                "authorization"
                    | "password"
                    | "secret"
                    | "clientsecret"
                    | "signature"
                    | "sig"
                    | "token"
                    | "apikey"
                    | "accesstoken"
                    | "deploymenttoken"
                    | "refreshtoken"
            )
        });
        if sensitive {
            format!("{}{}=[redacted]", &capture[1], &capture[2])
        } else {
            capture[0].to_owned()
        }
    });
    let value: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                ' '
            } else {
                character
            }
        })
        .collect();
    let mut value = value.trim().to_owned();
    if value.is_empty() {
        return "[empty]".into();
    }
    if value.len() > MAX_SOURCE_BYTES {
        let mut boundary = MAX_SOURCE_BYTES - 3;
        while !value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        value.truncate(boundary);
        value.push_str("...");
    }
    value
}

pub(crate) fn sanitize(message: &str, authorization: &HeaderValue) -> String {
    let token = authorization
        .to_str()
        .ok()
        .and_then(|header| header.split_once(' '))
        .map(|(_, token)| token)
        .unwrap_or_default();
    sanitize_known(message, &[token])
}

pub(crate) fn sanitize_known(message: &str, secrets: &[&str]) -> String {
    static URLS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"https?://[^\s<>"']+"#).expect("valid URL pattern"));
    static SECRETS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?i)\b(?:authorization|password|secret|signature|sig|token|api[_-]?key|access[_-]?token|deployment[_-]?token)\s*[=:]\s*["']?[^\s,;"'<>]+|\bBearer\s+[^\s,;"'<>]+"#,
        )
        .expect("valid credential pattern")
    });
    let mut message = message.to_owned();
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        message = message.replace(secret, "[redacted]");
    }
    let message = URLS.replace_all(&message, |capture: &Captures<'_>| {
        Url::parse(&capture[0])
            .ok()
            .and_then(|url| {
                url.host_str()
                    .map(|host| format!("{}://{host}/[redacted]", url.scheme()))
            })
            .unwrap_or_else(|| "[redacted URL]".into())
    });
    let message = SECRETS.replace_all(&message, "[redacted]");
    let message: String = message
        .chars()
        .filter(|character| !character.is_control() || character.is_whitespace())
        .collect();
    let message = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if message.chars().count() > MAX_DETAIL_CHARS {
        format!(
            "{}...",
            message.chars().take(MAX_DETAIL_CHARS).collect::<String>()
        )
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_metadata_keeps_locations_but_removes_sensitive_values() {
        for source in [
            "search-functions:keyword-search-button",
            "common/_search-ready.js",
            "input[placeholder=\"Diseases type to search\"]",
            "[data-secret=\"literal selector text\"]",
            "https://example.test/search-results/",
            "https://example.test/search-results/?q=heart&sort=newest#results",
            "**/search-results/?q=heart+attack",
        ] {
            assert_eq!(sanitize_source(source), source);
        }
        for source in [
            "https://person:password@example.test/search-results/?q=heart&token=private-token#access_token=private-fragment",
            "HTTPS://person:password@EXAMPLE.test/search-results/?q=heart&TOKEN=private-token",
            "**/search-results/?q=heart&api_key=private-key",
            "https://example.test/?t%6fken=private-token",
        ] {
            let output = sanitize_source(source);
            for secret in ["person", "password", "private"] {
                assert!(!output.contains(secret), "{output}");
            }
            assert_eq!(sanitize_source(&output), output);
        }
        assert_eq!(
            sanitize_source("https://example.test/search-results/?q=heart&token=private-token"),
            "https://example.test/search-results/?q=heart&token=[redacted]"
        );
    }

    #[test]
    fn source_metadata_is_bounded_printable_and_idempotent() {
        for source in ["", "\n\t", "a\u{1b}\u{202e}b", &"\u{1f600}".repeat(3000)] {
            let output = sanitize_source(source);
            assert!(!output.is_empty());
            assert!(output.len() <= MAX_SOURCE_BYTES);
            assert!(!output.chars().any(char::is_control));
            assert!(!output.contains('\u{202e}'));
            assert_eq!(sanitize_source(&output), output);
        }
    }

    #[test]
    fn known_secrets_urls_and_controls_are_removed_before_truncation() {
        let input = format!(
            "\u{1b}[31mfirst-secret second-secret https://user:password@example.test/path?sig=private {}",
            "x".repeat(3000)
        );
        let output = sanitize_known(&input, &["first-secret", "second-secret"]);
        for secret in [
            "first-secret",
            "second-secret",
            "user",
            "password",
            "private",
            "\u{1b}",
        ] {
            assert!(!output.contains(secret), "{output}");
        }
        assert!(output.chars().count() <= MAX_DETAIL_CHARS + 3);
        assert!(output.contains("https://example.test/[redacted]"));
    }
}
