//! Provider abstraction. A provider turns (system, user) prompts into raw
//! text; everything language-specific lives in the engine and validator so
//! new providers only implement transport.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProviderError {
    #[error("could not reach the provider: {0}")]
    Network(String),
    #[error("the provider is not running or not reachable at the configured address")]
    Offline,
    #[error("the request timed out")]
    Timeout,
    #[error("the API key is missing or invalid")]
    Unauthorized,
    #[error("rate limited by the provider")]
    RateLimited { retry_after_secs: Option<u64> },
    #[error("model '{0}' is not available")]
    ModelUnavailable(String),
    #[error("the provider refused to answer: {0}")]
    Refused(String),
    #[error("unexpected reply from the provider: {0}")]
    BadResponse(String),
    #[error("provider error ({status}): {message}")]
    Server { status: u16, message: String },
}

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            ProviderError::Network(_) => "network",
            ProviderError::Offline => "offline",
            ProviderError::Timeout => "timeout",
            ProviderError::Unauthorized => "unauthorized",
            ProviderError::RateLimited { .. } => "rate_limited",
            ProviderError::ModelUnavailable(_) => "model_unavailable",
            ProviderError::Refused(_) => "refused",
            ProviderError::BadResponse(_) => "bad_response",
            ProviderError::Server { .. } => "server",
        }
    }

    /// Whether one automatic retry is reasonable.
    pub fn is_transient(&self) -> bool {
        match self {
            ProviderError::Network(_) | ProviderError::Timeout => true,
            ProviderError::Server { status, .. } => *status >= 500,
            ProviderError::RateLimited { retry_after_secs } => retry_after_secs.is_none_or(|s| s <= 5),
            _ => false,
        }
    }

    pub fn from_reqwest(e: &reqwest::Error) -> Self {
        if e.is_timeout() {
            ProviderError::Timeout
        } else if e.is_connect() {
            ProviderError::Offline
        } else {
            // Never include URLs (which may contain keys) in messages.
            ProviderError::Network(e.without_url_string())
        }
    }
}

trait WithoutUrl {
    fn without_url_string(&self) -> String;
}
impl WithoutUrl for reqwest::Error {
    fn without_url_string(&self) -> String {
        let mut s = self.to_string();
        if let Some(u) = self.url() {
            s = s.replace(u.as_str(), "<url>");
        }
        s
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
}

/// Wire protocol spoken by a provider preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Ollama,
    Gemini,
    Anthropic,
    /// OpenAI Chat Completions — also used by most other providers.
    OpenAi,
}

pub fn http_client(timeout: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client")
}

pub fn retry_after(h: &reqwest::header::HeaderMap) -> Option<u64> {
    h.get("retry-after")?.to_str().ok()?.parse().ok()
}

#[async_trait]
pub trait Provider: Send + Sync {
    /// Preset id, e.g. "ollama", "openai", "openrouter", "custom".
    fn id(&self) -> &str;
    fn model(&self) -> &str;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;
    async fn complete_json(&self, system: &str, user: &str) -> Result<String, ProviderError>;
}

/// Map an HTTP status + body into a ProviderError. Shared by providers.
pub fn classify_http(status: u16, body: &str, model: &str, retry_after: Option<u64>) -> ProviderError {
    let lower = body.to_lowercase();
    match status {
        401 | 403 => ProviderError::Unauthorized,
        400 if lower.contains("api key") || lower.contains("api_key") => ProviderError::Unauthorized,
        404 => ProviderError::ModelUnavailable(model.to_string()),
        400 if lower.contains("not found") && lower.contains("model") => {
            ProviderError::ModelUnavailable(model.to_string())
        }
        429 => ProviderError::RateLimited {
            retry_after_secs: retry_after,
        },
        _ => ProviderError::Server {
            status,
            message: short_error_message(body),
        },
    }
}

/// Extract a provider error message without echoing request content.
pub fn short_error_message(body: &str) -> String {
    let msg = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("error"))
                .and_then(|m| m.as_str().map(String::from))
        })
        .unwrap_or_else(|| "no details".into());
    msg.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_classification() {
        assert_eq!(classify_http(401, "", "m", None), ProviderError::Unauthorized);
        assert_eq!(
            classify_http(
                400,
                r#"{"error":{"message":"API key not valid. Please pass a valid API key."}}"#,
                "m",
                None
            ),
            ProviderError::Unauthorized
        );
        assert_eq!(
            classify_http(404, "", "gem", None),
            ProviderError::ModelUnavailable("gem".into())
        );
        assert_eq!(
            classify_http(400, r#"{"error":"model 'x' not found"}"#, "x", None),
            ProviderError::ModelUnavailable("x".into())
        );
        assert_eq!(
            classify_http(429, "", "m", Some(30)),
            ProviderError::RateLimited {
                retry_after_secs: Some(30)
            }
        );
        assert!(matches!(
            classify_http(503, "oops", "m", None),
            ProviderError::Server { status: 503, .. }
        ));
    }

    #[test]
    fn transient_errors() {
        assert!(ProviderError::Timeout.is_transient());
        assert!(ProviderError::RateLimited { retry_after_secs: None }.is_transient());
        assert!(!ProviderError::RateLimited {
            retry_after_secs: Some(60)
        }
        .is_transient());
        assert!(!ProviderError::Unauthorized.is_transient());
        assert!(!ProviderError::Offline.is_transient());
        assert!(ProviderError::Server {
            status: 502,
            message: String::new()
        }
        .is_transient());
        assert!(!ProviderError::Server {
            status: 400,
            message: String::new()
        }
        .is_transient());
    }

    #[test]
    fn error_messages_are_truncated() {
        let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(1000));
        assert_eq!(short_error_message(&long).len(), 200);
        assert_eq!(short_error_message("<html>"), "no details");
    }
}
