//! Versioned, immutable agent definitions.
//!
//! A definition's ID is derived from its full content (instructions, tools,
//! model, limits). It is stored once in the ledger and never modified, so a
//! session always resumes with the exact definition it started with; any
//! change produces a new ID.

use super::specialists;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// Model calls allowed in one user turn.
    pub max_model_calls: u32,
    /// Output tokens allowed in one user turn.
    pub max_output_tokens: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_model_calls: 40,
            max_output_tokens: 120_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Stable kind, e.g. `coworker`.
    pub key: String,
    /// Bumped when DigitsNepal changes the definition's shape or tools.
    pub version: u32,
    pub name: String,
    pub instructions: String,
    pub tools: Vec<String>,
    /// Engine connection name and model the agent uses.
    pub connection: String,
    pub model: String,
    pub limits: Limits,
}

impl AgentDefinition {
    pub fn id(&self) -> String {
        let canonical = serde_json::to_string(self).expect("definition serializes");
        let digest = super::ledger::sha256_hex(canonical.as_bytes());
        format!("dn-{}-v{}-{}", self.key, self.version, &digest[..10])
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("definition serializes")
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        serde_json::from_value(v.clone()).ok()
    }
}

/// The definition for a specialist on the given connection and model.
pub fn for_specialist(s: &specialists::Specialist, connection: &str, model: &str) -> AgentDefinition {
    AgentDefinition {
        key: s.key.into(),
        version: s.version,
        name: s.name.into(),
        instructions: s.instructions(),
        tools: s.tools.iter().map(|t| t.to_string()).collect(),
        connection: connection.into(),
        model: model.into(),
        limits: Limits::default(),
    }
}

/// The general DigitsNepal coworker (Sahayogi).
pub fn coworker(connection: &str, model: &str) -> AgentDefinition {
    for_specialist(
        specialists::find("general").expect("general specialist"),
        connection,
        model,
    )
}

#[cfg(feature = "coworker")]
pub fn to_engine(def: &AgentDefinition, id: &str) -> openworker_sdk::Agent {
    use openworker_sdk::{Agent, Approvals, BuiltinTools, TurnLimits};
    Agent::new(id.to_string())
        .name(def.name.clone())
        .instructions(def.instructions.clone())
        .model(def.connection.clone(), def.model.clone())
        .tools(def.tools.clone())
        .approvals(Approvals::Always)
        .builtin_tools(BuiltinTools {
            ask_user: Some(true),
            read_history: Some(false),
        })
        .limits(TurnLimits {
            max_model_calls: def.limits.max_model_calls,
            max_output_tokens: def.limits.max_output_tokens,
            ..TurnLimits::default()
        })
        .planning(false)
        .memory(false)
        .scheduling(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_changes_with_any_content_and_round_trips() {
        let a = coworker("conn", "model-a");
        let b = coworker("conn", "model-b");
        assert_ne!(a.id(), b.id());
        assert!(a.id().starts_with("dn-general-v4-"));
        let mut c = a.clone();
        c.limits.max_model_calls = 5;
        assert_ne!(a.id(), c.id());
        assert_eq!(AgentDefinition::from_json(&a.to_json()).unwrap(), a);
        assert_eq!(coworker("conn", "model-a").id(), a.id(), "deterministic");
    }

    #[test]
    fn specialists_use_only_registered_typed_tools() {
        for s in specialists::SPECIALISTS {
            let d = for_specialist(s, "c", "m");
            assert!(d.id().starts_with(&format!("dn-{}-v{}-", s.key, s.version)));
            for t in &d.tools {
                assert!(super::super::tools::spec(t).is_some(), "{t} is not a registered tool");
                assert_eq!(super::super::tools::canonical(t), t, "{t} must use the canonical name");
            }
        }
    }
}
