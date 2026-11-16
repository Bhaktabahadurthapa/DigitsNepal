//! REA (github.com/morluto/rea, npm `rea-agents`) adapter.
//!
//! DigitsNepal does not reimplement any REA analysis. It:
//! * finds a supported Node.js and an installed `rea-agents` package;
//! * launches the official stdio MCP server (`rea mcp`) scoped to the tool
//!   families an investigation needs (`REA_MCP_TOOL_FAMILIES`);
//! * reads the server's real tool catalog and annotations over MCP
//!   (`initialize` + `tools/list`) — no tool names are hard-coded.
//!
//! REA performs no authorization of its own; DigitsNepal's policy engine
//! decides every REA call (see `policy.rs`).

use super::policy::ToolHints;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Official REA tool families (src/contracts/toolFamilyIds.ts).
pub const FAMILIES: &[&str] = &[
    "direct",
    "enhanced",
    "native",
    "artifact",
    "managed",
    "firmware",
    "android",
    "pcap",
    "flutter",
    "frida",
    "apktool",
    "jeb",
    "adb",
    "browser",
    "electron",
    "javascript-runtime",
    "application",
    "session",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Investigation {
    Website,
    JavascriptApp,
    DesktopApp,
}

impl Investigation {
    /// Minimal family scope per investigation type (keeps tool schemas out of
    /// the model's context and the attack surface small).
    pub fn families(self) -> &'static [&'static str] {
        match self {
            Investigation::Website => &["browser", "application", "session"],
            Investigation::JavascriptApp => &["application", "electron", "javascript-runtime", "session"],
            Investigation::DesktopApp => &["artifact", "application", "session"],
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Investigation::Website => "web",
            Investigation::JavascriptApp => "js",
            Investigation::DesktopApp => "desktop",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Readiness {
    pub ready: bool,
    pub node: Option<String>,
    pub node_version: Option<String>,
    pub rea_script: Option<String>,
    pub rea_version: Option<String>,
    /// Plain-language problems and fixes.
    pub problems: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Installation {
    pub node: PathBuf,
    pub script: PathBuf,
    pub version: String,
}

/// First REA release that honours `REA_MCP_TOOL_FAMILIES`. Older servers
/// advertise every tool (207 in 6.7.0), so DigitsNepal refuses to use them.
pub const MIN_SCOPED_VERSION: (u64, u64, u64) = (6, 8, 0);

pub fn supports_scoping(version: &str) -> bool {
    parse_version(version).is_some_and(|v| v >= MIN_SCOPED_VERSION)
}

fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut it = v.split(['.', '-', '+']).map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next().flatten().unwrap_or(0)))
}

/// REA's engines: `^22.23.4 || ^24.22.1 || >=26.12.1`.
pub fn node_supported(version: &str) -> bool {
    match parse_version(version) {
        Some((22, m, p)) => (m, p) >= (23, 4),
        Some((24, m, p)) => (m, p) >= (22, 1),
        Some((maj, m, p)) if maj >= 26 => (maj, m, p) >= (26, 12, 1),
        _ => false,
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Node candidates: apps launched from Finder have a minimal PATH, so look in
/// the usual install locations as well.
fn node_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&path).map(|d| d.join("node")));
    }
    let nvm = home().join(".nvm/versions/node");
    if let Ok(rd) = std::fs::read_dir(&nvm) {
        let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("bin/node")).collect();
        v.sort();
        v.reverse();
        out.extend(v);
    }
    out.extend(
        ["/opt/homebrew/bin/node", "/usr/local/bin/node"]
            .iter()
            .map(PathBuf::from)
            .chain([home().join(".volta/bin/node")]),
    );
    let mut seen = std::collections::HashSet::new();
    out.into_iter()
        .filter(|p| p.is_file() && seen.insert(p.clone()))
        .collect()
}

fn node_version(node: &Path) -> Option<String> {
    let out = Command::new(node).arg("--version").stdin(Stdio::null()).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn package_version(dir: &Path) -> Option<String> {
    let pkg: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).ok()?).ok()?;
    (pkg["name"] == "rea-agents")
        .then(|| pkg["version"].as_str().map(str::to_owned))
        .flatten()
}

