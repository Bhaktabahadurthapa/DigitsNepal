//! Ollama provider (local inference on the user's machine).
//!
//! By default only loopback addresses are accepted, so "offline" really
//! means the text never leaves the computer. See `security::validate_ollama_url`.

use super::provider::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";

pub struct Ollama {
    client: reqwest::Client,
    id: String,
    base_url: String,
    model: String,
}

impl Ollama {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            // Local models can be slow on first load.
            client: http_client(Duration::from_secs(180)),
            id: "ollama".into(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        }
    }
}

/// Prefer model families with reasonable multilingual ability. This is a
/// starting point only; quality must be measured with the eval dataset.
pub fn choose_default_model(models: &[ModelInfo]) -> Option<String> {
    const PREFERRED: &[&str] = &[
        "gemma3", "gemma", "qwen3", "qwen2.5", "qwen", "aya", "llama3", "mistral",
    ];
    let usable: Vec<&ModelInfo> = models.iter().filter(|m| !m.id.contains("embed")).collect();
    for fam in PREFERRED {
        if let Some(m) = usable.iter().find(|m| m.id.starts_with(fam)) {
            return Some(m.id.clone());
        }
    }
    usable.first().map(|m| m.id.clone())
}

#[async_trait]
impl Provider for Ollama {
    fn id(&self) -> &str {
        &self.id
    }
    fn model(&self) -> &str {
        &self.model
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| ProviderError::from_reqwest(&e))?;
        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
        if status != 200 {
            return Err(classify_http(status, &body, "", None));
        }
        let v: Value = serde_json::from_str(&body).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
        Ok(v.get("models")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let id = m.get("name").or_else(|| m.get("model"))?.as_str()?.to_string();
                let size = m
                    .pointer("/details/parameter_size")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                Some(ModelInfo {
                    display_name: id.clone(),
                    description: size.to_string(),
                    id,
                })
            })
            .collect())
    }

    async fn complete_json(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        if self.model.is_empty() {
            return Err(ProviderError::ModelUnavailable("(none selected)".into()));
        }
        let body = json!({
            "model": self.model,
            "stream": false,
            "format": "json",
            "options": { "temperature": 0.2 },
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });
        let resp = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::from_reqwest(&e))?;
        let status = resp.status().as_u16();
        let text = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
        if status != 200 {
            return Err(classify_http(status, &text, &self.model, None));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
        let content = v
            .pointer("/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| ProviderError::BadResponse("no message content".into()))?;
        if content.trim().is_empty() {
            return Err(ProviderError::BadResponse("empty reply".into()));
        }
        Ok(content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Minimal one-shot HTTP server for transport tests (no extra deps).
    fn serve_once(status: u16, body: &'static str) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 65536];
                let _ = s.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(resp.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn chat_success() {
        let url = serve_once(
            200,
            r#"{"message":{"role":"assistant","content":"{\"accurate_english\":\"hi\"}"},"done":true}"#,
        );
        let o = Ollama::new(url, "gemma3".into());
        assert_eq!(o.complete_json("s", "u").await.unwrap(), r#"{"accurate_english":"hi"}"#);
    }

    #[tokio::test]
    async fn model_missing() {
        let url = serve_once(404, r#"{"error":"model \"nope\" not found, try pulling it first"}"#);
        let o = Ollama::new(url, "nope".into());
        assert_eq!(
            o.complete_json("s", "u").await.unwrap_err(),
            ProviderError::ModelUnavailable("nope".into())
        );
    }

    #[tokio::test]
    async fn not_running_is_offline() {
        // Port 9 (discard) is essentially never listening on macOS.
        let o = Ollama::new("http://127.0.0.1:9".into(), "m".into());
        assert_eq!(o.complete_json("s", "u").await.unwrap_err(), ProviderError::Offline);
    }

    #[tokio::test]
    async fn list_models_parses_tags() {
        let url = serve_once(
            200,
            r#"{"models":[{"name":"qwen2.5:7b","details":{"parameter_size":"7.6B"}},{"name":"nomic-embed-text"}]}"#,
        );
        let o = Ollama::new(url, String::new());
        let m = o.list_models().await.unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(choose_default_model(&m).as_deref(), Some("qwen2.5:7b"));
    }

    #[test]
    fn default_model_skips_embeddings() {
        let m = vec![ModelInfo {
            id: "nomic-embed-text".into(),
            display_name: String::new(),
            description: String::new(),
        }];
        assert_eq!(choose_default_model(&m), None);
    }
}
