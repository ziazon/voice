//! The agents Persona Builder can drive. Later providers join this list.

/// One agent: its id (for the CLI and the app) and its name (for people).
#[derive(Debug, PartialEq, Eq)]
pub struct Agent {
    pub id: &'static str,
    pub name: &'static str,
}

pub const AGENTS: &[Agent] = &[Agent {
    id: crate::claude::AGENT_ID,
    name: "Claude, through your own Claude Code",
}];

/// The agent with this id.
pub fn agent(id: &str) -> Option<&'static Agent> {
    AGENTS.iter().find(|agent| agent.id == id)
}
