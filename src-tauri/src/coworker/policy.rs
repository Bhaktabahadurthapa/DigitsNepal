//! The coworker policy engine: the single place that decides whether an
//! agent's tool call runs automatically, waits for the user, or is refused.
//!
//! The agent engine pauses on every tool call; DigitsNepal answers each pause
//! with [`evaluate`]. The model has no tool that can change grants, targets or
//! this policy, so it cannot widen its own permissions.

use super::files::{is_protected, resolve_in_roots};
use super::ledger::{normalize_origin, Target, TargetKind};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// MCP tool annotations reported by a server (hints only, never authorization).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ToolHints {
    pub read_only: bool,
    pub destructive: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Decision {
    /// Runs without asking; recorded in the audit log.
    Allow { reason: String },
    /// Shown to the user as an approval card. `standing_ok` permits
    /// "Allow for this session"; hard-floor actions are always once-only.
    Ask {
        reason: String,
        standing_ok: bool,
        hard_floor: bool,
    },
    /// Refused without running.
    Deny { reason: String },
}

pub struct Context<'a> {
    pub roots: &'a [PathBuf],
    pub targets: &'a [Target],
    /// Annotations of MCP tools, keyed by qualified name (e.g. `rea.inspect_web_page`).
    pub hints: &'a HashMap<String, ToolHints>,
}

const LOCAL_PURE: &[&str] = &["dn.understand_text"];
const PROVIDER_CALL: &[&str] = &["dn.translate_text"];
const FILE_READS: &[&str] = &["dn.list_folder", "dn.search_files", "dn.read_file", "dn.read_pdf"];
const FILE_WRITES: &[&str] = &["dn.write_file"];

/// Words that mark irreversible or outward-facing actions. Such calls always
/// need a fresh human decision, whatever the autonomy settings.
const HARD_FLOOR_WORDS: &[&str] = &[
    "send",
    "delete",
    "remove",
    "uninstall",
    "install",
    "publish",
    "push",
    "purchase",
    "pay",
    "submit",
    "deploy",
    "transfer",
    "post_",
    "email",
    "sudo",
];
/// REA operations that launch, drive or instrument a target. They change
/// external state or execute target code, so they are hard-floor.
const REA_RUNTIME_WORDS: &[&str] = &[
    "capture_",
    "observe_",
    "instrument",
    "start_",
    "launch",
    "frida",
    "gdb",
    "rizin_debug",
    "adb",
    "stop_",
    "load_frida",
    "unload_frida",
    "resume_",
    "trace_native_ui",
    "build_desktop",
    "extract_firmware",
    "pull_",
    "push_",
];

pub fn evaluate(tool: &str, args: &Value, cx: &Context) -> Decision {
    let name = tool.to_ascii_lowercase();
    let short = name.rsplit('.').next().unwrap_or(&name);

    if LOCAL_PURE.contains(&tool) {
        return allow("Works offline on the text you gave it.");
    }
    if PROVIDER_CALL.contains(&tool) {
        return allow("Uses your configured AI provider, which you already approved in Settings.");
    }
    if FILE_READS.contains(&tool) {
        return match file_paths(args) {
            Some(paths) if !paths.is_empty() => {
                for p in &paths {
                    match resolve_in_roots(p, cx.roots, true) {
                        Ok(real) if is_protected(&real) => {
                            return deny("That file usually contains passwords or keys.")
                        }
                        Ok(_) => {}
                        Err(e) => return deny(&e.to_string()),
                    }
                }
                allow("Reads inside a folder you granted.")
            }
            _ => deny("No file or folder path was given."),
        };
    }
    if FILE_WRITES.contains(&tool) {
        return match args.get("path").and_then(Value::as_str) {
            Some(p) => match resolve_in_roots(p, cx.roots, false) {
                Ok(real) if is_protected(&real) => deny("That file usually contains passwords or keys."),
                Ok(_) => Decision::Ask {
                    reason: "Writes a file inside a granted folder. An existing file is backed up first.".into(),
                    standing_ok: true,
                    hard_floor: false,
                },
                Err(e) => deny(&e.to_string()),
            },
            None => deny("No file path was given."),
        };
    }

    let is_rea = name.starts_with("rea.") || name.starts_with("rea_");
    if is_rea {
        if let Err(reason) = targets_authorized(args, cx.targets) {
            return deny(&reason);
        }
        let hints = cx.hints.get(tool).copied().unwrap_or_default();
        if hints.destructive || REA_RUNTIME_WORDS.iter().any(|w| short.contains(w)) {
            return Decision::Ask {
                reason: "This REA step runs, drives or changes the target. It needs your approval every time.".into(),
                standing_ok: false,
                hard_floor: true,
            };
        }
        if hints.read_only {
            return allow("Read-only REA analysis of a target you authorized.");
        }
        return Decision::Ask {
            reason: "REA analysis step on an authorized target.".into(),
            standing_ok: false,
            hard_floor: false,
        };
    }

    if HARD_FLOOR_WORDS.iter().any(|w| short.contains(w)) {
        return Decision::Ask {
            reason: "This action is irreversible or affects others. It always needs your approval.".into(),
            standing_ok: false,
            hard_floor: true,
        };
    }
    Decision::Ask {
        reason: "DigitsNepal does not recognise this tool, so it asks first.".into(),
        standing_ok: false,
        hard_floor: false,
    }
}