/// Installed `rea-agents` packages: global installs beside each Node, then the
/// npx cache. Highest version first.
fn rea_candidates(nodes: &[PathBuf]) -> Vec<(PathBuf, String)> {
    let mut dirs: Vec<PathBuf> = nodes
        .iter()
        .filter_map(|n| {
            n.parent()?
                .parent()
                .map(|prefix| prefix.join("lib/node_modules/rea-agents"))
        })
        .collect();
    if let Ok(rd) = std::fs::read_dir(home().join(".npm/_npx")) {
        dirs.extend(rd.flatten().map(|e| e.path().join("node_modules/rea-agents")));
    }
    let mut found: Vec<(PathBuf, String)> = dirs
        .into_iter()
        .filter_map(|d| {
            let v = package_version(&d)?;
            let script = d.join("scripts/rea.mjs");
            script.is_file().then_some((script, v))
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(parse_version(&f.1)));
    found.dedup_by(|a, b| a.0 == b.0);
    found
}

pub fn readiness() -> (Readiness, Option<Installation>) {
    let nodes = node_candidates();
    let supported = nodes.iter().find_map(|n| {
        let v = node_version(n)?;
        node_supported(&v).then(|| (n.clone(), v))
    });
    let rea = rea_candidates(&nodes).into_iter().next();
    let mut problems = Vec::new();
    if supported.is_none() {
        problems.push(
            "REA needs Node.js 22.23.4+, 24.22.1+ or 26.12.1+. Install a current Node.js from nodejs.org, then reopen DigitsNepal."
                .into(),
        );
    }
    match &rea {
        None => problems.push("REA is not installed. In Terminal run: npm install -g rea-agents".into()),
        Some((_, v)) if !supports_scoping(v) => problems.push(format!(
            "REA {v} cannot limit its tools to one investigation type. Update it in Terminal: npm install -g rea-agents@latest"
        )),
        _ => {}
    }
    let install = match (&supported, &rea) {
        (Some((n, _)), Some((s, v))) if supports_scoping(v) => Some(Installation {
            node: n.clone(),
            script: s.clone(),
            version: v.clone(),
        }),
        _ => None,
    };
    (
        Readiness {
            ready: install.is_some(),
            node: supported.as_ref().map(|(n, _)| n.display().to_string()),
            node_version: supported.map(|(_, v)| v),
            rea_script: rea.as_ref().map(|(s, _)| s.display().to_string()),
            rea_version: rea.map(|(_, v)| v),
            problems,
        },
        install,
    )
}

/// The MCP launch line. `/usr/bin/env` sets the family scope; Node runs the
/// package's own entry script, so no PATH lookup is involved.
pub fn launch_args(inst: &Installation, families: &[&str]) -> (String, Vec<String>) {
    (
        "/usr/bin/env".into(),
        vec![
            format!("REA_MCP_TOOL_FAMILIES={}", families.join(",")),
            inst.node.display().to_string(),
            inst.script.display().to_string(),
            "mcp".into(),
        ],
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogTool {
    pub name: String,
    pub description: String,
    pub hints: ToolHints,
}

/// Read the live tool catalog from a scoped REA MCP server.
pub fn discover_tools(inst: &Installation, families: &[&str], timeout: Duration) -> Result<Vec<CatalogTool>, String> {
    let (program, args) = launch_args(inst, families);
    let mut child = Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not start REA: {e}"))?;
    let result = (|| {
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let deadline = Instant::now() + timeout;
        let mut send = |v: Value| -> Result<(), String> {
            writeln!(stdin, "{v}").map_err(|e| e.to_string())?;
            stdin.flush().map_err(|e| e.to_string())
        };
        let wait_for = |id: i64| -> Result<Value, String> {
            loop {
                let left = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("REA did not answer in time.")?;
                let line = rx
                    .recv_timeout(left)
                    .map_err(|_| "REA did not answer in time.".to_string())?;
                let Ok(v) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if v["id"] == json!(id) {
                    if let Some(err) = v.get("error") {
                        return Err(format!("REA error: {}", err["message"].as_str().unwrap_or("unknown")));
                    }
                    return Ok(v["result"].clone());
                }
            }
        };
        send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18","capabilities":{},
            "clientInfo":{"name":"DigitsNepal","version":env!("CARGO_PKG_VERSION")}}}))?;
        wait_for(1)?;
        send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))?;
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for page in 0..20 {
            let id = 2 + page;
            let mut params = json!({});
            if let Some(c) = &cursor {
                params["cursor"] = json!(c);
            }
            send(json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params}))?;
            let r = wait_for(id)?;
            for t in r["tools"].as_array().into_iter().flatten() {
                let Some(name) = t["name"].as_str() else { continue };
                tools.push(CatalogTool {
                    name: name.to_string(),
                    description: t["description"].as_str().unwrap_or("").chars().take(300).collect(),
                    hints: ToolHints {
                        read_only: t["annotations"]["readOnlyHint"] == json!(true),
                        destructive: t["annotations"]["destructiveHint"] == json!(true),
                    },
                });
            }
            match r["nextCursor"].as_str() {
                Some(c) if !c.is_empty() => cursor = Some(c.to_string()),
                _ => break,
            }
        }
        Ok(tools)
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// Map REA catalog entries to qualified names in a namespace (e.g. `rea_web`).
pub fn hints_for(namespace: &str, tools: &[CatalogTool]) -> HashMap<String, ToolHints> {
    tools
        .iter()
        .map(|t| (format!("{namespace}.{}", t.name), t.hints))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_engine_ranges_match_rea() {
        for ok in [
            "v22.23.4", "22.30.0", "v24.22.1", "v24.30.0", "v26.12.1", "v26.13.0", "v27.0.0",
        ] {
            assert!(node_supported(ok), "{ok}");
        }
        for bad in [
            "v22.20.0", "v22.23.3", "v24.10.0", "v23.5.0", "v25.9.9", "v26.12.0", "v20.11.1", "garbage",
        ] {
            assert!(!node_supported(bad), "{bad}");
        }
    }

    #[test]
    fn families_are_official_and_scoped() {
        for inv in [
            Investigation::Website,
            Investigation::JavascriptApp,
            Investigation::DesktopApp,
        ] {
            assert!(inv.families().contains(&"session"));
            assert!(inv.families().iter().all(|f| FAMILIES.contains(f)));
            assert!(!inv.families().contains(&"native"), "native engines are opt-in");
        }
    }

    #[test]
    fn launch_uses_absolute_programs_only() {
        let inst = Installation {
            node: "/opt/node/bin/node".into(),
            script: "/x/rea-agents/scripts/rea.mjs".into(),
            version: "6.9.0".into(),
        };
        let (prog, args) = launch_args(&inst, Investigation::Website.families());
        assert_eq!(prog, "/usr/bin/env");
        assert_eq!(args[0], "REA_MCP_TOOL_FAMILIES=browser,application,session");
        assert_eq!(
            &args[1..],
            ["/opt/node/bin/node", "/x/rea-agents/scripts/rea.mjs", "mcp"]
        );
    }

    #[test]
    fn old_rea_without_scoping_is_rejected() {
        assert!(!supports_scoping("6.7.0"));
        assert!(supports_scoping("6.8.0"));
        assert!(supports_scoping("6.9.0"));
        assert!(supports_scoping("7.0.0-beta.1"));
    }

    /// Live contract test against the installed REA (skipped when REA or a
    /// supported Node is missing). Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_catalog_is_scoped_and_annotated() {
        let (r, inst) = readiness();
        let Some(inst) = inst else {
            eprintln!("REA not ready: {:?}", r.problems);
            return;
        };
        let web = discover_tools(&inst, Investigation::Website.families(), Duration::from_secs(120)).unwrap();
        assert!(!web.is_empty());
        let names: Vec<&str> = web.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"binary_session"), "session family present: {names:?}");
        assert!(
            !names.iter().any(|n| n.starts_with("start_frida")),
            "frida family excluded"
        );
        eprintln!("REA {} web scope: {} tools", inst.version, web.len());
    }
}
