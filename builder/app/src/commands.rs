#[tauri::command]
pub fn app_info() -> persona_builder_api::dto::AppInfo {
    persona_builder_api::app_info()
}

#[tauri::command(async)]
pub fn list_assistants()
-> Result<Vec<persona_builder_api::dto::Assistant>, persona_builder_api::dto::Failure> {
    persona_builder_api::list_assistants()
}

#[tauri::command(async)]
pub fn new_assistant(
    request: persona_builder_api::dto::NewAssistantRequest,
) -> Result<persona_builder_api::dto::Assistant, persona_builder_api::dto::Failure> {
    persona_builder_api::new_assistant(request)
}

#[tauri::command(async)]
pub fn show_assistant_folder(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<(), persona_builder_api::dto::Failure> {
    persona_builder_api::show_assistant_folder(request)
}

#[tauri::command(async)]
pub fn consent_text()
-> Result<persona_builder_api::dto::ConsentText, persona_builder_api::dto::Failure> {
    persona_builder_api::consent_text()
}

#[tauri::command(async)]
pub fn agree_consent(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::Consent, persona_builder_api::dto::Failure> {
    persona_builder_api::agree_consent(request)
}

#[tauri::command(async)]
pub fn claude_check()
-> Result<persona_builder_api::dto::ClaudeStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::claude_check()
}
