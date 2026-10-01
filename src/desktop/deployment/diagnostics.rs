use anyhow::{Error, anyhow};
use futures::StreamExt;
use reqwest::{Response, header::HeaderValue};
use serde_json::{Value, json};

use super::DeploymentProvider;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) async fn http_error(
    response: Response,
    provider: DeploymentProvider,
    authorization: &HeaderValue,
) -> Error {
    let status = response.status().as_u16();
    let header = |names: &[&str]| {
        names
            .iter()
            .find_map(|name| response.headers().get(*name))
            .and_then(|value| value.to_str().ok())
            .filter(|value| {
                value.len() <= 128
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            })
            .map(str::to_owned)
    };
    let request_id = header(&["x-ms-request-id", "x-nf-request-id", "x-request-id"]);
    let code = header(&["x-ms-error-code"]);
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if bytes.len() + chunk.len() <= MAX_RESPONSE_BYTES => {
                bytes.extend_from_slice(&chunk);
            }
            _ => {
                bytes.clear();
                break;
            }
        }
    }
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let error = failure(
        provider,
        "request",
        Some(status),
        &json!({"code": code, "error": value}),
        authorization,
    );
    match request_id {
        Some(id) => anyhow!(
            "{error} (provider request ID: {})",
            sanitize(&id, authorization)
        ),
        None => error,
    }
}

pub(super) fn failure(
    provider: DeploymentProvider,
    operation: &str,
    status: Option<u16>,
    value: &Value,
    authorization: &HeaderValue,
) -> Error {
    let mut messages = Vec::new();
    collect_details(value, 0, &mut messages);
    let detail = sanitize(&messages.join("; "), authorization);
    let lower = detail.to_ascii_lowercase();
    let size_error = status == Some(413)
        || ["size", "storage", "too large", "quota", "megabyte"]
            .iter()
            .any(|word| lower.contains(word));
    let hint = match (provider, status) {
        (DeploymentProvider::AzureSwa, _) if size_error => {
            "Check Azure storage usage and unused preview environments; deploy fewer suites or upgrade Free (250 MB per environment) to Standard (500 MB per environment)."
        }
        (DeploymentProvider::Netlify, _) if size_error => {
            "Check this Netlify project's deploy limits and team usage; deploy fewer suites or increase the plan's allowance."
        }
        (_, Some(401 | 403)) => {
            "Check that the deployment token is current and has access to this target; verify team/SSO authorization and deployment restrictions."
        }
        (_, Some(404)) => "Check the target/Site ID and the deployment token's access to it.",
        (DeploymentProvider::Netlify, Some(402)) => {
            "Check Netlify team billing, credits, and usage limits; the project may be paused."
        }
        (_, Some(408 | 504)) => {
            "The provider timed out; check its deployment history before retrying."
        }
        (_, Some(429)) => "The provider rate limit was reached; wait before retrying.",
        (_, Some(500..=599)) => {
            "The provider encountered a server error; check its status page and deployment history before retrying."
        }
        (DeploymentProvider::AzureSwa, _) => {
            "Check the Static Web App deployment history and configuration in the Azure portal."
        }
        (DeploymentProvider::Netlify, _) => {
            "Check the project's deploy log and configuration in Netlify."
        }
        (DeploymentProvider::CloudflareWorkers, _) => {
            "Check the Cloudflare Account ID, Worker name, Workers Scripts Write permission, and the Worker's deployment history."
        }
    };
    let status = status
        .map(|status| format!(" (HTTP {status})"))
        .unwrap_or_default();
    let detail = if detail.is_empty() {
        "Provider returned no readable error detail.".to_string()
    } else {
        detail
    };
    anyhow!(
        "{} {operation} failed{status}: {detail} {hint}",
        provider.label()
    )
}

fn collect_details(value: &Value, depth: usize, messages: &mut Vec<String>) {
    if depth > 6 || messages.len() >= 8 {
        return;
    }
    match value {
        Value::String(message) if !message.trim().is_empty() => {
            if let Ok(nested @ (Value::Object(_) | Value::Array(_))) =
                serde_json::from_str::<Value>(message)
            {
                collect_details(&nested, depth + 1, messages);
            } else if !messages.contains(message) {
                messages.push(message.clone());
            }
        }
        Value::Array(values) => {
            for value in values.iter().take(8) {
                collect_details(value, depth + 1, messages);
            }
        }
        Value::Object(object) => {
            for field in [
                "code",
                "errorCode",
                "Code",
                "message",
                "Message",
                "error_message",
                "errorMessage",
                "deploymentErrorMessage",
                "statusMessage",
                "error",
                "Error",
                "errors",
                "Errors",
                "details",
                "errorDetails",
                "response",
                "unhealthyRegions",
            ] {
                if let Some(value) = object.get(field) {
                    collect_details(value, depth + 1, messages);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn sanitize(message: &str, authorization: &HeaderValue) -> String {
    crate::diagnostics::redaction::sanitize(message, authorization)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_preserve_provider_reason_without_credentials_or_signed_urls() {
        let authorization = HeaderValue::from_static("Bearer synthetic-secret-token");
        let value = json!({"error": {"code":"StorageLimitExceeded", "message":
            "Report is too large: 290 MB; token=synthetic-secret-token https://user:password@storage.blob.core.windows.net/app?sig=private",
            "details":[{"message":"Maximum size is 250 MB"}], "token":"never-print-this"}});
        let error = failure(
            DeploymentProvider::AzureSwa,
            "deployment",
            Some(400),
            &value,
            &authorization,
        )
        .to_string();
        assert!(error.contains("StorageLimitExceeded"));
        assert!(error.contains("Maximum size is 250 MB"));
        assert!(error.contains("290 MB"));
        for secret in [
            "synthetic-secret-token",
            "password",
            "private",
            "never-print-this",
        ] {
            assert!(!error.contains(secret), "{error}");
        }
    }

    #[test]
    fn details_are_bounded_and_support_json_encoded_errors() {
        let authorization = HeaderValue::from_static("token secret");
        let nested = json!({"response": json!({"errorMessage":"Too many files"}).to_string()});
        assert!(
            failure(
                DeploymentProvider::Netlify,
                "deployment",
                None,
                &nested,
                &authorization
            )
            .to_string()
            .contains("Too many files")
        );
        let error = failure(
            DeploymentProvider::Netlify,
            "request",
            Some(402),
            &json!({"message":"x".repeat(10_000)}),
            &authorization,
        )
        .to_string();
        assert!(error.len() < 2500);
        assert!(error.contains("billing"));
    }
}
