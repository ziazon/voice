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
pub fn claude_code_folders(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ClaudeCodeFolders, persona_builder_api::dto::Failure> {
    persona_builder_api::claude_code_folders(request)
}

#[tauri::command(async)]
pub fn read_claude_code(
    request: persona_builder_api::dto::ReadRequest,
    on_progress: tauri::ipc::Channel<persona_builder_api::dto::ReadProgress>,
) -> Result<persona_builder_api::dto::ReadCounts, persona_builder_api::dto::Failure> {
    persona_builder_api::read_claude_code(request, &mut |progress| {
        let _ = on_progress.send(progress);
    })
}

#[tauri::command(async)]
pub fn choose_claudeai_export(
    request: persona_builder_api::dto::ChooseExportRequest,
) -> Result<Option<persona_builder_api::dto::ExportStatus>, persona_builder_api::dto::Failure> {
    let slug = request.slug;
    persona_builder_api::can_read(persona_builder_api::dto::SlugRequest { slug: slug.clone() })?;
    let mut dialog = rfd::FileDialog::new().set_title("Choose your claude.ai download");
    if let Some(home) = std::env::var_os("HOME") {
        let downloads = std::path::PathBuf::from(home).join("Downloads");
        if downloads.is_dir() {
            dialog = dialog.set_directory(downloads);
        }
    }
    let chosen = if request.folder {
        dialog.pick_folder()
    } else {
        dialog
            .add_filter("claude.ai download", &["zip", "json"])
            .pick_file()
    };
    match chosen {
        None => Ok(None),
        Some(path) => persona_builder_api::use_claudeai_export(&slug, &path).map(Some),
    }
}

#[tauri::command(async)]
pub fn claudeai_export_status(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<Option<persona_builder_api::dto::ExportStatus>, persona_builder_api::dto::Failure> {
    persona_builder_api::claudeai_export_status(request)
}

#[tauri::command(async)]
pub fn list_claudeai_chats(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ChatList, persona_builder_api::dto::Failure> {
    persona_builder_api::list_claudeai_chats(request)
}

#[tauri::command(async)]
pub fn mark_claudeai_chats(
    request: persona_builder_api::dto::MarkChatsRequest,
) -> Result<persona_builder_api::dto::ChatList, persona_builder_api::dto::Failure> {
    persona_builder_api::mark_claudeai_chats(request)
}

#[tauri::command(async)]
pub fn read_claudeai(
    request: persona_builder_api::dto::SlugRequest,
    on_progress: tauri::ipc::Channel<persona_builder_api::dto::ReadProgress>,
) -> Result<persona_builder_api::dto::ClaudeaiCounts, persona_builder_api::dto::Failure> {
    persona_builder_api::read_claudeai(request, &mut |progress| {
        let _ = on_progress.send(progress);
    })
}

#[tauri::command(async)]
pub fn merge_histories(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::MergeCounts, persona_builder_api::dto::Failure> {
    persona_builder_api::merge_histories(request)
}
