//! Local compatibility relay for cloud OpenAI-compatible providers.
//!
//! The agent engine validates streamed Chat Completions strictly. Some
//! gateways append extra chunks after the final usage chunk (Token Lab adds a
//! second accounting chunk with a different `id`), which the engine rejects.
//! The relay forwards requests to the real provider and drops only those
//! stray chunks; everything else passes through unchanged.
//!
//! Security: it listens on 127.0.0.1 only, accepts only `POST …/chat/completions`
//! carrying a random per-launch token, and holds the real API key itself — the
//! engine never sees the key.

use futures_util::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const MAX_REQUEST: usize = 8 * 1024 * 1024;

/// Streaming SSE filter. Feed complete lines; returns the lines to forward.
#[derive(Default)]
pub struct SseFilter {
    first_id: Option<String>,
    tail_seen: bool,
}

impl SseFilter {
    pub fn line(&mut self, line: &str) -> Option<String> {
        let Some(data) = line.trim_end().strip_prefix("data:") else {
            // Comments, `id:` and `event:` lines: the engine ignores them.
            return Some(line.to_string());
        };
        let data = data.trim();
        if data == "[DONE]" {
            return Some(line.to_string());
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            return Some(line.to_string());
        };
        if self.tail_seen {
            return None; // anything after the usage tail is gateway accounting
        }
        if let Some(id) = event["id"].as_str() {
            match &self.first_id {
                None => self.first_id = Some(id.to_string()),
                Some(first) if first != id => return None,
                _ => {}
            }
        }
        if event["choices"].as_array().is_some_and(|c| c.is_empty()) && event.get("usage").is_some_and(|u| !u.is_null())
        {
            self.tail_seen = true;
        }
        Some(line.to_string())
    }
}

pub struct Relay {
    pub base_url: String,
    pub token: String,
}

struct Upstream {
    base: String,
    key: Option<String>,
    token: String,
    client: reqwest::Client,
}

fn random_token() -> String {
    let mut bytes = [0u8; 24];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = (uuid::Uuid::new_v4().as_bytes()[i % 16]) ^ (i as u8);
    }
    super::ledger::sha256_hex(&bytes)
}

/// Start a relay on `port` (0 = any free port) that forwards to `upstream_base`.
pub async fn start(port: u16, upstream_base: &str, key: Option<String>) -> std::io::Result<Relay> {
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(_) if port != 0 => TcpListener::bind(("127.0.0.1", 0)).await?,
        Err(e) => return Err(e),
    };
    let addr = listener.local_addr()?;
    let token = random_token();
    let up = Arc::new(Upstream {
        base: upstream_base.trim_end_matches('/').to_string(),
        key,
        token: token.clone(),
        client: reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(std::io::Error::other)?,
    });
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let up = up.clone();
            tokio::spawn(async move {
                let _ = serve(stream, up).await;
            });
        }
    });
    Ok(Relay {
        base_url: format!("http://127.0.0.1:{}/v1", addr.port()),
        token,
    })
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    stream
        .write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes())
        .await
}

