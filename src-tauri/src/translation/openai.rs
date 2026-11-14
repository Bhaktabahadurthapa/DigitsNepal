//! OpenAI-compatible Chat Completions provider.
//!
//! One transport covers OpenAI, OpenRouter, Groq, Mistral, DeepSeek,
//! Together, xAI, LM Studio, llama.cpp, vLLM and any other server exposing
//! `/chat/completions` and `/models`.
//!
//! Providers differ in which optional parameters they accept. If a request
//! with `response_format` / `temperature` is rejected with HTTP 400, we retry
//! once with a minimal body (the validator still enforces JSON).

use super::provider::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

pub struct OpenAiCompatible {
    client: reqwest::Client,
    id: String,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl OpenAiCompatible {
    pub fn new(id: String, base_url: String, api_key: Option<String>, model: String, local: bool) -> Self {
        Self {
            client: http_client(Duration::from_secs(if local { 180 } else { 90 })),
            id,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.filter(|k| !k.is_empty()),
            model,
        }
    }

    fn auth(&self, r: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(k) => r.bearer_auth(k),
            None => r,
        }
    }

    async fn post(&self, body: &Value) -> Result<(u16, Option<u64>, String), ProviderError> {
        let resp = self
            .auth(self.client.post(format!("{}/chat/completions", self.base_url)))
            .header("X-Title", "DigitsNepal") // OpenRouter attribution; ignored elsewhere
            .json(body)
            .send()
            .await
            .map_err(|e| ProviderError::from_reqwest(&e))?;
        let (status, ra) = (resp.status().as_u16(), retry_after(resp.headers()));
        let text = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
        Ok((status, ra, text))
    }
}

const NON_CHAT: &[&str] = &[
    "embed",
    "whisper",
    "tts",
    "dall-e",
    "moderation",
    "image",
    "audio",
    "transcribe",
    "realtime",
    "search",
    "rerank",
    "guard",
];

pub fn is_chat_model(id: &str) -> bool {
    let l = id.to_lowercase();
    !NON_CHAT.iter().any(|x| l.contains(x))
}

/// No hard-coded IDs: suggest the first chat-capable model the server lists.
pub fn choose_default_model(models: &[ModelInfo]) -> Option<String> {
    models.iter().find(|m| is_chat_model(&m.id)).map(|m| m.id.clone())
}

pub fn parse_chat_response(text: &str) -> Result<String, ProviderError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
    let choice = v
        .pointer("/choices/0")
        .ok_or_else(|| ProviderError::BadResponse("no choices".into()))?;
    if choice.get("finish_reason").and_then(Value::as_str) == Some("content_filter") {
        return Err(ProviderError::Refused("content_filter".into()));
    }
    let msg = choice
        .get("message")
        .ok_or_else(|| ProviderError::BadResponse("no message".into()))?;
    if let Some(r) = msg.get("refusal").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        return Err(ProviderError::Refused(r.chars().take(120).collect()));
    }
    let content = match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        // Some servers return content parts.
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect(),
        _ => String::new(),
    };
    if content.trim().is_empty() {
        return Err(ProviderError::BadResponse("empty reply".into()));
    }
    Ok(content)
}

fn rejects_optional_params(status: u16, body: &str) -> bool {
    let l = body.to_lowercase();
    status == 400
        && [
            "response_format",
            "json_object",
            "temperature",
            "unsupported",
            "not supported",
        ]
        .iter()
        .any(|k| l.contains(k))
}

