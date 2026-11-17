//! Durable tasks: a user goal tracked through a validated state machine,
//! with the agent's plan, step progress, evidence and a verified result.
//!
//! The agent reports structure through three tools (`task.plan`,
//! `task.update`, `task.complete`) whose inputs DigitsNepal validates. Task
//! state also follows the engine's events (approvals, questions, errors,
//! cancellation). A task only reaches COMPLETED when every action it claims
//! is backed by a successful tool run in the audit log; claims DigitsNepal
//! cannot verify (bookings, purchases, payments, messages) block completion.

use super::intent::IntentHints;
use super::ledger::{Ledger, TaskField, TaskRow};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    Created,
    NeedsClarification,
    Planning,
    Ready,
    Running,
    WaitingForApproval,
    WaitingForExternalResponse,
    Retrying,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

use TaskState::*;

impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            Created => "CREATED",
            NeedsClarification => "NEEDS_CLARIFICATION",
            Planning => "PLANNING",
            Ready => "READY",
            Running => "RUNNING",
            WaitingForApproval => "WAITING_FOR_APPROVAL",
            WaitingForExternalResponse => "WAITING_FOR_EXTERNAL_RESPONSE",
            Retrying => "RETRYING",
            Blocked => "BLOCKED",
            Completed => "COMPLETED",
            Failed => "FAILED",
            Cancelled => "CANCELLED",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        serde_json::from_value(Value::String(s.to_string())).ok()
    }

    pub fn terminal(self) -> bool {
        matches!(self, Completed | Cancelled)
    }

    /// Valid transitions. FAILED is final for the run but may be retried by
    /// the user; COMPLETED and CANCELLED are final.
    pub fn can_go(self, to: TaskState) -> bool {
        if self == to {
            return false;
        }
        match self {
            Created => matches!(to, Planning | NeedsClarification | Cancelled | Failed),
            NeedsClarification => matches!(to, Planning | Running | Blocked | Cancelled | Failed),
            Planning => matches!(
                to,
                Ready
                    | Running
                    | NeedsClarification
                    | WaitingForApproval
                    | Retrying
                    | Blocked
                    | Failed
                    | Cancelled
                    | Completed
            ),
            Ready => matches!(to, Running | Blocked | Cancelled),
            Running => matches!(
                to,
                WaitingForApproval
                    | WaitingForExternalResponse
                    | NeedsClarification
                    | Retrying
                    | Blocked
                    | Completed
                    | Failed
                    | Cancelled
            ),
            WaitingForApproval => matches!(to, Running | Blocked | Failed | Cancelled),
            WaitingForExternalResponse => matches!(to, Running | Blocked | Failed | Cancelled),
            Retrying => matches!(to, Running | Planning | Blocked | Failed | Cancelled),
            // A corrected, verified result may finish a blocked task.
            Blocked => matches!(
                to,
                Running | Planning | NeedsClarification | Completed | Cancelled | Failed
            ),
            Failed => matches!(to, Retrying | Cancelled),
            Completed | Cancelled => false,
        }
    }
}

/// Validated transition; returns false (and changes nothing) if not allowed
/// or if the state changed concurrently.
pub fn transition(ledger: &Ledger, task: &TaskRow, to: TaskState, reason: &str) -> bool {
    let Some(from) = TaskState::parse(&task.state) else {
        return false;
    };
    if !from.can_go(to) {
        return false;
    }
    ledger
        .set_task_state(&task.id, from.as_str(), to.as_str(), reason)
        .unwrap_or(false)
}

/// Move the task for a session to `to` if that transition is valid now.
pub fn advance(ledger: &Ledger, session: &str, to: TaskState, reason: &str) -> bool {
    match ledger.task_for_session(session) {
        Some(t) => transition(ledger, &t, to, reason),
        None => false,
    }
}

// ---- Workflow playbooks --------------------------------------------------------

pub struct Playbook {
    pub key: &'static str,
    pub title: &'static str,
    pub specialist: &'static str,
    pub steps: &'static [&'static str],
    /// What the agent must stop at, because DigitsNepal has no integration
    /// that could complete it.
    pub stop_at: &'static str,
}

