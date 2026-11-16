//! Typed tool registry.
//!
//! Every tool declares its risk class, a strict input schema, an output
//! schema, a timeout and whether it can be cancelled. The policy engine reads
//! the risk class; the runtime wrapper enforces the timeout and cancellation
//! and records each run in the audit log. Tool handlers still re-check folder
//! grants at execution time.

use super::files;
use super::language;
use super::ledger::Ledger;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Intrinsic side-effect class of a tool (drives approval).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// No side effects outside DigitsNepal.
    Read,
    /// Creates or changes files inside a granted folder.
    WriteLocal,
    /// Reaches the network with model-chosen input.
    Egress,
    /// Runs commands (requires the OS sandbox; none ship yet).
    Exec,
    /// Side effects off this Mac (send, publish, share, pay).
    External,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    /// Qualified name, `namespace.tool`.
    pub name: &'static str,
    pub description: &'static str,
    pub risk: Risk,
    pub input_schema: Value,
    pub output_schema: Value,
    pub timeout: Duration,
    pub cancellable: bool,
}

/// Uses DigitsNepal's existing translation engine (and the provider the user
/// configured) on behalf of the agent.
#[async_trait::async_trait]
pub trait Translator: Send + Sync {
    async fn translate(&self, text: &str, mode: &str) -> Result<Value, String>;
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

fn path_prop() -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": 4096, "description": "Absolute path inside a folder the user granted"})
}

fn error_out(ok: Value) -> Value {
    json!({"oneOf": [ok, {"type": "object", "properties": {"error": {"type": "string"}}, "required": ["error"]}]})
}

