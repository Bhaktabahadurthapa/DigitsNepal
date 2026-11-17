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

/// Saved automation steps by name, as (tool, args) pairs.
pub type AutomationLookup = dyn Fn(&str) -> Option<Vec<(String, Value)>> + Sync;

pub struct Context<'a> {
    pub roots: &'a [PathBuf],
    /// Annotations of MCP tools, keyed by qualified name (e.g. `github.create_issue`).
    pub hints: &'a HashMap<String, ToolHints>,
    /// Looks up a saved automation's steps by name.
    pub automation: &'a AutomationLookup,
    /// The user's global switches from the Security Center.
    pub security: &'a super::ledger::Security,
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

    if cx.security.paused {
        return deny("Sahayogi is paused. Resume it in the Security Center.");
    }
    let networked = cx.hints.contains_key(tool)
        || super::tools::spec(tool).is_some_and(|s| s.risk == Risk::Egress)
        || name.starts_with("apps.");
    if cx.security.network_off && networked {
        return deny("Internet tools are turned off in the Security Center.");
    }

    match super::tools::canonical(tool) {
        "automation.create" => {
            return match super::tools::validate_steps(&args["steps"]) {
                Err(e) => deny(&e),
                Ok(()) => Decision::Ask {
                    reason: format!(
                        "Saves a reusable automation:\n{}\nIt does not run now; each run is checked and approved separately.",
                        describe_steps(&args["steps"])
                    ),
                    standing_ok: false,
                    hard_floor: false,
                },
            };
        }
        "automation.run" => {
            let name = args["name"].as_str().unwrap_or("");
            let Some(steps) = (cx.automation)(name) else {
                return deny(&format!("There is no saved automation named {name}."));
            };
            let mut asks = false;
            for (i, (t, a)) in steps.iter().enumerate() {
                if t.starts_with("automation.") {
                    return deny("Automations cannot run other automations.");
                }
                match evaluate(t, a, cx) {
                    Decision::Deny { reason } => {
                        return deny(&format!("Step {} ({t}) would be refused: {reason}", i + 1))
                    }
                    Decision::Ask { hard_floor: true, .. } => {
                        return deny(&format!(
                            "Step {} ({t}) always needs a fresh approval, so it cannot be automated.",
                            i + 1
                        ))
                    }
                    Decision::Ask { .. } => asks = true,
                    Decision::Allow { .. } => {}
                }
            }
            let list = Value::Array(
                steps
                    .iter()
                    .map(|(t, a)| serde_json::json!({"tool": t, "args": a}))
                    .collect(),
            );
            return if asks {
                Decision::Ask {
                    reason: format!(
                        "Runs the automation \"{name}\" now — approving allows all of these steps once:\n{}",
                        describe_steps(&list)
                    ),
                    standing_ok: false,
                    hard_floor: false,
                }
            } else {
                allow("Every step only reads inside folders you granted.")
            };
        }
        _ => {}
    }

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
            Risk::Egress if cx.security.auto_web && super::tools::READ_ONLY_WEB.contains(&spec.name) => {
                allow("Read-only web research; you turned on \"Research the web without asking\".")
            }
            Risk::Egress => Decision::Ask {
                reason: "Reaches the internet with text chosen by the assistant.".into(),
                standing_ok: true,
                hard_floor: false,
            },
            // Code runs only in the macOS sandbox, only when the user turned
            // it on, and every run is approved.
            Risk::Exec => {
                if !super::sandbox::available() {
                    return deny("The macOS sandbox is not available on this Mac, so code cannot run.");
                }
                if !cx.security.code_enabled {
                    return deny(
                        "Running code is off. The user can turn on \"Run code in the sandbox\" in the Security Center.",
                    );
                }
                for p in &paths {
                    match resolve_in_roots(p, cx.roots, true) {
                        Ok(real) if is_protected(&real) => {
                            return deny("That file usually contains passwords or keys.")
                        }
                        Ok(_) => {}
                        Err(e) => return deny(&e.to_string()),
                    }
                }
                Decision::Ask {
                    reason: format!(
                        "Runs {} code in DigitsNepal's sandbox: no internet, no access to your files or apps{}. Read the code before allowing it.",
                        args["language"].as_str().unwrap_or(""),
                        if paths.is_empty() { String::new() } else { format!("; {} input file(s) are copied in read-only", paths.len()) }
                    ),
                    standing_ok: false,
                    hard_floor: false,
                }
            }
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
    // Tools from connected MCP apps: the server's annotations are hints only,
    // so they can make a call ask *more*, never run without asking.
    if let Some(h) = cx.hints.get(tool) {
        let app = super::mcp::app_of(tool).unwrap_or("connected app");
        return if h.destructive {
            Decision::Ask {
                reason: format!(
                    "The connected app \"{app}\" says this can change or delete data. It always needs your approval."
                ),
                standing_ok: false,
                hard_floor: true,
            }
        } else if h.read_only {
            Decision::Ask {
                reason: format!("Reads data through the connected app \"{app}\"; your request is sent to it."),
                standing_ok: true,
                hard_floor: false,
            }
        } else {
            Decision::Ask {
                reason: format!("Uses the connected app \"{app}\", which may change data there."),
                standing_ok: false,
                hard_floor: false,
            }
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

/// Every path-like argument a file tool can take (including research sources).
fn file_paths(args: &Value) -> Vec<String> {
    let mut out: Vec<String> = ["path", "folder", "from", "to"]
        .iter()
        .filter_map(|k| args.get(*k).and_then(Value::as_str).map(str::to_string))
        .collect();
    for s in args.get("sources").and_then(Value::as_array).into_iter().flatten() {
        if let Some(p) = s.get("path").and_then(Value::as_str) {
            out.push(p.to_string());
        }
    }
    for p in args.get("input_files").and_then(Value::as_array).into_iter().flatten() {
        if let Some(p) = p.as_str() {
            out.push(p.to_string());
        }
    }
    out
}

fn describe_steps(steps: &Value) -> String {
    steps
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, s)| {
            let a = &s["args"];
            let target = ["path", "url", "query", "repo", "from", "folder"]
                .iter()
                .find_map(|k| a.get(*k).and_then(Value::as_str))
                .unwrap_or("");
            format!("{}. {} {}", i + 1, s["tool"].as_str().unwrap_or("?"), target)
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(tool: &str, args: Value, roots: &[PathBuf], hints: &HashMap<String, ToolHints>) -> Decision {
        evaluate(
            tool,
            &args,
            &Context {
                roots,
                hints,
                automation: &|_| None,
                security: &crate::coworker::ledger::Security::default(),
            },
        )
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

    #[test]
    fn automation_runs_are_judged_by_their_steps() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().canonicalize().unwrap();
        std::fs::write(g.join("a.txt"), "x").unwrap();
        let roots = vec![g.clone()];
        let h = HashMap::new();
        let read = vec![("files.read".to_string(), json!({"path": g.join("a.txt")}))];
        let write = vec![
            ("files.read".to_string(), json!({"path": g.join("a.txt")})),
            (
                "files.create".to_string(),
                json!({"path": g.join("b.md"), "content": "y"}),
            ),
        ];
        let outside = vec![("files.read".to_string(), json!({"path": "/etc/hosts"}))];
        let lookup = move |n: &str| match n {
            "read" => Some(read.clone()),
            "write" => Some(write.clone()),
            "outside" => Some(outside.clone()),
            _ => None,
        };
        let sec = crate::coworker::ledger::Security::default();
        let cx = Context {
            roots: &roots,
            hints: &h,
            automation: &lookup,
            security: &sec,
        };
        assert!(matches!(
            evaluate("automation.run", &json!({"name": "read"}), &cx),
            Decision::Allow { .. }
        ));
        match evaluate("automation.run", &json!({"name": "write"}), &cx) {
            Decision::Ask {
                reason, standing_ok, ..
            } => {
                assert!(!standing_ok, "each run asks");
                assert!(reason.contains("2. files.create"), "{reason}");
            }
            d => panic!("{d:?}"),
        }
        assert!(matches!(
            evaluate("automation.run", &json!({"name": "outside"}), &cx),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            evaluate("automation.run", &json!({"name": "nope"}), &cx),
            Decision::Deny { .. }
        ));
        // Creating: steps validated; automations cannot nest or use unknown tools.
        assert!(matches!(
            evaluate(
                "automation.create",
                &json!({"name": "x1", "description": "d", "steps": [{"tool": "automation.run", "args": {}}]}),
                &cx
            ),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            evaluate(
                "automation.create",
                &json!({"name": "x1", "description": "d", "steps": [{"tool": "shell.exec", "args": {}}]}),
                &cx
            ),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            evaluate(
                "automation.create",
                &json!({"name": "x1", "description": "d", "steps": [{"tool": "files.list", "args": {"path": "/x"}}]}),
                &cx
            ),
            Decision::Ask { .. }
        ));
    }

    #[test]
    fn network_tools_ask_and_research_sources_must_be_granted() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().canonicalize().unwrap();
        std::fs::write(g.join("a.txt"), "x").unwrap();
        let roots = vec![g.clone()];
        let h = HashMap::new();
        for t in ["browser.search", "browser.open", "github.search", "github.inspect"] {
            assert!(
                matches!(
                    run(
                        t,
                        json!({"query": "q", "url": "https://x.org", "repo": "a/b"}),
                        &roots,
                        &h
                    ),
                    Decision::Ask {
                        standing_ok: true,
                        hard_floor: false,
                        ..
                    }
                ),
                "{t}"
            );
        }
        assert!(matches!(
            run(
                "research.summarize",
                json!({"sources": [{"path": g.join("a.txt")}, {"text": "t"}]}),
                &roots,
                &h
            ),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            run(
                "research.summarize",
                json!({"sources": [{"path": "/etc/hosts"}]}),
                &roots,
                &h
            ),
            Decision::Deny { .. }
        ));
        assert!(matches!(
            run(
                "documents.create",
                json!({"path": g.join("r.md"), "title": "t", "sections": []}),
                &roots,
                &h
            ),
            Decision::Ask { .. }
        ));
        assert!(matches!(
            run(
                "documents.create",
                json!({"path": "/tmp/r.md", "title": "t", "sections": []}),
                &roots,
                &h
            ),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn mcp_hints_can_only_make_calls_stricter() {
        let mut h = HashMap::new();
        h.insert(
            "apps.notes_find".to_string(),
            ToolHints {
                read_only: true,
                destructive: false,
            },
        );
        h.insert(
            "apps.notes_clear".to_string(),
            ToolHints {
                read_only: false,
                destructive: true,
            },
        );
        h.insert(
            "apps.notes_tag".to_string(),
            ToolHints {
                read_only: false,
                destructive: false,
            },
        );
        assert!(matches!(
            run("apps.notes_find", json!({}), &[], &h),
            Decision::Ask {
                standing_ok: true,
                hard_floor: false,
                ..
            }
        ));
        assert!(matches!(
            run("apps.notes_clear", json!({}), &[], &h),
            Decision::Ask { hard_floor: true, .. }
        ));
        assert!(matches!(
            run("apps.notes_tag", json!({}), &[], &h),
            Decision::Ask {
                standing_ok: false,
                hard_floor: false,
                ..
            }
        ));
        // A "read-only" claim does not soften a hard-floor action name.
        h.insert(
            "apps.mail_send_email".to_string(),
            ToolHints {
                read_only: true,
                destructive: false,
            },
        );
        assert!(matches!(
            run("apps.mail_send_email", json!({}), &[], &h),
            Decision::Ask { hard_floor: true, .. }
        ));
    }

    #[test]
    fn security_switches_override_everything() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().canonicalize().unwrap();
        std::fs::write(g.join("a.txt"), "x").unwrap();
        let roots = vec![g.clone()];
        let mut h = HashMap::new();
        h.insert(
            "apps.x_find".to_string(),
            ToolHints {
                read_only: true,
                destructive: false,
            },
        );
        let eval = |t: &str, a: Value, sec: crate::coworker::ledger::Security| {
            evaluate(
                t,
                &a,
                &Context {
                    roots: &roots,
                    hints: &h,
                    automation: &|_| None,
                    security: &sec,
                },
            )
        };
        let read = json!({"path": g.join("a.txt")});
        let paused = crate::coworker::ledger::Security {
            paused: true,
            ..Default::default()
        };
        assert!(matches!(
            eval("files.read", read.clone(), paused),
            Decision::Deny { .. }
        ));
        let offline = crate::coworker::ledger::Security {
            network_off: true,
            ..Default::default()
        };
        assert!(matches!(eval("files.read", read, offline), Decision::Allow { .. }));
        for t in ["browser.open", "github.search", "apps.x_find", "apps.y_other"] {
            assert!(
                matches!(eval(t, json!({"url": "https://x.org"}), offline), Decision::Deny { .. }),
                "{t}"
            );
        }
        // Code: off by default; when on it always asks, never per chat.
        let code = json!({"language": "python", "code": "print(1)"});
        assert!(matches!(
            eval("code.run", code.clone(), Default::default()),
            Decision::Deny { .. }
        ));
        let on = crate::coworker::ledger::Security {
            code_enabled: true,
            ..Default::default()
        };
        if crate::coworker::sandbox::available() {
            assert!(matches!(
                eval("code.run", code, on),
                Decision::Ask { standing_ok: false, .. }
            ));
            assert!(matches!(
                eval(
                    "code.run",
                    json!({"language": "shell", "code": "x", "input_files": ["/etc/hosts"]}),
                    on
                ),
                Decision::Deny { .. }
            ));
        }
    }

    #[test]
    fn automatic_web_research_covers_only_read_only_web_tools() {
        let h = HashMap::new();
        let sec = crate::coworker::ledger::Security {
            auto_web: true,
            ..Default::default()
        };
        let eval = |t: &str, a: Value, sec: crate::coworker::ledger::Security| {
            evaluate(
                t,
                &a,
                &Context {
                    roots: &[],
                    hints: &h,
                    automation: &|_| None,
                    security: &sec,
                },
            )
        };
        for t in ["browser.search", "browser.open", "github.search", "github.inspect"] {
            assert!(
                matches!(
                    eval(t, json!({"query": "x", "url": "https://x.org", "repo": "a/b"}), sec),
                    Decision::Allow { .. }
                ),
                "{t}"
            );
        }
        // Writes, connected apps and code still ask or are refused.
        assert!(matches!(
            eval(
                "documents.create",
                json!({"path": "/tmp/x.md", "title": "t", "sections": []}),
                sec
            ),
            Decision::Deny { .. }
        ));
        let mut hh = HashMap::new();
        hh.insert(
            "apps.x_find".to_string(),
            ToolHints {
                read_only: true,
                destructive: false,
            },
        );
        assert!(matches!(
            evaluate(
                "apps.x_find",
                &json!({}),
                &Context {
                    roots: &[],
                    hints: &hh,
                    automation: &|_| None,
                    security: &sec
                }
            ),
            Decision::Ask { .. }
        ));
        // The internet switch still wins.
        let off = crate::coworker::ledger::Security {
            auto_web: true,
            network_off: true,
            ..Default::default()
        };
        assert!(matches!(
            eval("browser.search", json!({"query": "x"}), off),
            Decision::Deny { .. }
        ));
    }
}
