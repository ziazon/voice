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

#[tauri::command(async)]
pub fn save_options(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::SaveOptions, persona_builder_api::dto::Failure> {
    persona_builder_api::save_options(request)
}

#[tauri::command(async)]
pub fn open_link(
    request: persona_builder_api::dto::LinkRequest,
) -> Result<(), persona_builder_api::dto::Failure> {
    persona_builder_api::open_link(request)
}

#[tauri::command(async)]
pub fn github_sign_in_start()
-> Result<persona_builder_api::dto::SignInCode, persona_builder_api::dto::Failure> {
    persona_builder_api::github_sign_in_start()
}

#[tauri::command(async)]
pub fn github_sign_in_finish()
-> Result<persona_builder_api::dto::Said, persona_builder_api::dto::Failure> {
    persona_builder_api::github_sign_in_finish()
}

#[tauri::command(async)]
pub fn github_sign_in_cancel() -> Result<(), persona_builder_api::dto::Failure> {
    persona_builder_api::github_sign_in_cancel()
}

#[tauri::command(async)]
pub fn github_sign_out() -> Result<persona_builder_api::dto::Said, persona_builder_api::dto::Failure>
{
    persona_builder_api::github_sign_out()
}

#[tauri::command(async)]
pub fn save_choose(
    request: persona_builder_api::dto::SaveChoiceRequest,
) -> Result<persona_builder_api::dto::Said, persona_builder_api::dto::Failure> {
    persona_builder_api::save_choose(request)
}

#[tauri::command(async)]
pub fn save_push(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::Said, persona_builder_api::dto::Failure> {
    persona_builder_api::save_push(request)
}
