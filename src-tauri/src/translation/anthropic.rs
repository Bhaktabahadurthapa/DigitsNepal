//! Anthropic Messages API provider (Claude models, cloud).
//! Key goes in the `x-api-key` header; model IDs are discovered at runtime.

use super::provider::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

pub struct Anthropic {
    client: reqwest::Client,
    id: String,
    base_url: String,
    api_key: String,
    model: String,
}

impl Anthropic {
    pub fn new(id: String, base_url: String, api_key: String, model: String) -> Self {
        Self {
            client: http_client(Duration::from_secs(90)),
            id,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
        }
    }
    fn req(&self, r: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        r.header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
    }
}

/// The models endpoint lists newest first; prefer the newest mid-size model
/// ("sonnet"), then the small fast one ("haiku"), then anything.
pub fn choose_default_model(models: &[ModelInfo]) -> Option<String> {
    ["sonnet", "haiku", "opus"]
        .iter()
        .find_map(|fam| models.iter().find(|m| m.id.contains(fam)))
        .or_else(|| models.first())
        .map(|m| m.id.clone())
}

pub fn parse_messages_response(text: &str) -> Result<String, ProviderError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
    if v.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        return Err(ProviderError::Refused("refusal".into()));
    }
    let joined: String = v
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::BadResponse("no content".into()))?
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect();
    if joined.trim().is_empty() {
        return Err(ProviderError::BadResponse("empty reply".into()));
    }
    Ok(joined)
}

#[async_trait]
impl Provider for Anthropic {
    fn id(&self) -> &str {
        &self.id
    }
    fn model(&self) -> &str {
        &self.model
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        if self.api_key.is_empty() {
            return Err(ProviderError::Unauthorized);
        }
        let resp = self
            .req(
                self.client
                    .get(format!("{}/models", self.base_url))
                    .query(&[("limit", "1000")]),
            )
            .send()
            .await
            .map_err(|e| ProviderError::from_reqwest(&e))?;
        let (status, ra) = (resp.status().as_u16(), retry_after(resp.headers()));
        let body = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
        if status != 200 {
            return Err(classify_http(status, &body, "", ra));
        }
        let v: Value = serde_json::from_str(&body).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let id = m.get("id")?.as_str()?.to_string();
                let name = m.get("display_name").and_then(Value::as_str).unwrap_or(&id).to_string();
                Some(ModelInfo {
                    id,
                    display_name: name,
                    description: String::new(),
                })
            })
            .collect())
    }

    async fn complete_json(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        if self.api_key.is_empty() {
            return Err(ProviderError::Unauthorized);
        }
        if self.model.is_empty() {
            return Err(ProviderError::ModelUnavailable("(none selected)".into()));
        }
        let body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "temperature": 0.2,
            "system": system,
            "messages": [{ "role": "user", "content": user }]
        });
        let resp = self
            .req(self.client.post(format!("{}/messages", self.base_url)))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::from_reqwest(&e))?;
        let (status, ra) = (resp.status().as_u16(), retry_after(resp.headers()));
        let text = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
        if status != 200 {
            return Err(classify_http(status, &text, &self.model, ra));
        }
        parse_messages_response(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_blocks() {
        let r = r#"{"content":[{"type":"thinking","thinking":"..."},{"type":"text","text":"{\"a\":"},{"type":"text","text":"1}"}],"stop_reason":"end_turn"}"#;
        assert_eq!(parse_messages_response(r).unwrap(), "{\"a\":1}");
        assert!(matches!(
            parse_messages_response(r#"{"content":[],"stop_reason":"refusal"}"#),
            Err(ProviderError::Refused(_))
        ));
        assert!(matches!(
            parse_messages_response(r#"{"content":[]}"#),
            Err(ProviderError::BadResponse(_))
        ));
    }

    #[test]
    fn default_model_prefers_sonnet_in_listed_order() {
        let m = |id: &str| ModelInfo {
            id: id.into(),
            display_name: id.into(),
            description: String::new(),
        };
        let list = vec![
            m("claude-opus-x"),
            m("claude-sonnet-new"),
            m("claude-haiku-y"),
            m("claude-sonnet-old"),
        ];
        assert_eq!(choose_default_model(&list).as_deref(), Some("claude-sonnet-new"));
        assert_eq!(choose_default_model(&[]), None);
    }

    #[tokio::test]
    async fn missing_key() {
        let a = Anthropic::new(
            "anthropic".into(),
            "http://127.0.0.1:9".into(),
            String::new(),
            "m".into(),
        );
        assert_eq!(
            a.complete_json("s", "u").await.unwrap_err(),
            ProviderError::Unauthorized
        );
    }
}