/// The canonical tool catalogue.
pub fn specs() -> Vec<ToolSpec> {
    let s = Duration::from_secs;
    vec![
        ToolSpec {
            name: "translation.translate",
            description: "Translate Romanized Nepali, Devanagari or mixed text to English with DigitsNepal's translation engine (faithful, keeps technical terms). mode: accurate, professional, casual, academic, technical or grammar.",
            risk: Risk::Read,
            input_schema: obj(json!({"text": {"type": "string", "minLength": 1, "maxLength": 6000}, "mode": {"type": "string", "enum": ["accurate", "professional", "casual", "academic", "technical", "grammar"]}}), &["text"]),
            output_schema: error_out(json!({"type": "object", "properties": {"accurate_english": {"type": "string"}, "styled_english": {"type": ["string", "null"]}, "warnings": {"type": "array"}}, "required": ["accurate_english"]})),
            timeout: s(90),
            cancellable: true,
        },
        ToolSpec {
            name: "translation.explain",
            description: "Offline: detect the language (Romanized Nepali / Devanagari / English / mixed) and give dictionary meanings of Nepali words. Call first for any Nepali or mixed request.",
            risk: Risk::Read,
            input_schema: obj(json!({"text": {"type": "string", "minLength": 1, "maxLength": 4000}}), &["text"]),
            output_schema: json!({"type": "object", "properties": {"detected_language": {"type": "string"}, "dictionary_glosses": {"type": "array"}}, "required": ["detected_language"]}),
            timeout: s(5),
            cancellable: false,
        },
        ToolSpec {
            name: "files.granted_folders",
            description: "List the folders the user granted (full paths) and the home folder. Call before any file task instead of guessing paths.",
            risk: Risk::Read,
            input_schema: obj(json!({}), &[]),
            output_schema: json!({"type": "object", "properties": {"granted_folders": {"type": "array", "items": {"type": "string"}}, "home": {"type": "string"}}, "required": ["granted_folders"]}),
            timeout: s(5),
            cancellable: false,
        },
        ToolSpec {
            name: "files.list",
            description: "List a folder inside a granted folder (newest first).",
            risk: Risk::Read,
            input_schema: obj(json!({"path": path_prop()}), &["path"]),
            output_schema: error_out(json!({"type": "object", "properties": {"entries": {"type": "array"}}, "required": ["entries"]})),
            timeout: s(15),
            cancellable: true,
        },
        ToolSpec {
            name: "files.search",
            description: "Find files by name (case-insensitive) under a granted folder; optional extension such as \"pdf\".",
            risk: Risk::Read,
            input_schema: obj(json!({"folder": path_prop(), "query": {"type": "string", "maxLength": 200}, "extension": {"type": "string", "maxLength": 16}}), &["folder"]),
            output_schema: error_out(json!({"type": "object", "properties": {"results": {"type": "array"}}, "required": ["results"]})),
            timeout: s(30),
            cancellable: true,
        },
        ToolSpec {
            name: "files.read",
            description: "Read a UTF-8 text file inside a granted folder (up to 256 KB). PDFs: use files.read_pdf.",
            risk: Risk::Read,
            input_schema: obj(json!({"path": path_prop()}), &["path"]),
            output_schema: error_out(json!({"type": "object", "properties": {"text": {"type": "string"}, "truncated": {"type": "boolean"}}, "required": ["text"]})),
            timeout: s(15),
            cancellable: true,
        },
        ToolSpec {
            name: "files.read_pdf",
            description: "Extract the text of a PDF inside a granted folder.",
            risk: Risk::Read,
            input_schema: obj(json!({"path": path_prop()}), &["path"]),
            output_schema: error_out(json!({"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]})),
            timeout: s(45),
            cancellable: true,
        },
        ToolSpec {
            name: "files.create",
            description: "Create or replace a text file inside a granted folder. Always asks the user first; an existing file is backed up.",
            risk: Risk::WriteLocal,
            input_schema: obj(json!({"path": path_prop(), "content": {"type": "string", "maxLength": 524288}}), &["path", "content"]),
            output_schema: error_out(json!({"type": "object", "properties": {"path": {"type": "string"}, "backup": {"type": ["string", "null"]}}, "required": ["path"]})),
            timeout: s(20),
            cancellable: false,
        },
        ToolSpec {
            name: "files.move",
            description: "Move or rename a file inside granted folders. Asks the user first; never overwrites an existing file.",
            risk: Risk::WriteLocal,
            input_schema: obj(json!({"from": path_prop(), "to": path_prop()}), &["from", "to"]),
            output_schema: error_out(json!({"type": "object", "properties": {"from": {"type": "string"}, "to": {"type": "string"}}, "required": ["from", "to"]})),
            timeout: s(20),
            cancellable: false,
        },
    ]
}

/// Legacy names used by conversations created before the typed registry.
/// Kept registered so those sessions can still resume.
pub const LEGACY: &[(&str, &str)] = &[
    ("dn.understand_text", "translation.explain"),
    ("dn.granted_folders", "files.granted_folders"),
    ("dn.list_folder", "files.list"),
    ("dn.search_files", "files.search"),
    ("dn.read_file", "files.read"),
    ("dn.read_pdf", "files.read_pdf"),
    ("dn.write_file", "files.create"),
];

/// Canonical name for a qualified tool name (resolving legacy aliases).
pub fn canonical(name: &str) -> &str {
    LEGACY
        .iter()
        .find(|(old, _)| *old == name)
        .map(|(_, new)| *new)
        .unwrap_or(name)
}

pub fn spec(name: &str) -> Option<ToolSpec> {
    let name = canonical(name);
    specs().into_iter().find(|s| s.name == name)
}

/// Shared state handed to handlers.
#[derive(Clone)]
pub struct Deps {
    pub ledger: Arc<Ledger>,
    pub backups: PathBuf,
    pub translator: Option<Arc<dyn Translator>>,
}

fn arg<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn file_result(r: Result<Value, files::FileError>) -> Value {
    r.unwrap_or_else(|e| json!({"error": e.to_string()}))
}

/// Execute a tool by canonical name. Errors are returned to the model as data.
pub async fn execute(name: &str, args: Value, deps: &Deps) -> Value {
    let roots = deps.ledger.granted_roots();
    match canonical(name) {
        "translation.explain" => language::understand_text(arg(&args, "text")),
        "translation.translate" => match &deps.translator {
            Some(t) => {
                let mode = args.get("mode").and_then(Value::as_str).unwrap_or("accurate");
                t.translate(arg(&args, "text"), mode)
                    .await
                    .unwrap_or_else(|e| json!({"error": e}))
            }
            None => json!({"error": "Translation is not available right now."}),
        },
        "files.granted_folders" => json!({
            "granted_folders": roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "home": std::env::var("HOME").unwrap_or_default(),
            "note": if roots.is_empty() { "No folders granted yet. Ask the user to add one under Sahayogi → Folder access." } else { "" }
        }),
        "files.list" => file_result(files::list_folder(arg(&args, "path"), &roots)),
        "files.search" => {
            let ext = args.get("extension").and_then(Value::as_str).filter(|s| !s.is_empty());
            file_result(files::search_files(
                arg(&args, "folder"),
                arg(&args, "query"),
                ext,
                &roots,
            ))
        }
        "files.read" => file_result(files::read_file(arg(&args, "path"), &roots)),
        "files.read_pdf" => {
            let path = arg(&args, "path").to_string();
            tokio::task::spawn_blocking(move || file_result(files::read_pdf(&path, &roots)))
                .await
                .unwrap_or_else(|e| json!({"error": e.to_string()}))
        }
        "files.create" => file_result(files::write_file(
            arg(&args, "path"),
            arg(&args, "content"),
            &roots,
            &deps.backups,
        )),
        "files.move" => file_result(files::move_file(arg(&args, "from"), arg(&args, "to"), &roots)),
        other => json!({"error": format!("Unknown tool {other}")}),
    }
}

/// Run a tool with its timeout and the engine's cancellation token, and
/// record the run (duration, outcome) in the audit log.
pub async fn run_guarded(
    name: &str,
    args: Value,
    deps: &Deps,
    session: &str,
    cancel: tokio_util::sync::CancellationToken,
) -> Value {
    let Some(spec) = spec(name) else {
        return json!({"error": format!("Unknown tool {name}")});
    };
    let started = std::time::Instant::now();
    let digest_args = args.clone();
    let fut = execute(name, args, deps);
    let out = tokio::select! {
        r = tokio::time::timeout(spec.timeout, fut) => r.unwrap_or_else(|_| json!({"error": format!("{} timed out after {} s.", spec.name, spec.timeout.as_secs())})),
        _ = cancel.cancelled(), if spec.cancellable => json!({"error": "Cancelled."}),
    };
    let ok = out.get("error").is_none();
    deps.ledger
        .record(
            session,
            "agent",
            "tool_run",
            spec.name,
            &digest_args,
            if ok { "ok" } else { "error" },
            &format!("{} ms", started.elapsed().as_millis()),
        )
        .ok();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_strict_schema_risk_timeout() {
        let all = specs();
        let mut names = std::collections::HashSet::new();
        for s in &all {
            assert!(names.insert(s.name), "duplicate {}", s.name);
            assert!(s.name.contains('.'), "{} must be qualified", s.name);
            assert_eq!(s.input_schema["additionalProperties"], json!(false), "{}", s.name);
            assert_eq!(s.input_schema["type"], "object");
            assert!(!s.output_schema.is_null());
            assert!(s.timeout >= Duration::from_secs(1) && s.timeout <= Duration::from_secs(120));
            assert!(!s.description.is_empty());
        }
        // No arbitrary command execution and no delete tool ship.
        assert!(all.iter().all(|s| s.risk != Risk::Exec && s.risk != Risk::External));
        assert!(all
            .iter()
            .all(|s| !s.name.contains("delete") && !s.name.contains("shell")));
    }

    #[test]
    fn legacy_names_resolve_to_registered_tools() {
        for (old, new) in LEGACY {
            assert_eq!(canonical(old), *new);
            assert!(spec(old).is_some(), "{old}");
        }
        assert_eq!(spec("files.create").unwrap().risk, Risk::WriteLocal);
        assert_eq!(spec("dn.write_file").unwrap().risk, Risk::WriteLocal);
    }

    #[tokio::test]
    async fn timeouts_and_cancellation_return_errors_and_are_audited() {
        let ledger = Arc::new(Ledger::in_memory().unwrap());
        struct Slow;
        #[async_trait::async_trait]
        impl Translator for Slow {
            async fn translate(&self, _: &str, _: &str) -> Result<Value, String> {
                tokio::time::sleep(Duration::from_secs(600)).await;
                Ok(json!({}))
            }
        }
        let deps = Deps {
            ledger: ledger.clone(),
            backups: std::env::temp_dir(),
            translator: Some(Arc::new(Slow)),
        };
        let token = tokio_util::sync::CancellationToken::new();
        let t2 = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            t2.cancel();
        });
        let out = run_guarded("translation.translate", json!({"text": "namaste"}), &deps, "s", token).await;
        assert_eq!(out["error"], "Cancelled.");
        let log = ledger.recent(5).unwrap();
        assert_eq!((log[0].kind.as_str(), log[0].decision.as_str()), ("tool_run", "error"));
    }

    #[tokio::test]
    async fn translation_tool_uses_the_injected_translator() {
        struct Echo;
        #[async_trait::async_trait]
        impl Translator for Echo {
            async fn translate(&self, text: &str, mode: &str) -> Result<Value, String> {
                Ok(json!({"accurate_english": format!("[{mode}] {text}")}))
            }
        }
        let deps = Deps {
            ledger: Arc::new(Ledger::in_memory().unwrap()),
            backups: std::env::temp_dir(),
            translator: Some(Arc::new(Echo)),
        };
        let out = execute(
            "translation.translate",
            json!({"text": "Malai thaha chaina", "mode": "professional"}),
            &deps,
        )
        .await;
        assert_eq!(out["accurate_english"], "[professional] Malai thaha chaina");
        let none = Deps {
            translator: None,
            ..deps
        };
        assert!(execute("translation.translate", json!({"text": "x"}), &none).await["error"].is_string());
    }
}
