//! Google Gemini API provider (hosted / cloud).
//!
//! The API key is passed in the `x-goog-api-key` header (never in the URL)
//! and comes from the macOS Keychain. Model IDs are discovered at runtime.

use super::provider::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct Gemini {
    client: reqwest::Client,
    id: String,
    base_url: String,
    api_key: String,
    model: String,
}

impl Gemini {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: http_client(Duration::from_secs(60)),
            id: "gemini".into(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
            api_key,
            model,
        }
    }

    fn model_path(&self) -> String {
        let m = self.model.trim_start_matches("models/");
        format!("{}/models/{}:generateContent", self.base_url, m)
    }
}

/// Rank discovered models for translation. Preference: stable over preview,
/// "flash" (good multilingual quality at low cost) over "pro", newest version.
pub fn choose_default_model(models: &[ModelInfo]) -> Option<String> {
    let excluded = [
        "embedding",
        "tts",
        "image",
        "vision",
        "audio",
        "live",
        "aqa",
        "learnlm",
        "imagen",
        "veo",
        "robotics",
        "computer-use",
        "native",
    ];
    let mut candidates: Vec<&ModelInfo> = models
        .iter()
        .filter(|m| m.id.starts_with("gemini"))
        .filter(|m| !excluded.iter().any(|x| m.id.contains(x)))
        .collect();
    if candidates.is_empty() {
        return models.first().map(|m| m.id.clone());
    }
    fn version(id: &str) -> f32 {
        id.split('-').nth(1).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0)
    }
    fn score(id: &str) -> i32 {
        let mut s = 0;
        if !(id.contains("preview") || id.contains("exp")) {
            s += 100;
        }
        if id.contains("flash") && !id.contains("lite") {
            s += 30;
        } else if id.contains("flash-lite") {
            s += 20;
        } else if id.contains("pro") {
            s += 10;
        }
        if id.contains("latest") {
            s -= 5; // floating alias; prefer a pinned version
        }
        s
    }
    candidates.sort_by(|a, b| {
        score(&b.id)
            .cmp(&score(&a.id))
            .then(
                version(&b.id)
                    .partial_cmp(&version(&a.id))
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.id.len().cmp(&b.id.len()))
    });
    Some(candidates[0].id.clone())
}

#[async_trait]
impl Provider for Gemini {
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
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        for _ in 0..10 {
            let mut req = self
                .client
                .get(format!("{}/models", self.base_url))
                .header("x-goog-api-key", &self.api_key)
                .query(&[("pageSize", "1000")]);
            if let Some(p) = &page {
                req = req.query(&[("pageToken", p.as_str())]);
            }
            let resp = req.send().await.map_err(|e| ProviderError::from_reqwest(&e))?;
            let status = resp.status().as_u16();
            let ra = retry_after(resp.headers());
            let body = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
            if status != 200 {
                return Err(classify_http(status, &body, "", ra));
            }
            let v: Value = serde_json::from_str(&body).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
            for m in v.get("models").and_then(Value::as_array).into_iter().flatten() {
                let supports = m
                    .get("supportedGenerationMethods")
                    .and_then(Value::as_array)
                    .is_some_and(|a| a.iter().any(|x| x == "generateContent"));
                if !supports {
                    continue;
                }
                let id = m
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim_start_matches("models/")
                    .to_string();
                if id.is_empty() {
                    continue;
                }
                out.push(ModelInfo {
                    display_name: m.get("displayName").and_then(Value::as_str).unwrap_or(&id).to_string(),
                    description: m
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .chars()
                        .take(200)
                        .collect(),
                    id,
                });
            }
            page = v
                .get("nextPageToken")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
            if page.is_none() {
                break;
            }
        }
        Ok(out)
    }

    async fn complete_json(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        if self.api_key.is_empty() {
            return Err(ProviderError::Unauthorized);
        }
        if self.model.is_empty() {
            return Err(ProviderError::ModelUnavailable("(none selected)".into()));
        }
        let body = json!({
            "systemInstruction": { "parts": [{ "text": system }] },
            "contents": [{ "role": "user", "parts": [{ "text": user }] }],
            "generationConfig": { "temperature": 0.2, "responseMimeType": "application/json" }
        });
        let resp = self
            .client
            .post(self.model_path())
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::from_reqwest(&e))?;
        let status = resp.status().as_u16();
        let ra = retry_after(resp.headers());
        let text = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
        if status != 200 {
            return Err(classify_http(status, &text, &self.model, ra));
        }
        parse_generate_response(&text)
    }
}

pub fn parse_generate_response(text: &str) -> Result<String, ProviderError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
    if let Some(reason) = v.pointer("/promptFeedback/blockReason").and_then(Value::as_str) {
        return Err(ProviderError::Refused(reason.to_string()));
    }
    let cand = v
        .pointer("/candidates/0")
        .ok_or_else(|| ProviderError::BadResponse("no candidates".into()))?;
    if let Some(fr) = cand.get("finishReason").and_then(Value::as_str) {
        if matches!(
            fr,
            "SAFETY" | "RECITATION" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII"
        ) {
            return Err(ProviderError::Refused(fr.to_string()));
        }
    }
    let parts = cand
        .pointer("/content/parts")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::BadResponse("no content".into()))?;
    let joined: String = parts
        .iter()
        .filter(|p| !p.get("thought").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|p| p.get("text").and_then(Value::as_str))
        .collect();
    if joined.trim().is_empty() {
        return Err(ProviderError::BadResponse("empty reply".into()));
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mi(id: &str) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            display_name: id.into(),
            description: String::new(),
        }
    }

    #[test]
    fn picks_stable_flash_newest() {
        let models = vec![
            mi("gemini-2.0-flash"),
            mi("gemini-2.5-pro"),
            mi("gemini-2.5-flash"),
            mi("gemini-3.0-flash-preview"),
            mi("gemini-2.5-flash-lite"),
            mi("text-embedding-004"),
            mi("gemini-2.5-flash-preview-tts"),
        ];
        assert_eq!(choose_default_model(&models).as_deref(), Some("gemini-2.5-flash"));
    }

    #[test]
    fn falls_back_when_only_preview() {
        assert_eq!(
            choose_default_model(&[mi("gemini-9-pro-preview")]).as_deref(),
            Some("gemini-9-pro-preview")
        );
        assert_eq!(choose_default_model(&[]), None);
    }

    #[test]
    fn parses_text_and_skips_thoughts() {
        let r = r#"{"candidates":[{"content":{"parts":[{"text":"thinking","thought":true},{"text":"{\"a\":1}"}]},"finishReason":"STOP"}]}"#;
        assert_eq!(parse_generate_response(r).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn detects_blocks_and_empty() {
        assert!(matches!(
            parse_generate_response(r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#),
            Err(ProviderError::Refused(_))
        ));
        assert!(matches!(
            parse_generate_response(r#"{"candidates":[{"finishReason":"SAFETY","content":{"parts":[{"text":"x"}]}}]}"#),
            Err(ProviderError::Refused(_))
        ));
        assert!(matches!(
            parse_generate_response(r#"{"candidates":[]}"#),
            Err(ProviderError::BadResponse(_))
        ));
        assert!(matches!(
            parse_generate_response("nope"),
            Err(ProviderError::BadResponse(_))
        ));
    }

    #[tokio::test]
    async fn missing_key_is_unauthorized_without_network() {
        let g = Gemini::new(String::new(), "gemini-x".into(), Some("http://127.0.0.1:9".into()));
        assert_eq!(
            g.complete_json("s", "u").await.unwrap_err(),
            ProviderError::Unauthorized
        );
    }
}