fn allow(r: &str) -> Decision {
    Decision::Allow { reason: r.into() }
}
fn deny(r: &str) -> Decision {
    Decision::Deny { reason: r.into() }
}

fn file_paths(args: &Value) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for key in ["path", "folder"] {
        if let Some(s) = args.get(key).and_then(Value::as_str) {
            out.push(s.to_string());
        }
    }
    Some(out)
}

/// Every URL and absolute path in the arguments must belong to a target the
/// user authorized. Arguments without any target reference (for example a
/// follow-up view of retained evidence) pass.
pub fn targets_authorized(args: &Value, targets: &[Target]) -> Result<(), String> {
    let mut strings = Vec::new();
    collect_strings(args, &mut strings, 0);
    for s in strings {
        let s = s.trim();
        if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ws://") || s.starts_with("wss://") {
            let as_http = s.replacen("wss://", "https://", 1).replacen("ws://", "http://", 1);
            let origin = normalize_origin(&as_http).ok_or_else(|| format!("{s} is not a supported web address."))?;
            // Local browser debugging endpoints (CDP) are the user's own browser.
            let loopback = origin.starts_with("http://127.") || origin.starts_with("http://localhost");
            let ok = loopback
                || targets
                    .iter()
                    .any(|t| t.kind == TargetKind::Origin && t.value == origin);
            if !ok {
                return Err(format!(
                    "{origin} is not an authorized target. Add it in REA Lab first."
                ));
            }
        } else if looks_like_local_path(s) {
            let real = std::fs::canonicalize(s).unwrap_or_else(|_| PathBuf::from(s));
            let ok = targets
                .iter()
                .any(|t| t.kind == TargetKind::Path && real.starts_with(&t.value));
            if !ok {
                return Err(format!(
                    "{} is not an authorized target. Add it in REA Lab first.",
                    real.display()
                ));
            }
        }
    }
    Ok(())
}

/// A filesystem path (not a URL route such as `/api/users`): it exists on this
/// Mac or starts at a top-level macOS location.
fn looks_like_local_path(s: &str) -> bool {
    const TOP: &[&str] = &[
        "/Users/",
        "/Applications/",
        "/Volumes/",
        "/private/",
        "/tmp/",
        "/var/",
        "/opt/",
        "/Library/",
        "/System/",
        "/usr/",
        "/etc/",
        "/bin/",
        "/sbin/",
    ];
    s.starts_with('/')
        && s.len() > 1
        && !s.contains('\n')
        && (TOP.iter().any(|t| s.starts_with(t)) || std::path::Path::new(s).exists())
}

