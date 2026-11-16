//! DigitsNepal Coworker: an agent that understands Romanized Nepali, works in
//! folders the user grants, and asks before changing anything.
//!
//! * `ledger`   — grants, sessions, messages, tamper-evident audit
//! * `policy`   — allow / ask / deny for every tool call (the only authority)
//! * `files`    — folder-scoped file tools with copy-on-write
//! * `language` — offline Nepali understanding tool
//! * `providers`— settings → agent-engine model connections
//! * `service`  — the OpenWorker Harness runtime and session/approval loop

pub mod agents;
pub mod documents;
pub mod files;
pub mod language;
pub mod ledger;
pub mod mcp;
pub mod policy;
pub mod providers;
#[cfg(feature = "coworker")]
pub mod relay;
pub mod sandbox;
#[cfg(feature = "coworker")]
pub mod service;
pub mod specialists;
pub mod tools;
pub mod web;
