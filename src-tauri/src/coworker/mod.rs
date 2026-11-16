//! DigitsNepal Coworker: an agent that understands Romanized Nepali, works in
//! folders the user grants, and investigates authorized software with REA.
//!
//! * `ledger`   — grants, REA targets, sessions, messages, tamper-evident audit
//! * `policy`   — allow / ask / deny for every tool call (the only authority)
//! * `files`    — folder-scoped file tools with copy-on-write
//! * `language` — offline Nepali understanding tool
//! * `providers`— settings → agent-engine model connections
//! * `rea`      — REA readiness, scoped MCP launch, live tool catalog
//! * `service`  — the OpenWorker Harness runtime and session/approval loop

pub mod files;
pub mod language;
pub mod ledger;
pub mod policy;
pub mod providers;
pub mod rea;
#[cfg(feature = "coworker")]
pub mod relay;
#[cfg(feature = "coworker")]
pub mod service;
pub mod specialists;
