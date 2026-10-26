pub mod assistants;
pub use assistants::*;
pub mod dto;
pub mod parity;

pub fn app_info() -> dto::AppInfo {
    dto::AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        agents: persona_builder_agent::agents::AGENTS
            .iter()
            .map(|agent| dto::AgentInfo {
                id: agent.id.to_owned(),
                name: agent.name.to_owned(),
            })
            .collect(),
    }
}

pub mod save;
pub use save::*;

pub mod history;
pub use history::*;
pub mod use_it;
pub use use_it::*;

pub mod building;
pub use building::*;

pub mod review;
pub use review::*;
