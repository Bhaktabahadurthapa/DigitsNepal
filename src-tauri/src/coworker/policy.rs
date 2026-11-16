//! The coworker policy engine: the single place that decides whether an
//! agent's tool call runs automatically, waits for the user, or is refused.
//!
//! The agent engine pauses on every tool call; DigitsNepal answers each pause
//! with [`evaluate`]. The model has no tool that can change grants, targets or
//! this policy, so it cannot widen its own permissions.

use super::files::{is_protected, resolve_in_roots};
use super::tools::Risk;
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
    /// Annotations of MCP tools, keyed by qualified name (e.g. `github.create_issue`).
    pub hints: &'a HashMap<String, ToolHints>,
}

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

pub fn evaluate(tool: &str, args: &Value, cx: &Context) -> Decision {
    let name = tool.to_ascii_lowercase();
    let short = name.rsplit('.').next().unwrap_or(&name);

    if let Some(spec) = super::tools::spec(tool) {
        let paths = file_paths(args);
        return match spec.risk {
            Risk::Read => {
                for p in &paths {
                    match resolve_in_roots(p, cx.roots, true) {
                        Ok(real) if is_protected(&real) => {
                            return deny("That file usually contains passwords or keys.")
                        }
                        Ok(_) => {}
                        Err(e) => return deny(&e.to_string()),
                    }
                }
                if paths.is_empty()
                    && spec.input_schema["required"]
                        .as_array()
                        .is_some_and(|r| r.iter().any(|k| matches!(k.as_str(), Some("path" | "folder"))))
                {
                    return deny("No file or folder path was given.");
                }
                allow(if paths.is_empty() {
                    "Read-only; works on the text you gave it."
                } else {
                    "Reads inside a folder you granted."
                })
            }
            Risk::WriteLocal => {
                if paths.is_empty() {
                    return deny("No file path was given.");
                }
                for p in &paths {
                    match resolve_in_roots(p, cx.roots, false) {
                        Ok(real) if is_protected(&real) => {
                            return deny("That file usually contains passwords or keys.")
                        }
                        Ok(_) => {}
                        Err(e) => return deny(&e.to_string()),
                    }
                }
                Decision::Ask {
                    reason: "Changes files inside a granted folder. An existing file is backed up first; nothing is deleted.".into(),
                    standing_ok: true,
                    hard_floor: false,
                }
            }
            Risk::Egress => Decision::Ask {
                reason: "Reaches the internet with text chosen by the assistant.".into(),
                standing_ok: true,
                hard_floor: false,
            },
            // Command tools require the OS sandbox, which does not ship yet.
            Risk::Exec => deny("Running commands needs DigitsNepal's sandbox, which is not available yet."),
            Risk::External => Decision::Ask {
                reason: "This affects people or systems outside your Mac. It always needs your approval.".into(),
                standing_ok: false,
                hard_floor: true,
            },
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

/// Every path-like argument a file tool can take.
fn file_paths(args: &Value) -> Vec<String> {
    ["path", "folder", "from", "to"]
        .iter()
        .filter_map(|k| args.get(*k).and_then(Value::as_str).map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(tool: &str, args: Value, roots: &[PathBuf], hints: &HashMap<String, ToolHints>) -> Decision {
        evaluate(tool, &args, &Context { roots, hints })
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
            run("dn.read_file", json!({"path": g.join("a.txt")}), &roots, &h),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            run("dn.read_file", json!({"path": d.path().join("b.txt")}), &roots, &h),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            run("dn.list_folder", json!({}), &roots, &h),
            Decision::Deny { .. }
        ));
        assert!(
            matches!(
                run("dn.read_file", json!({"path": g.join("a.txt")}), &[], &h),
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
                &h
            ),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            run(
                "dn.write_file",
                json!({"path": d.path().join(".env"), "content": "x"}),
                &roots,
                &h
            ),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn typed_tools_follow_their_risk_class() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().join("g");
        std::fs::create_dir(&g).unwrap();
        std::fs::write(g.join("a.txt"), "x").unwrap();
        let roots = vec![g.clone()];
        let h = HashMap::new();
        assert!(matches!(
            run("files.read", json!({"path": g.join("a.txt")}), &roots, &h),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            run("translation.translate", json!({"text": "namaste"}), &roots, &h),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            run("files.list", json!({}), &roots, &h),
            Decision::Deny { .. }
        ));
        // Move: both ends must be inside a grant.
        assert!(matches!(
            run(
                "files.move",
                json!({"from": g.join("a.txt"), "to": g.join("b.txt")}),
                &roots,
                &h
            ),
            Decision::Ask {
                standing_ok: true,
                hard_floor: false,
                ..
            }
        ));
        assert!(matches!(
            run(
                "files.move",
                json!({"from": g.join("a.txt"), "to": d.path().join("out.txt")}),
                &roots,
                &h
            ),
            Decision::Deny { .. }
        ));
        // Legacy names keep their old behaviour.
        assert!(matches!(
            run("dn.read_file", json!({"path": g.join("a.txt")}), &roots, &h),
            Decision::Allow { .. }
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
            match run(tool, json!({}), &[], &h) {
                Decision::Ask {
                    hard_floor,
                    standing_ok,
                    ..
                } => assert!(hard_floor && !standing_ok, "{tool}"),
                other => panic!("{tool}: {other:?}"),
            }
        }
        assert!(matches!(
            run("other.tool", json!({}), &[], &h),
            Decision::Ask {
                hard_floor: false,
                standing_ok: false,
                ..
            }
        ));
    }

    #[test]
    fn language_tools_run_locally() {
        let h = HashMap::new();
        assert!(matches!(
            run("dn.understand_text", json!({"text": "k cha"}), &[], &h),
            Decision::Allow { .. }
        ));
    }
}
