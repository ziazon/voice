//! The providers Persona Builder can drive, each with where its data goes.
use crate::{claude, provider::Reach};

/// One provider: its id (for the CLI and the app), name (for people), and where its data goes.
#[derive(Debug, PartialEq, Eq)]
pub struct Agent {
    pub id: &'static str,
    pub name: &'static str,
    pub reach: Reach,
}

pub const AGENTS: &[Agent] = &[Agent {
    id: claude::AGENT_ID,
    name: claude::AGENT_NAME,
    reach: claude::REACH,
}];

/// The agent with this id.
pub fn agent(id: &str) -> Option<&'static Agent> {
    AGENTS.iter().find(|agent| agent.id == id)
}