#[async_trait]
impl Provider for OpenAiCompatible {
    fn id(&self) -> &str {
        &self.id
    }
    fn model(&self) -> &str {
        &self.model
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let resp = self
            .auth(self.client.get(format!("{}/models", self.base_url)))
            .send()
            .await
            .map_err(|e| ProviderError::from_reqwest(&e))?;
        let (status, ra) = (resp.status().as_u16(), retry_after(resp.headers()));
        let body = resp.text().await.map_err(|e| ProviderError::from_reqwest(&e))?;
        if status != 200 {
            return Err(classify_http(status, &body, "", ra));
        }
        let v: Value = serde_json::from_str(&body).map_err(|e| ProviderError::BadResponse(e.to_string()))?;
        let mut out: Vec<ModelInfo> = v
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let id = m.get("id")?.as_str()?.to_string();
                let name = m.get("name").and_then(Value::as_str).unwrap_or(&id).to_string();
                Some(ModelInfo {
                    id,
                    display_name: name,
                    description: String::new(),
                })
            })
            .filter(|m| is_chat_model(&m.id))
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    async fn complete_json(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        if self.model.is_empty() {
            return Err(ProviderError::ModelUnavailable("(none selected)".into()));
        }
        let messages = json!([
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]);
        let full = json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.2,
            "response_format": { "type": "json_object" }
        });
        let (mut status, mut ra, mut text) = self.post(&full).await?;
        if rejects_optional_params(status, &text) {
            let minimal = json!({ "model": self.model, "messages": messages });
            (status, ra, text) = self.post(&minimal).await?;
        }
        if status != 200 {
            return Err(classify_http(status, &text, &self.model, ra));
        }
        parse_chat_response(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Serves the given responses in order and records request bodies.
    fn serve(responses: Vec<(u16, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut s, _)) = l.accept() else { return };
                let mut buf = vec![0u8; 65536];
                let n = s.read(&mut buf).unwrap_or(0);
                seen2
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(resp.as_bytes());
            }
        });
        (format!("http://{addr}/v1"), seen)
    }

    const OK: &str = r#"{"choices":[{"message":{"role":"assistant","content":"{\"accurate_english\":\"hi\"}"},"finish_reason":"stop"}]}"#;

    #[tokio::test]
    async fn sends_bearer_and_json_mode() {
        let (url, seen) = serve(vec![(200, OK)]);
        let p = OpenAiCompatible::new("openai".into(), url, Some("sk-test".into()), "m".into(), false);
        assert_eq!(p.complete_json("s", "u").await.unwrap(), r#"{"accurate_english":"hi"}"#);
        let req = seen.lock().unwrap()[0].to_lowercase();
        assert!(req.contains("authorization: bearer sk-test"));
        assert!(req.contains("json_object"));
        assert!(req.starts_with("post /v1/chat/completions"));
    }

    #[tokio::test]
    async fn falls_back_when_params_unsupported() {
        let (url, seen) = serve(vec![
            (400, r#"{"error":{"message":"Unsupported parameter: 'temperature'"}}"#),
            (200, OK),
        ]);
        let p = OpenAiCompatible::new("custom".into(), url, None, "m".into(), true);
        assert!(p.complete_json("s", "u").await.is_ok());
        let reqs = seen.lock().unwrap();
        assert_eq!(reqs.len(), 2);
        assert!(!reqs[1].contains("temperature"));
        assert!(
            !reqs[0].to_lowercase().contains("authorization"),
            "no key → no auth header"
        );
    }

    #[tokio::test]
    async fn auth_and_model_errors() {
        let (url, _) = serve(vec![(401, r#"{"error":{"message":"Incorrect API key"}}"#)]);
        let p = OpenAiCompatible::new("openai".into(), url, Some("bad".into()), "m".into(), false);
        assert_eq!(
            p.complete_json("s", "u").await.unwrap_err(),
            ProviderError::Unauthorized
        );
        let (url, _) = serve(vec![(404, r#"{"error":{"message":"The model `m` does not exist"}}"#)]);
        let p = OpenAiCompatible::new("openai".into(), url, Some("k".into()), "m".into(), false);
        assert_eq!(
            p.complete_json("s", "u").await.unwrap_err(),
            ProviderError::ModelUnavailable("m".into())
        );
    }

    #[tokio::test]
    async fn lists_and_filters_models() {
        let (url, _) = serve(vec![(
            200,
            r#"{"data":[{"id":"text-embedding-3-small"},{"id":"chat-b"},{"id":"whisper-1"},{"id":"chat-a"}]}"#,
        )]);
        let p = OpenAiCompatible::new("openai".into(), url, Some("k".into()), String::new(), false);
        let m = p.list_models().await.unwrap();
        assert_eq!(
            m.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["chat-a", "chat-b"]
        );
        assert_eq!(choose_default_model(&m).as_deref(), Some("chat-a"));
    }

    #[test]
    fn parse_variants() {
        assert!(matches!(
            parse_chat_response(r#"{"choices":[{"message":{"refusal":"no"}}]}"#),
            Err(ProviderError::Refused(_))
        ));
        assert!(matches!(
            parse_chat_response(r#"{"choices":[{"message":{"content":""},"finish_reason":"content_filter"}]}"#),
            Err(ProviderError::Refused(_))
        ));
        assert_eq!(
            parse_chat_response(r#"{"choices":[{"message":{"content":[{"type":"text","text":"x"}]}}]}"#).unwrap(),
            "x"
        );
        assert!(matches!(
            parse_chat_response(r#"{"choices":[]}"#),
            Err(ProviderError::BadResponse(_))
        ));
    }
}
