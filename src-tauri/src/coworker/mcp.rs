//! Minimal client for remote MCP servers (Streamable HTTP transport).
//!
//! Only servers the user adds by URL are contacted. Local MCP programs (stdio)
//! are not supported: launching them would run arbitrary code outside a
//! sandbox. Discovered tools stay disabled until the user enables each one,
//! and every call still goes through the policy engine.

use super::ledger::McpToolInfo;
use serde_json::{json, Value};
use std::time::Duration;

const PROTOCOL: &str = "2025-06-18";

/// Engine namespace for a server: `mcp_<name>`.
pub fn namespace(server: &str) -> String {
    format!("mcp_{server}")
}

/// Server and tool names must be short identifiers.
pub fn valid_name(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

pub fn check_url(url: &str) -> Result<(), String> {
    let u = reqwest::Url::parse(url).map_err(|_| "That is not a valid address.".to_string())?;
    let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    match u.scheme() {
        "https" => Ok(()),
        "http" if local => Ok(()),
        _ => Err("Use an https:// address (http is allowed only for servers on this Mac).".into()),
    }
}

struct Conn {
    client: reqwest::Client,
    url: String,
    session: Option<String>,
    next_id: u64,
}

fn parse_body(content_type: &str, body: &str, id: u64) -> Option<Value> {
    if content_type.contains("text/event-stream") {
        let mut data = String::new();
        for line in body.lines().chain(std::iter::once("")) {
            if let Some(d) = line.strip_prefix("data:") {
                data.push_str(d.trim_start());
            } else if line.is_empty() && !data.is_empty() {
                if let Ok(v) = serde_json::from_str::<Value>(&data) {
                    if v["id"] == json!(id) {
                        return Some(v);
                    }
                }
                data.clear();
            }
        }
        None
    } else {
        serde_json::from_str(body).ok()
    }
}

impl Conn {
    async fn open(url: &str) -> Result<Self, String> {
        check_url(url)?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;
        let mut c = Conn {
            client,
            url: url.to_string(),
            session: None,
            next_id: 1,
        };
        let init = c
            .request(
                "initialize",
                json!({"protocolVersion": PROTOCOL, "capabilities": {}, "clientInfo": {"name": "DigitsNepal", "version": env!("CARGO_PKG_VERSION")}}),
            )
            .await?;
        if init.get("protocolVersion").is_none() {
            return Err("The server did not complete the MCP handshake.".into());
        }
        c.notify("notifications/initialized").await;
        Ok(c)
    }

    fn post(&self, body: &Value) -> reqwest::RequestBuilder {
        let mut r = self
            .client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", PROTOCOL)
            .json(body);
        if let Some(s) = &self.session {
            r = r.header("Mcp-Session-Id", s);
        }
        r
    }

    async fn notify(&self, method: &str) {
        let _ = self.post(&json!({"jsonrpc": "2.0", "method": method})).send().await;
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let resp = self
            .post(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .send()
            .await
            .map_err(|e| format!("Could not reach the MCP server: {}", e.without_url()))?;
        if let Some(s) = resp.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
            self.session = Some(s.to_string());
        }
        let status = resp.status();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("The MCP server answered with HTTP {}.", status.as_u16()));
        }
        let msg = parse_body(&ct, &body, id).ok_or("The MCP server sent an unreadable reply.")?;
        if let Some(e) = msg.get("error") {
            return Err(format!("MCP error: {}", e["message"].as_str().unwrap_or("unknown")));
        }
        Ok(msg["result"].clone())
    }
}

/// Connect and list the server's tools.
pub async fn discover(url: &str) -> Result<Vec<McpToolInfo>, String> {
    let mut c = Conn::open(url).await?;
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let params = cursor.as_ref().map_or(json!({}), |c| json!({"cursor": c}));
        let r = c.request("tools/list", params).await?;
        for t in r["tools"].as_array().into_iter().flatten() {
            let Some(name) = t["name"].as_str() else { continue };
            let mut schema = t["inputSchema"].clone();
            if schema["type"] != "object" {
                schema = json!({"type": "object", "properties": {}});
            }
            out.push(McpToolInfo {
                name: name.to_string(),
                description: t["description"].as_str().unwrap_or("").chars().take(1000).collect(),
                input_schema: schema,
                read_only: t["annotations"]["readOnlyHint"].as_bool().unwrap_or(false),
                destructive: t["annotations"]["destructiveHint"].as_bool().unwrap_or(true),
            });
        }
        cursor = r["nextCursor"].as_str().map(str::to_string);
        if cursor.is_none() || out.len() >= 200 {
            break;
        }
    }
    Ok(out)
}

/// Call one tool; the result is returned to the model as untrusted data.
pub async fn call(url: &str, tool: &str, args: Value) -> Value {
    let r = async {
        let mut c = Conn::open(url).await?;
        c.request("tools/call", json!({"name": tool, "arguments": args})).await
    }
    .await;
    match r {
        Ok(v) => {
            let text: Vec<String> = v["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["text"].as_str().map(|t| t.chars().take(60_000).collect()))
                .collect();
            let mut out = json!({"content": text.join("\n"), "untrusted_content": true});
            if v["isError"] == true {
                out["error"] = json!("The connected app reported an error.");
            }
            if !v["structuredContent"].is_null() {
                out["structured"] = v["structuredContent"].clone();
            }
            out
        }
        Err(e) => json!({"error": e}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_urls_are_checked() {
        assert!(valid_name("notion", 20));
        for bad in ["", "Notion", "1abc", "a-b", "a.b", "aaaaaaaaaaaaaaaaaaaaa"] {
            assert!(!valid_name(bad, 20), "{bad}");
        }
        assert!(check_url("https://mcp.example.com/mcp").is_ok());
        assert!(check_url("http://127.0.0.1:3333/mcp").is_ok());
        assert!(check_url("http://mcp.example.com/mcp").is_err());
        assert!(check_url("file:///x").is_err());
    }

    #[test]
    fn sse_replies_are_matched_by_id() {
        let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"ok\":true}}\n\n";
        assert_eq!(parse_body("text/event-stream", body, 7).unwrap()["result"]["ok"], true);
        assert!(parse_body("text/event-stream", body, 8).is_none());
        assert_eq!(
            parse_body("application/json", "{\"id\":1,\"result\":{}}", 1).unwrap()["id"],
            1
        );
    }
}
