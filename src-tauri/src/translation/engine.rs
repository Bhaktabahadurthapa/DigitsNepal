//! The translation engine: input checks → detection → prompt → provider →
//! validation, with a conservative retry policy. The engine never
//! fabricates output: if the provider or validation fails, the caller gets
//! an error and the user's text is left untouched.

use super::detect::detect;
use super::prompt::{self, REPAIR_REMINDER};
use super::provider::{Provider, ProviderError};
use super::types::*;
use super::validate::{self, ValidationError};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    #[error("Please enter some text to translate.")]
    EmptyInput,
    #[error("The text is too long ({len} characters). The limit is {max}; please split it into smaller parts.")]
    TooLong { len: usize, max: usize },
    #[error("{0}")]
    Provider(#[from] ProviderError),
    #[error("The model's reply could not be used: {0}")]
    Invalid(#[from] ValidationError),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl EngineError {
    pub fn payload(&self) -> ErrorPayload {
        let (code, retryable) = match self {
            EngineError::EmptyInput => ("empty_input".to_string(), false),
            EngineError::TooLong { .. } => ("too_long".to_string(), false),
            EngineError::Provider(p) => (
                p.code().to_string(),
                p.is_transient() || matches!(p, ProviderError::Offline),
            ),
            EngineError::Invalid(_) => ("malformed_response".to_string(), true),
        };
        let message = match self {
            EngineError::Provider(ProviderError::Offline) => {
                "Could not reach the local Ollama server. Make sure Ollama is running (`ollama serve`) and the address in Settings is correct.".to_string()
            }
            EngineError::Provider(ProviderError::Unauthorized) => {
                "The Gemini API key is missing or invalid. Add a key in Settings → Providers.".to_string()
            }
            EngineError::Provider(ProviderError::RateLimited { retry_after_secs: Some(s) }) => {
                format!("The provider is rate-limiting requests. Try again in about {s} seconds.")
            }
            other => other.to_string(),
        };
        ErrorPayload {
            code,
            message,
            retryable,
        }
    }
}

pub fn check_input(text: &str) -> Result<&str, EngineError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(EngineError::EmptyInput);
    }
    let len = trimmed.chars().count();
    if len > MAX_INPUT_CHARS {
        return Err(EngineError::TooLong {
            len,
            max: MAX_INPUT_CHARS,
        });
    }
    Ok(trimmed)
}

pub struct RetryPolicy {
    pub transient_retries: u32,
    pub repair_retries: u32,
    pub backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            transient_retries: 1,
            repair_retries: 1,
            backoff: Duration::from_millis(800),
        }
    }
}