pub const PLAYBOOKS: &[Playbook] = &[
    Playbook {
        key: "housing",
        title: "Find a place to rent",
        specialist: "general",
        steps: &[
            "Confirm area, type (room, flat, house), budget per month and move-in date (ask only for what is missing).",
            "Search listing sites for that area (browser.search) and open the 3–5 most relevant listing pages (browser.open).",
            "Compare rent, size, location and contact details with research.summarize; cite every listing; mark prices you could not confirm and listings that look old.",
            "Recommend the best options within the budget, with a short message the user can send to the owner or agent.",
        ],
        stop_at: "Stop at the shortlist and the prepared message. DigitsNepal cannot contact owners, visit, sign or pay: list those as next steps. Never invent listings or prices.",
    },
    Playbook {
        key: "local_service",
        title: "Find and arrange a local service",
        specialist: "general",
        steps: &[
            "Confirm the service, area, date/time and budget (ask only for what is missing and essential).",
            "Search the web for providers in that area (browser.search), then open the 3–5 most relevant pages (browser.open).",
            "Compare price, availability, reviews and contact details with research.summarize; cite every source.",
            "Recommend the best eligible option and one backup, with reasons.",
            "Prepare a ready-to-send booking request (call or message text in Nepali and English) with documents.create if the user wants it saved.",
        ],
        stop_at: "Stop at the recommendation and the prepared booking request. DigitsNepal cannot book, call, message or pay providers: list those as next steps for the user. Never say a booking was made.",
    },
    Playbook {
        key: "job_search",
        title: "Job search assistant",
        specialist: "general",
        steps: &[
            "Find and read the user's CV in a granted folder (files.search, then files.read or files.read_pdf). If none is granted, ask for it.",
            "List the user's real skills, experience and qualifications from the CV only.",
            "Search for matching openings (browser.search) and open the most promising postings (browser.open) to check they are current and have a real application link.",
            "Score each job 0–100 with a short explanation (skills matched, gaps, location, level).",
            "For the top matches, draft a tailored cover letter with documents.create (asks the user). Use only facts from the CV; never invent experience or credentials.",
        ],
        stop_at: "Stop before submitting any application. DigitsNepal cannot submit applications: give the verified application links as next steps. Never say an application was submitted.",
    },
    Playbook {
        key: "supplier_sourcing",
        title: "Supplier and price comparison",
        specialist: "general",
        steps: &[
            "Confirm the product specification, quantity, location and budget limits (ask only for what is missing).",
            "Search for suppliers (browser.search) and open their pages (browser.open) for prices, minimum order quantities and delivery.",
            "Compare total landed cost (price × quantity + delivery) in a table with research.summarize; cite sources; mark any price you could not confirm.",
            "Draft a quotation request and a counteroffer within the user's stated limits with documents.create (asks the user).",
            "Present the best proposed deal and what still needs the user's decision.",
        ],
        stop_at: "Stop before contacting suppliers, accepting an offer or paying. DigitsNepal cannot send messages, negotiate live or purchase: list those as next steps. Never invent offers, suppliers or prices.",
    },
];

pub fn playbook(category: &str) -> Option<&'static Playbook> {
    let key = match category {
        "appointment" | "travel" => "local_service",
        "shopping" => "supplier_sourcing",
        other => other,
    };
    PLAYBOOKS.iter().find(|p| p.key == key)
}