async fn serve(mut stream: TcpStream, up: Arc<Upstream>) -> std::io::Result<()> {
    let mut data = Vec::new();
    let mut buf = [0u8; 16384];
    let end = loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(e) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break e + 4;
        }
        if data.len() > 64 * 1024 {
            return respond(&mut stream, "431 Request Header Fields Too Large", "{}").await;
        }
    };
    let head = String::from_utf8_lossy(&data[..end]).to_string();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut len = 0usize;
    let mut auth_ok = false;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            if k == "content-length" {
                len = v.parse().unwrap_or(0);
            } else if k == "authorization" {
                auth_ok = v
                    .strip_prefix("Bearer ")
                    .is_some_and(|t| constant_eq(t.as_bytes(), up.token.as_bytes()));
            }
        }
    }
    let path_ok = request_line.starts_with("POST ")
        && request_line
            .split_whitespace()
            .nth(1)
            .is_some_and(|p| p.ends_with("/chat/completions"));
    if !auth_ok || !path_ok {
        return respond(&mut stream, "403 Forbidden", "{\"error\":{\"message\":\"forbidden\"}}").await;
    }
    if len > MAX_REQUEST {
        return respond(&mut stream, "413 Payload Too Large", "{}").await;
    }
    while data.len() < end + len {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    let body = data[end..(end + len).min(data.len())].to_vec();
    let mut req = up
        .client
        .post(format!("{}/chat/completions", up.base))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .body(body);
    if let Some(k) = &up.key {
        req = req.bearer_auth(k);
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(_) => {
            return respond(
                &mut stream,
                "502 Bad Gateway",
                "{\"error\":{\"message\":\"upstream unreachable\"}}",
            )
            .await
        }
    };
    let status = resp.status();
    let ctype = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    stream
        .write_all(
            format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {ctype}\r\nConnection: close\r\n\r\n",
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            )
            .as_bytes(),
        )
        .await?;
    let streaming = status.is_success() && ctype.contains("event-stream");
    let mut filter = SseFilter::default();
    let mut pending = Vec::new();
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        let Ok(chunk) = chunk else { break };
        if !streaming {
            stream.write_all(&chunk).await?;
            continue;
        }
        pending.extend_from_slice(&chunk);
        while let Some(i) = pending.iter().position(|&c| c == b'\n') {
            let line: Vec<u8> = pending.drain(..=i).collect();
            let text = String::from_utf8_lossy(&line);
            if let Some(out) = filter.line(&text) {
                stream.write_all(out.as_bytes()).await?;
            }
        }
    }
    if !pending.is_empty() {
        let text = String::from_utf8_lossy(&pending).to_string();
        if let Some(out) = filter.line(&text) {
            stream.write_all(out.as_bytes()).await?;
        }
    }
    stream.shutdown().await
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str]) -> Vec<String> {
        let mut f = SseFilter::default();
        lines.iter().filter_map(|l| f.line(l)).collect()
    }

    #[test]
    fn drops_token_lab_accounting_chunk_after_usage_tail() {
        let out = run(&[
            r#"data: {"id":"a","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}"#,
            "id: 0",
            r#"data: {"id":"a","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            r#"data: {"id":"a","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1}}"#,
            r#"data: {"id":"chatcmpl-9","choices":[{"index":0,"delta":{},"finish_reason":null}],"usage":{"prompt_tokens":1,"completion_tokens":1,"cost":0.1}}"#,
            "event: done",
            "data: [DONE]",
        ]);
        assert!(out.iter().all(|l| !l.contains("chatcmpl-9")), "{out:?}");
        assert!(out.iter().any(|l| l.contains("[DONE]")));
        assert_eq!(out.iter().filter(|l| l.starts_with("data: {")).count(), 3);
    }

    #[test]
    fn drops_chunks_with_a_different_id_and_keeps_normal_streams() {
        let normal = [
            r#"data: {"id":"x","choices":[{"index":0,"delta":{"content":"a"},"finish_reason":null}]}"#,
            r#"data: {"id":"x","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            "data: [DONE]",
        ];
        assert_eq!(run(&normal).len(), 3);
        let out = run(&[
            normal[0],
            r#"data: {"id":"y","choices":[{"index":0,"delta":{"content":"dup"}}]}"#,
            normal[1],
        ]);
        assert_eq!(out.len(), 2);
    }

    #[tokio::test]
    async fn rejects_requests_without_the_launch_token_or_wrong_path() {
        let r = start(0, "https://example.invalid/v1", Some("real-key".into()))
            .await
            .unwrap();
        let port = r
            .base_url
            .split(':')
            .nth(2)
            .unwrap()
            .split('/')
            .next()
            .unwrap()
            .to_string();
        for (path, auth) in [
            ("/v1/chat/completions", "Bearer wrong"),
            ("/v1/models", &*format!("Bearer {}", r.token)),
        ] {
            let mut s = TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap()))
                .await
                .unwrap();
            s.write_all(
                format!("POST {path} HTTP/1.1\r\nAuthorization: {auth}\r\nContent-Length: 2\r\n\r\n{{}}").as_bytes(),
            )
            .await
            .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).await.unwrap();
            assert!(out.starts_with("HTTP/1.1 403"), "{path}: {out}");
        }
    }
}