pub async fn translate(
    provider: &dyn Provider,
    req: &TranslateRequest,
    policy: &RetryPolicy,
) -> Result<TranslationResult, EngineError> {
    let text = check_input(&req.text)?;
    let detection = detect(text);
    let p = prompt::build(text, req.mode, &detection, req.include_word_meanings);
    let ctx = validate::Context {
        input: text,
        mode: req.mode,
        provider: provider.id(),
        model: provider.model(),
    };

    let mut transient_left = policy.transient_retries;
    let mut repair_left = policy.repair_retries;
    let mut user = p.user.clone();
    loop {
        match provider.complete_json(&p.system, &user).await {
            Ok(raw) => match validate::validate(&raw, &ctx) {
                Ok(r) => return Ok(r),
                Err(e) if repair_left > 0 => {
                    repair_left -= 1;
                    log::warn!("model reply failed validation ({}); retrying once", e);
                    user = format!("{}\n\n{}", p.user, REPAIR_REMINDER);
                }
                Err(e) => return Err(e.into()),
            },
            Err(e) if e.is_transient() && transient_left > 0 => {
                transient_left -= 1;
                log::warn!("transient provider error ({}); retrying once", e.code());
                tokio::time::sleep(policy.backoff).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::translation::provider::ModelInfo;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Scripted provider returning queued responses in order.
    pub struct Scripted {
        pub replies: Mutex<Vec<Result<String, ProviderError>>>,
        pub calls: Mutex<Vec<String>>,
    }
    impl Scripted {
        pub fn new(mut r: Vec<Result<String, ProviderError>>) -> Self {
            r.reverse();
            Self {
                replies: Mutex::new(r),
                calls: Mutex::new(vec![]),
            }
        }
        fn count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }
    #[async_trait]
    impl Provider for Scripted {
        fn id(&self) -> &str {
            "ollama"
        }
        fn model(&self) -> &str {
            "scripted"
        }
        async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
            Ok(vec![])
        }
        async fn complete_json(&self, _s: &str, u: &str) -> Result<String, ProviderError> {
            self.calls.lock().unwrap().push(u.to_string());
            self.replies
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(Err(ProviderError::Offline))
        }
    }

    fn fast() -> RetryPolicy {
        RetryPolicy {
            backoff: Duration::from_millis(1),
            ..Default::default()
        }
    }
    fn req(t: &str) -> TranslateRequest {
        TranslateRequest {
            text: t.into(),
            mode: TranslationMode::Accurate,
            include_word_meanings: true,
        }
    }
    const OK: &str = r#"{"accurate_english":"Why did this error occur?","detected_language":"romanized_nepali"}"#;

    #[tokio::test]
    async fn happy_path() {
        let p = Scripted::new(vec![Ok(OK.into())]);
        let r = translate(&p, &req("  Yo error kina aayo?  "), &fast()).await.unwrap();
        assert_eq!(r.accurate_english, "Why did this error occur?");
        assert_eq!(r.original_text, "Yo error kina aayo?");
        assert_eq!(r.provider, "ollama");
    }

    #[tokio::test]
    async fn empty_and_too_long_never_call_provider() {
        let p = Scripted::new(vec![]);
        assert_eq!(
            translate(&p, &req("   \n"), &fast()).await.unwrap_err(),
            EngineError::EmptyInput
        );
        let long = "a".repeat(MAX_INPUT_CHARS + 1);
        assert!(matches!(
            translate(&p, &req(&long), &fast()).await.unwrap_err(),
            EngineError::TooLong { .. }
        ));
        assert_eq!(p.count(), 0);
    }

    #[tokio::test]
    async fn malformed_reply_is_repaired_once() {
        let p = Scripted::new(vec![Ok("not json".into()), Ok(OK.into())]);
        assert!(translate(&p, &req("Yo error kina aayo?"), &fast()).await.is_ok());
        assert_eq!(p.count(), 2);
        assert!(p.calls.lock().unwrap()[1].contains(REPAIR_REMINDER));
    }

    #[tokio::test]
    async fn malformed_twice_fails_without_fabrication() {
        let p = Scripted::new(vec![Ok("nope".into()), Ok("{}".into())]);
        let e = translate(&p, &req("Yo error kina aayo?"), &fast()).await.unwrap_err();
        assert!(matches!(e, EngineError::Invalid(_)));
        assert_eq!(e.payload().code, "malformed_response");
    }

    #[tokio::test]
    async fn transient_retry_once() {
        let p = Scripted::new(vec![Err(ProviderError::Timeout), Ok(OK.into())]);
        assert!(translate(&p, &req("Yo error kina aayo?"), &fast()).await.is_ok());
        let p = Scripted::new(vec![
            Err(ProviderError::Timeout),
            Err(ProviderError::Timeout),
            Ok(OK.into()),
        ]);
        assert_eq!(
            translate(&p, &req("x y z"), &fast()).await.unwrap_err(),
            EngineError::Provider(ProviderError::Timeout)
        );
    }

    #[tokio::test]
    async fn auth_errors_do_not_retry() {
        let p = Scripted::new(vec![Err(ProviderError::Unauthorized), Ok(OK.into())]);
        let e = translate(&p, &req("Yo error kina aayo?"), &fast()).await.unwrap_err();
        assert_eq!(p.count(), 1);
        assert_eq!(e.payload().code, "unauthorized");
        assert!(e.payload().message.contains("Settings"));
    }

    #[tokio::test]
    async fn long_rate_limit_not_retried() {
        let p = Scripted::new(vec![Err(ProviderError::RateLimited {
            retry_after_secs: Some(40),
        })]);
        let e = translate(&p, &req("Yo error kina aayo?"), &fast()).await.unwrap_err();
        assert!(e.payload().message.contains("40 seconds"));
        assert_eq!(p.count(), 1);
    }

    #[tokio::test]
    async fn offline_message_is_actionable() {
        let p = Scripted::new(vec![Err(ProviderError::Offline)]);
        let e = translate(&p, &req("Yo error kina aayo?"), &fast()).await.unwrap_err();
        assert!(e.payload().message.contains("ollama serve"));
    }
}
