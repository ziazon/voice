#[tauri::command]
pub fn app_info() -> persona_builder_api::dto::AppInfo {
    persona_builder_api::app_info()
}
