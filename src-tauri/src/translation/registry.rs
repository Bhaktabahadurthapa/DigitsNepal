//! Provider presets. A preset is a name, a protocol, a default endpoint and
//! some flags; users can add any OpenAI-compatible service through "custom".
//!
//! Privacy rule: any endpoint that is not on this Mac (loopback) counts as
//! cloud and needs the user's explicit per-provider consent.

use super::provider::{Protocol, Provider};
use super::{anthropic, gemini, ollama, openai};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub protocol: Protocol,
    pub default_base_url: &'static str,
    /// Runs on this Mac by default.
    pub local: bool,
    /// An API key is required (false = optional or not used).
    pub needs_key: bool,
    /// The user may change the endpoint.
    pub editable_url: bool,
    /// Where to get an API key (shown in Settings).
    pub key_help: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn p(
    id: &'static str,
    name: &'static str,
    protocol: Protocol,
    url: &'static str,
    local: bool,
    needs_key: bool,
    editable_url: bool,
    key_help: &'static str,
) -> Preset {
    Preset {
        id,
        name,
        protocol,
        default_base_url: url,
        local,
        needs_key,
        editable_url,
        key_help,
    }
}

pub static PRESETS: &[Preset] = &[
    p(
        "ollama",
        "Ollama",
        Protocol::Ollama,
        ollama::DEFAULT_BASE_URL,
        true,
        false,
        true,
        "No key needed. Install from ollama.com.",
    ),
    p(
        "lmstudio",
        "LM Studio",
        Protocol::OpenAi,
        "http://127.0.0.1:1234/v1",
        true,
        false,
        true,
        "No key needed. Start the local server in LM Studio.",
    ),
    p(
        "gemini",
        "Google Gemini",
        Protocol::Gemini,
        gemini::DEFAULT_BASE_URL,
        false,
        true,
        false,
        "aistudio.google.com → Get API key",
    ),
    p(
        "anthropic",
        "Anthropic Claude",
        Protocol::Anthropic,
        anthropic::DEFAULT_BASE_URL,
        false,
        true,
        false,
        "console.anthropic.com → API Keys",
    ),
    p(
        "openai",
        "OpenAI",
        Protocol::OpenAi,
        "https://api.openai.com/v1",
        false,
        true,
        false,
        "platform.openai.com → API keys",
    ),
    p(
        "openrouter",
        "OpenRouter",
        Protocol::OpenAi,
        "https://openrouter.ai/api/v1",
        false,
        true,
        false,
        "openrouter.ai → Keys (one key, many models)",
    ),
    p(
        "groq",
        "Groq",
        Protocol::OpenAi,
        "https://api.groq.com/openai/v1",
        false,
        true,
        false,
        "console.groq.com → API Keys",
    ),
    p(
        "mistral",
        "Mistral AI",
        Protocol::OpenAi,
        "https://api.mistral.ai/v1",
        false,
        true,
        false,
        "console.mistral.ai → API Keys",
    ),
    p(
        "deepseek",
        "DeepSeek",
        Protocol::OpenAi,
        "https://api.deepseek.com/v1",
        false,
        true,
        false,
        "platform.deepseek.com → API keys",
    ),
    p(
        "together",
        "Together AI",
        Protocol::OpenAi,
        "https://api.together.xyz/v1",
        false,
        true,
        false,
        "api.together.ai → API Keys",
    ),
    p(
        "xai",
        "xAI Grok",
        Protocol::OpenAi,
        "https://api.x.ai/v1",
        false,
        true,
        false,
        "console.x.ai → API Keys",
    ),
    p(
        "custom",
        "Custom (OpenAI-compatible)",
        Protocol::OpenAi,
        "",
        false,
        false,
        true,
        "Any server with /v1/chat/completions — e.g. vLLM, llama.cpp, Azure-compatible gateways. Key optional.",
    ),
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Per-provider user configuration (stored in settings.json; keys are in Keychain).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProviderConfig {
    pub model: String,
    /// Overrides the preset endpoint where allowed. Empty = preset default.
    pub base_url: String,
    /// User confirmed that text is sent to this (non-local) provider.
    pub cloud_consent: bool,
}

pub fn keychain_account(provider_id: &str) -> String {
    format!("{provider_id}-api-key")
}

pub fn is_loopback_url(url: &str) -> bool {
    host_of(url).is_some_and(|h| matches!(h.as_str(), "localhost" | "::1") || h.starts_with("127."))
}

fn host_of(url: &str) -> Option<String> {
    let rest = url
        .trim()
        .strip_prefix("http://")
        .or_else(|| url.trim().strip_prefix("https://"))?;
    let host_port = rest.split('/').next()?;
    if host_port.contains('@') {
        return None;
    }
    Some(if let Some(h) = host_port.strip_prefix('[') {
        h.split(']').next()?.to_string()
    } else {
        host_port.split(':').next()?.to_ascii_lowercase()
    })
}

/// Resolve and validate the endpoint for a preset + config.
pub fn resolve_base_url(p: &Preset, cfg: &ProviderConfig) -> Result<String, String> {
    let url = if p.editable_url && !cfg.base_url.trim().is_empty() {
        cfg.base_url.trim()
    } else {
        p.default_base_url
    };
    let url = url.trim_end_matches('/');
    if url.is_empty() {
        return Err(format!("Enter the server address for {}.", p.name));
    }
    if url.contains('@') {
        return Err("Credentials in the address are not allowed; use the API key field.".into());
    }
    let host = host_of(url).ok_or("The address must start with http:// or https://")?;
    if host.is_empty() {
        return Err("The address has no host.".into());
    }
    if p.local && !is_loopback_url(url) {
        return Err(format!(
            "For privacy, {} must run on this Mac (localhost / 127.0.0.1). Use \"Custom\" for remote servers.",
            p.name
        ));
    }
    if url.starts_with("http://") && !is_loopback_url(url) {
        return Err("Remote servers must use https:// so your text and key are encrypted in transit.".into());
    }
    Ok(url.to_string())
}

/// True when text would leave this Mac.
pub fn is_cloud(p: &Preset, cfg: &ProviderConfig) -> bool {
    resolve_base_url(p, cfg)
        .map(|u| !is_loopback_url(&u))
        .unwrap_or(!p.local)
}

#[derive(Debug, PartialEq)]
pub enum BuildError {
    UnknownProvider(String),
    InvalidUrl(String),
    ConsentRequired(&'static str),
    MissingKey(&'static str),
}

pub fn build(id: &str, cfg: &ProviderConfig, key: Option<String>) -> Result<Box<dyn Provider>, BuildError> {
    let p = preset(id).ok_or_else(|| BuildError::UnknownProvider(id.to_string()))?;
    let url = resolve_base_url(p, cfg).map_err(BuildError::InvalidUrl)?;
    if !is_loopback_url(&url) && !cfg.cloud_consent {
        return Err(BuildError::ConsentRequired(p.name));
    }
    let key = key.filter(|k| !k.is_empty());
    if p.needs_key && key.is_none() {
        return Err(BuildError::MissingKey(p.name));
    }
    let model = cfg.model.clone();
    Ok(match p.protocol {
        Protocol::Ollama => Box::new(ollama::Ollama::new(url, model)),
        Protocol::Gemini => Box::new(gemini::Gemini::new(key.unwrap_or_default(), model, Some(url))),
        Protocol::Anthropic => Box::new(anthropic::Anthropic::new(
            id.into(),
            url,
            key.unwrap_or_default(),
            model,
        )),
        Protocol::OpenAi => Box::new(openai::OpenAiCompatible::new(
            id.into(),
            url.clone(),
            key,
            model,
            is_loopback_url(&url),
        )),
    })
}

pub fn choose_default_model(id: &str, models: &[super::provider::ModelInfo]) -> Option<String> {
    match preset(id).map(|p| p.protocol) {
        Some(Protocol::Ollama) => ollama::choose_default_model(models),
        Some(Protocol::Gemini) => gemini::choose_default_model(models),
        Some(Protocol::Anthropic) => anthropic::choose_default_model(models),
        _ => openai::choose_default_model(models),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(url: &str, consent: bool) -> ProviderConfig {
        ProviderConfig {
            model: "m".into(),
            base_url: url.into(),
            cloud_consent: consent,
        }
    }

    #[test]
    fn presets_are_unique_and_sane() {
        let mut ids = std::collections::HashSet::new();
        for p in PRESETS {
            assert!(ids.insert(p.id), "duplicate {}", p.id);
            if p.id != "custom" {
                assert!(resolve_base_url(p, &ProviderConfig::default()).is_ok(), "{}", p.id);
            }
            if !p.local && p.id != "custom" {
                assert!(p.default_base_url.starts_with("https://"), "{}", p.id);
            }
        }
    }

    #[test]
    fn cloud_requires_consent_local_does_not() {
        assert_eq!(
            build("openai", &cfg("", false), Some("k".into())).err(),
            Some(BuildError::ConsentRequired("OpenAI"))
        );
        assert!(build("openai", &cfg("", true), Some("k".into())).is_ok());
        assert!(build("ollama", &cfg("", false), None).is_ok());
        assert!(build("lmstudio", &cfg("", false), None).is_ok());
        // A custom server on localhost is local; a remote one is cloud.
        assert!(build("custom", &cfg("http://127.0.0.1:8000/v1", false), None).is_ok());
        assert!(matches!(
            build("custom", &cfg("https://llm.example.com/v1", false), None),
            Err(BuildError::ConsentRequired(_))
        ));
        assert!(build("custom", &cfg("https://llm.example.com/v1", true), None).is_ok());
    }

    #[test]
    fn keys_required_where_needed() {
        assert_eq!(
            build("anthropic", &cfg("", true), None).err(),
            Some(BuildError::MissingKey("Anthropic Claude"))
        );
        assert_eq!(
            build("groq", &cfg("", true), Some(String::new())).err(),
            Some(BuildError::MissingKey("Groq"))
        );
        assert!(
            build("custom", &cfg("https://x.example/v1", true), None).is_ok(),
            "custom key optional"
        );
    }

    #[test]
    fn url_rules() {
        let custom = preset("custom").unwrap();
        let ollama = preset("ollama").unwrap();
        let openai = preset("openai").unwrap();
        assert!(resolve_base_url(custom, &cfg("", false)).is_err());
        assert!(resolve_base_url(custom, &cfg("http://remote.example/v1", true))
            .unwrap_err()
            .contains("https"));
        assert!(resolve_base_url(custom, &cfg("https://u:p@remote.example/v1", true)).is_err());
        assert!(resolve_base_url(ollama, &cfg("http://10.0.0.5:11434", true)).is_err());
        assert!(resolve_base_url(ollama, &cfg("http://localhost.evil.com:11434", true)).is_err());
        // Non-editable presets ignore overrides.
        assert_eq!(
            resolve_base_url(openai, &cfg("https://evil.example", true)).unwrap(),
            "https://api.openai.com/v1"
        );
        assert!(is_cloud(openai, &cfg("", false)));
        assert!(!is_cloud(ollama, &cfg("", false)));
    }

    #[test]
    fn unknown_provider() {
        assert!(matches!(
            build("nope", &cfg("", true), None),
            Err(BuildError::UnknownProvider(_))
        ));
    }
}
