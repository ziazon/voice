use persona_builder_api::{
    app_info,
    dto::{AgentInfo, AppInfo},
};

#[test]
fn gives_the_workspace_version_and_agents_in_order() {
    let info = app_info();
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    let agents: Vec<_> = persona_builder_agent::agents::AGENTS
        .iter()
        .map(|agent| AgentInfo {
            id: agent.id.to_owned(),
            name: agent.name.to_owned(),
        })
        .collect();
    assert_eq!(info.agents, agents);
}

#[test]
fn serializes_the_answer_with_camel_case_keys() {
    let info: AppInfo = app_info();
    let agents: Vec<_> = persona_builder_agent::agents::AGENTS
        .iter()
        .map(|agent| serde_json::json!({"id": agent.id, "name": agent.name}))
        .collect();
    assert_eq!(
        serde_json::to_value(info).unwrap(),
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"), "agents": agents,
        })
    );
}