/// The message that starts a task in its conversation.
pub fn start_message(task: &TaskRow, hints: &IntentHints) -> String {
    let mut m = format!(
        "[Task {}] {}\n\nDigitsNepal's offline reading of this request (keep these values exactly; do not change amounts, dates or places):\n{}\n",
        task.id,
        task.request,
        serde_json::to_string_pretty(&json!({
            "category": hints.category,
            "language": hints.language,
            "dates": hints.dates,
            "times": hints.times,
            "amounts": hints.amounts,
            "locations": hints.locations,
            "preferences": hints.preferences,
            "sensitive_actions_requested": hints.sensitive_actions,
        }))
        .unwrap_or_default()
    );
    if let Some(p) = playbook(&hints.category) {
        m.push_str(&format!("\nWorkflow: {}\n", p.title));
        for (i, s) in p.steps.iter().enumerate() {
            m.push_str(&format!("{}. {s}\n", i + 1));
        }
        m.push_str(&format!("Limit: {}\n", p.stop_at));
    }
    m.push_str("\nFollow the task protocol: task.plan first, task.update after each step, task.complete at the end.");
    m
}

// ---- Validation of the agent's reports -------------------------------------------

const STEP_STATUSES: &[&str] = &["pending", "running", "done", "skipped", "failed"];

