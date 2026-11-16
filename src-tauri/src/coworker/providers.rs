//! Bridge from DigitsNepal's provider settings (Settings → AI provider, keys in
//! the Keychain) to the agent engine's model connections.

use crate::translation::provider::Protocol;
use crate::translation::registry::{self, ProviderConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Api {
    ChatCompletions,
    Anthropic,
}

#[derive(Clone)]
pub struct ProviderSpec {
    /// Engine connection name; changes when the endpoint changes, because the
    /// engine treats a connection's identity as immutable.
    pub connection: String,
    pub api: Api,
    pub base_url: String,
    pub key: Option<String>,
    pub model: String,
    pub cloud: bool,
}

impl std::fmt::Debug for ProviderSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderSpec")
            .field("connection", &self.connection)
            .field("api", &self.api)
            .field("base_url", &self.base_url)
            .field("key", &self.key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .finish()
    }
}

pub fn spec_for(id: &str, cfg: &ProviderConfig, key: Option<String>) -> Result<ProviderSpec, String> {
    let p = registry::preset(id).ok_or_else(|| format!("Unknown provider '{id}'."))?;
    let base = registry::resolve_base_url(p, cfg)?;
    let cloud = !registry::is_loopback_url(&base);
    if cloud && !cfg.cloud_consent {
        return Err(format!(
            "Allow DigitsNepal to send text to {} in Settings → AI provider first.",
            p.name
        ));
    }
    if p.needs_key && key.as_deref().is_none_or(str::is_empty) {
        return Err(format!("Add your {} API key in Settings → AI provider.", p.name));
    }
    let model = cfg.model.trim().to_string();
    if model.is_empty() {
        return Err(format!("Choose a model for {} in Settings → AI provider.", p.name));
    }
    let (api, base_url) = match p.protocol {
        Protocol::Anthropic => (Api::Anthropic, base),
        // Ollama and Gemini both serve the OpenAI-compatible Chat Completions API.
        Protocol::Ollama => (Api::ChatCompletions, format!("{base}/v1")),
        Protocol::Gemini => (Api::ChatCompletions, format!("{base}/openai")),
        Protocol::OpenAi => (Api::ChatCompletions, base),
    };
    let fp = &super::ledger::sha256_hex(base_url.as_bytes())[..8];
    Ok(ProviderSpec {
        connection: format!("{id}-{fp}"),
        api,
        base_url,
        key,
        model,
        cloud,
    })
}

#[cfg(feature = "coworker")]
pub fn to_engine(spec: &ProviderSpec) -> openworker_sdk::ModelProvider {
    use openworker_sdk::{Credentials, ModelProvider};
    let mut p = match spec.api {
        Api::Anthropic => ModelProvider::anthropic(spec.connection.clone()),
        Api::ChatCompletions => ModelProvider::chat_completions(spec.connection.clone()),
    }
    .base_url(spec.base_url.clone())
    .stream_usage(true);
    if let Some(k) = &spec.key {
        p = p.credentials(Credentials::secure(k.clone()).memory_only());
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(model: &str, consent: bool) -> ProviderConfig {
        ProviderConfig {
            model: model.into(),
            base_url: String::new(),
            cloud_consent: consent,
        }
    }

    #[test]
    fn maps_each_protocol_to_an_engine_endpoint() {
        let o = spec_for("ollama", &cfg("qwen3", false), None).unwrap();
        assert_eq!(
            (o.api.clone(), o.base_url.as_str(), o.cloud),
            (Api::ChatCompletions, "http://127.0.0.1:11434/v1", false)
        );
        let g = spec_for("gemini", &cfg("gemini-x", true), Some("k".into())).unwrap();
        assert_eq!(g.base_url, "https://generativelanguage.googleapis.com/v1beta/openai");
        let a = spec_for("anthropic", &cfg("claude-x", true), Some("k".into())).unwrap();
        assert_eq!(
            (a.api, a.base_url.as_str()),
            (Api::Anthropic, "https://api.anthropic.com/v1")
        );
        let r = spec_for("openrouter", &cfg("m", true), Some("k".into())).unwrap();
        assert_eq!(r.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn enforces_consent_key_and_model() {
        assert!(spec_for("gemini", &cfg("m", false), Some("k".into()))
            .unwrap_err()
            .contains("Allow"));
        assert!(spec_for("gemini", &cfg("m", true), None)
            .unwrap_err()
            .contains("API key"));
        assert!(spec_for("ollama", &cfg("", false), None)
            .unwrap_err()
            .contains("Choose a model"));
    }

    #[test]
    fn debug_never_prints_keys() {
        let g = spec_for("gemini", &cfg("m", true), Some("AIzaSECRET".into())).unwrap();
        assert!(!format!("{g:?}").contains("SECRET"));
    }
}
