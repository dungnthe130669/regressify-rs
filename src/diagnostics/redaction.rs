use std::sync::LazyLock;

use regex::{Captures, Regex};
use reqwest::{Url, header::HeaderValue};

const MAX_DETAIL_CHARS: usize = 2048;

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