/// Validate and normalise a plan. Returns the stored plan and warnings where
/// the plan disagrees with what the user actually said.
pub fn check_plan(args: &Value, hints: &IntentHints) -> Result<(Value, Vec<String>), String> {
    let summary = args["summary"].as_str().unwrap_or("").trim();
    if summary.is_empty() {
        return Err("The plan needs a one-sentence summary of the goal.".into());
    }
    let steps = args["steps"].as_array().ok_or("The plan needs steps.")?;
    if steps.is_empty() || steps.len() > 12 {
        return Err("Give between 1 and 12 steps.".into());
    }
    let mut out_steps = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        let title = s["title"].as_str().unwrap_or("").trim();
        if title.is_empty() {
            return Err(format!("Step {} needs a title.", i + 1));
        }
        out_steps.push(json!({
            "title": title.chars().take(200).collect::<String>(),
            "tool": s["tool"].as_str().map(normalize_tool).unwrap_or_default(),
            "needs_approval": s["needs_approval"].as_bool().unwrap_or(false),
            "status": "pending",
            "note": "",
        }));
    }
    let mut warnings = Vec::new();
    // Amounts the user gave must appear unchanged.
    let plan_amounts: Vec<f64> = args["amounts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["amount"].as_f64())
        .collect();
    for a in &hints.amounts {
        if !plan_amounts.iter().any(|p| (p - a.amount).abs() < 0.5) {
            warnings.push(format!(
                "The user said {} ({} {}); the plan does not keep that amount.",
                a.text, a.amount, a.currency
            ));
        }
    }
    let plan_dates: Vec<&str> = args["dates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    for d in &hints.dates {
        if !plan_dates.contains(&d.date.as_str()) {
            warnings.push(format!(
                "The user said \"{}\" ({}); the plan does not keep that date.",
                d.text, d.date
            ));
        }
    }
    let plan_text = args.to_string().to_lowercase();
    for l in &hints.locations {
        if l != "Nepal" && !plan_text.contains(&l.to_lowercase()) {
            warnings.push(format!("The user mentioned {l}; the plan does not.",));
        }
    }
    let missing: Vec<String> = args["missing_info"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .take(10)
        .collect();
    Ok((
        json!({
            "summary": summary,
            "category": args["category"].as_str().unwrap_or(&hints.category),
            "amounts": args["amounts"],
            "dates": args["dates"],
            "missing_info": missing,
            "steps": out_steps,
            "warnings": warnings,
        }),
        warnings,
    ))
}

pub fn update_step(plan: &mut Value, args: &Value) -> Result<(), String> {
    let i = args["step"].as_u64().ok_or("Give the step number.")? as usize;
    let status = args["status"].as_str().unwrap_or("");
    if !STEP_STATUSES.contains(&status) {
        return Err(format!("status must be one of {}", STEP_STATUSES.join(", ")));
    }
    let steps = plan["steps"].as_array_mut().ok_or("Call task.plan first.")?;
    let step = steps
        .get_mut(i.checked_sub(1).ok_or("Steps are numbered from 1.")?)
        .ok_or("There is no such step.")?;
    step["status"] = json!(status);
    if let Some(n) = args["note"].as_str() {
        step["note"] = json!(n.chars().take(500).collect::<String>());
    }
    Ok(())
}

/// Accept both the qualified name (`files.read`) and the model-facing alias
/// (`files__read`).
pub fn normalize_tool(name: &str) -> String {
    let n = name.trim();
    let dotted = if n.contains('.') {
        n.to_string()
    } else {
        n.replacen("__", ".", 1)
    };
    super::tools::canonical(&dotted).to_string()
}

/// Actions DigitsNepal can never have performed (no tools exist for them).
const UNSUPPORTED: &[&str] = &[
    "book", "purchase", "buy", "pay", "send", "submit", "email", "call", "order", "transfer",
];

/// Verify each action the agent claims against the audit log and the disk.
pub fn verify_claims(ledger: &Ledger, session: &str, claims: &[Value]) -> Vec<Value> {
    claims
        .iter()
        .take(30)
        .map(|c| {
            let tool = normalize_tool(c["tool"].as_str().unwrap_or(""));
            let target = c["target"].as_str().unwrap_or("").to_string();
            let (ok, why) = match super::tools::spec(&tool) {
                None => {
                    let lower = tool.to_lowercase();
                    let why = if UNSUPPORTED.iter().any(|w| lower.contains(w)) {
                        "DigitsNepal has no tool that can do this, so it cannot have happened."
                    } else {
                        "Not a DigitsNepal tool."
                    };
                    (false, why.to_string())
                }
                Some(_) if !ledger.session_ran_ok(session, &tool) => (
                    false,
                    "No successful run of this tool is recorded for this task.".into(),
                ),
                Some(_) if matches!(tool.as_str(), "files.create" | "documents.create" | "files.move") => {
                    if !target.is_empty() && std::path::Path::new(&target).exists() {
                        (true, "Recorded in the audit log and the file exists.".into())
                    } else {
                        (false, "The file it names does not exist.".into())
                    }
                }
                Some(_) => (true, "Recorded in the audit log.".into()),
            };
            json!({"tool": tool, "target": target, "verified": ok, "why": why})
        })
        .collect()
}

pub fn record(ledger: &Ledger, task: &TaskRow, field: TaskField, value: &Value) {
    ledger.set_task_field(&task.id, field, value).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_rejects_invalid_transitions() {
        assert!(Created.can_go(Planning));
        assert!(Running.can_go(WaitingForApproval));
        assert!(WaitingForApproval.can_go(Running));
        assert!(Failed.can_go(Retrying));
        assert!(!Created.can_go(Completed), "cannot complete without running");
        assert!(!Completed.can_go(Running), "completed is final");
        assert!(!Cancelled.can_go(Running), "cancelled is final");
        assert!(!Ready.can_go(Completed));
        assert!(!Running.can_go(Running));
        for s in [
            Created,
            NeedsClarification,
            Planning,
            Ready,
            Running,
            WaitingForApproval,
            WaitingForExternalResponse,
            Retrying,
            Blocked,
            Failed,
        ] {
            assert!(s.can_go(Cancelled), "{s:?} must be cancellable");
        }
        assert_eq!(TaskState::parse("WAITING_FOR_APPROVAL"), Some(WaitingForApproval));
        assert_eq!(WaitingForApproval.as_str(), "WAITING_FOR_APPROVAL");
    }

    #[test]
    fn transitions_are_durable_and_compare_and_set() {
        let l = Ledger::in_memory().unwrap();
        let t = TaskRow {
            id: "t1".into(),
            session: "s1".into(),
            specialist: "general".into(),
            request: "r".into(),
            intent: json!({}),
            plan: json!({}),
            evidence: json!([]),
            result: json!({}),
            state: "CREATED".into(),
            state_reason: String::new(),
            paused: false,
            created_at: 0,
            updated_at: 0,
        };
        l.insert_task(&t).unwrap();
        assert!(transition(&l, &t, Planning, "start"));
        assert!(!transition(&l, &t, Planning, "stale row"), "state changed meanwhile");
        let now = l.task("t1").unwrap();
        assert!(!transition(&l, &now, WaitingForExternalResponse, "skip"), "invalid");
        assert!(advance(&l, "s1", Running, "go"));
        assert_eq!(l.task("t1").unwrap().state, "RUNNING");
        let ev = l.task_events("t1");
        assert_eq!(ev.len(), 2);
        assert_eq!(
            (ev[1]["from"].as_str(), ev[1]["to"].as_str()),
            (Some("PLANNING"), Some("RUNNING"))
        );
    }

    #[test]
    fn plans_must_keep_the_users_amounts_dates_and_places() {
        let h = super::super::intent::analyze_at(
            "Kathmandu ma bholi 5 hajar bhitra ghar safa garne service",
            chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        );
        let good = json!({"summary": "Find a cleaner in Kathmandu", "amounts": [{"amount": 5000, "currency": "NPR"}], "dates": ["2026-11-17"], "steps": [{"title": "Search"}]});
        let (_, w) = check_plan(&good, &h).unwrap();
        assert!(w.is_empty(), "{w:?}");
        let changed = json!({"summary": "Find a cleaner", "amounts": [{"amount": 50000}], "dates": ["2026-11-18"], "steps": [{"title": "Search"}]});
        let (_, w) = check_plan(&changed, &h).unwrap();
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(check_plan(&json!({"summary": "x", "steps": []}), &h).is_err());
        assert!(check_plan(&json!({"steps": [{"title": "a"}]}), &h).is_err());
    }

    #[test]
    fn steps_update_with_validation() {
        let mut p = json!({"steps": [{"title": "a", "status": "pending"}]});
        update_step(&mut p, &json!({"step": 1, "status": "done", "note": "ok"})).unwrap();
        assert_eq!(p["steps"][0]["status"], "done");
        assert!(update_step(&mut p, &json!({"step": 2, "status": "done"})).is_err());
        assert!(update_step(&mut p, &json!({"step": 1, "status": "finished"})).is_err());
        assert!(update_step(&mut p, &json!({"step": 0, "status": "done"})).is_err());
    }

    #[test]
    fn claims_are_verified_against_the_audit_log() {
        let l = Ledger::in_memory().unwrap();
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("letter.md");
        std::fs::write(&f, "x").unwrap();
        l.record("s", "agent", "tool_run", "documents.create", &json!({}), "ok", "")
            .unwrap();
        l.record("s", "agent", "tool_run", "browser.search", &json!({}), "ok", "")
            .unwrap();
        let v = verify_claims(
            &l,
            "s",
            &[
                json!({"tool": "documents.create", "target": f.display().to_string()}),
                json!({"tool": "browser.search", "target": "cleaners"}),
                json!({"tool": "files.create", "target": "/x"}),
                json!({"tool": "booking.create", "target": "CleanCo 10am"}),
                json!({"tool": "documents.create", "target": "/nope.md"}),
            ],
        );
        let ok: Vec<bool> = v.iter().map(|x| x["verified"].as_bool().unwrap()).collect();
        assert_eq!(ok, vec![true, true, false, false, false]);
        assert!(v[3]["why"].as_str().unwrap().contains("no tool"));
        // The model-facing alias form is accepted too.
        let a = verify_claims(&l, "s", &[json!({"tool": "browser__search", "target": "x"})]);
        assert_eq!(a[0]["verified"], true, "{a:?}");
        assert_eq!(normalize_tool("documents__create"), "documents.create");
        assert_eq!(normalize_tool("dn.read_file"), "files.read");
    }

    #[test]
    fn categories_map_to_the_three_workflows() {
        assert_eq!(playbook("job_search").unwrap().key, "job_search");
        assert_eq!(playbook("appointment").unwrap().key, "local_service");
        assert_eq!(playbook("shopping").unwrap().key, "supplier_sourcing");
        assert!(playbook("documents").is_none());
        assert_eq!(playbook("housing").unwrap().key, "housing");
        for p in PLAYBOOKS {
            assert!(p.stop_at.contains("Never"), "{}", p.key);
        }
    }
}