fn collect_strings<'a>(v: &'a Value, out: &mut Vec<&'a str>, depth: usize) {
    if depth > 8 {
        return;
    }
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| collect_strings(x, out, depth + 1)),
        Value::Object(o) => o.values().for_each(|x| collect_strings(x, out, depth + 1)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn target(kind: TargetKind, value: &str) -> Target {
        Target {
            id: 1,
            kind,
            value: value.into(),
            attestation: "I own this".into(),
            created_at: 0,
        }
    }

    fn run(
        tool: &str,
        args: Value,
        roots: &[PathBuf],
        targets: &[Target],
        hints: &HashMap<String, ToolHints>,
    ) -> Decision {
        evaluate(tool, &args, &Context { roots, targets, hints })
    }

    #[test]
    fn reads_inside_grant_run_and_outside_are_denied() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().join("g");
        std::fs::create_dir(&g).unwrap();
        std::fs::write(g.join("a.txt"), "x").unwrap();
        std::fs::write(d.path().join("b.txt"), "x").unwrap();
        let roots = vec![g.clone()];
        let h = HashMap::new();
        assert!(matches!(
            run("dn.read_file", json!({"path": g.join("a.txt")}), &roots, &[], &h),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            run("dn.read_file", json!({"path": d.path().join("b.txt")}), &roots, &[], &h),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            run("dn.list_folder", json!({}), &roots, &[], &h),
            Decision::Deny { .. }
        ));
        assert!(
            matches!(
                run("dn.read_file", json!({"path": g.join("a.txt")}), &[], &[], &h),
                Decision::Deny { .. }
            ),
            "no grants"
        );
    }

    #[test]
    fn writes_always_ask_and_never_outside() {
        let d = tempfile::tempdir().unwrap();
        let roots = vec![d.path().to_path_buf()];
        let h = HashMap::new();
        match run(
            "dn.write_file",
            json!({"path": d.path().join("brief.md"), "content": "x"}),
            &roots,
            &[],
            &h,
        ) {
            Decision::Ask {
                standing_ok,
                hard_floor,
                ..
            } => assert!(standing_ok && !hard_floor),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            run(
                "dn.write_file",
                json!({"path": "/etc/hosts", "content": "x"}),
                &roots,
                &[],
                &h
            ),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            run(
                "dn.write_file",
                json!({"path": d.path().join(".env"), "content": "x"}),
                &roots,
                &[],
                &h
            ),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn hard_floor_actions_are_once_only() {
        let h = HashMap::new();
        for tool in [
            "gmail.send_email",
            "github.push_branch",
            "files.delete_file",
            "shop.purchase",
        ] {
            match run(tool, json!({}), &[], &[], &h) {
                Decision::Ask {
                    hard_floor,
                    standing_ok,
                    ..
                } => assert!(hard_floor && !standing_ok, "{tool}"),
                other => panic!("{tool}: {other:?}"),
            }
        }
        assert!(matches!(
            run("other.tool", json!({}), &[], &[], &h),
            Decision::Ask {
                hard_floor: false,
                standing_ok: false,
                ..
            }
        ));
    }

    #[test]
    fn rea_requires_authorized_targets() {
        let mut h = HashMap::new();
        h.insert(
            "rea.inspect_web_page".to_string(),
            ToolHints {
                read_only: true,
                destructive: false,
            },
        );
        let t = [target(TargetKind::Origin, "https://digitsnepal.com")];
        assert!(matches!(
            run(
                "rea.inspect_web_page",
                json!({"url": "https://digitsnepal.com/app"}),
                &[],
                &t,
                &h
            ),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            run(
                "rea.inspect_web_page",
                json!({"url": "https://example.com"}),
                &[],
                &t,
                &h
            ),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            run(
                "rea.inspect_web_page",
                json!({"url": "https://digitsnepal.com"}),
                &[],
                &[],
                &h
            ),
            Decision::Deny { .. }
        ));
        // Nested and websocket references are checked too.
        assert!(matches!(
            run(
                "rea.inspect_web_page",
                json!({"opts": {"targets": ["wss://evil.example/socket"]}}),
                &[],
                &t,
                &h
            ),
            Decision::Deny { .. }
        ));
        // URL routes inside evidence queries are not filesystem paths.
        assert!(matches!(
            run(
                "rea.inspect_web_page",
                json!({"url": "https://digitsnepal.com", "where": {"route": "/api/users"}}),
                &[],
                &t,
                &h
            ),
            Decision::Allow { .. }
        ));
        // Paths must be authorized REA path targets.
        assert!(matches!(
            run(
                "rea.inspect_web_page",
                json!({"path": "/Applications/Other.app"}),
                &[],
                &t,
                &h
            ),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn rea_runtime_and_destructive_tools_are_hard_floor() {
        let mut h = HashMap::new();
        h.insert(
            "rea.export_evidence_bundle".to_string(),
            ToolHints {
                read_only: false,
                destructive: true,
            },
        );
        let t = [target(TargetKind::Origin, "https://digitsnepal.com")];
        for tool in [
            "rea.capture_browser_scenario",
            "rea.start_frida_session",
            "rea.export_evidence_bundle",
        ] {
            match run(tool, json!({"url": "https://digitsnepal.com"}), &[], &t, &h) {
                Decision::Ask {
                    hard_floor: true,
                    standing_ok: false,
                    ..
                } => {}
                other => panic!("{tool}: {other:?}"),
            }
        }
        // Unknown REA tool without hints: ask, never a session-wide grant.
        assert!(matches!(
            run("rea.analyze_something", json!({}), &[], &t, &h),
            Decision::Ask {
                standing_ok: false,
                hard_floor: false,
                ..
            }
        ));
    }

    #[test]
    fn language_tools_run_locally() {
        let h = HashMap::new();
        assert!(matches!(
            run("dn.understand_text", json!({"text": "k cha"}), &[], &[], &h),
            Decision::Allow { .. }
        ));
    }
}
