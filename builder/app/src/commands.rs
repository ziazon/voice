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
pub fn claude_sign_in()
-> Result<persona_builder_api::dto::ClaudeStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::claude_sign_in()
}

#[tauri::command(async)]
pub fn claude_sign_in_cancel() -> Result<(), persona_builder_api::dto::Failure> {
    persona_builder_api::claude_sign_in_cancel()
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

#[tauri::command(async)]
pub fn claude_code_status(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ClaudeCodeStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::claude_code_status(request)
}

#[tauri::command(async)]
pub fn turn_on(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::Said, persona_builder_api::dto::Failure> {
    persona_builder_api::turn_on(request)
}

#[tauri::command(async)]
pub fn turn_off(
    request: persona_builder_api::dto::TurnOffRequest,
) -> Result<persona_builder_api::dto::Said, persona_builder_api::dto::Failure> {
    persona_builder_api::turn_off(request)
}

#[tauri::command(async)]
pub fn claude_ai_files(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ClaudeAiFiles, persona_builder_api::dto::Failure> {
    persona_builder_api::claude_ai_files(request)
}

#[tauri::command(async)]
pub fn make_claude_ai_files(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ClaudeAiFiles, persona_builder_api::dto::Failure> {
    persona_builder_api::make_claude_ai_files(request)
}

#[tauri::command(async)]
pub fn show_claude_ai_files(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<(), persona_builder_api::dto::Failure> {
    persona_builder_api::show_claude_ai_files(request)
}

#[tauri::command(async)]
pub fn forget_preview(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ForgetPreview, persona_builder_api::dto::Failure> {
    persona_builder_api::forget_preview(request)
}

#[tauri::command(async)]
pub fn forget(
    request: persona_builder_api::dto::ForgetRequest,
) -> Result<persona_builder_api::dto::Said, persona_builder_api::dto::Failure> {
    persona_builder_api::forget(request)
}

#[tauri::command(async)]
pub fn build_assistant(
    request: persona_builder_api::dto::SlugRequest,
    on_progress: tauri::ipc::Channel<persona_builder_api::dto::BuildProgress>,
) -> Result<persona_builder_api::dto::BuildDone, persona_builder_api::dto::Failure> {
    persona_builder_api::build_assistant(request, &mut |p| {
        let _ = on_progress.send(p);
    })
}

#[tauri::command(async)]
pub fn stop_build(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<(), persona_builder_api::dto::Failure> {
    persona_builder_api::stop_build(request)
}

#[tauri::command(async)]
pub fn spend_today(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::Spend, persona_builder_api::dto::Failure> {
    persona_builder_api::spend_today(request)
}

#[tauri::command(async)]
pub fn spending_limit(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::SpendingLimit, persona_builder_api::dto::Failure> {
    persona_builder_api::spending_limit(request)
}

#[tauri::command(async)]
pub fn set_spending_limit(
    request: persona_builder_api::dto::SpendingLimitRequest,
) -> Result<persona_builder_api::dto::SpendingLimit, persona_builder_api::dto::Failure> {
    persona_builder_api::set_spending_limit(request)
}

#[tauri::command(async)]
pub fn interview_status(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::InterviewStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::interview_status(request)
}

#[tauri::command(async)]
pub fn draft_interview(
    request: persona_builder_api::dto::DraftInterviewRequest,
) -> Result<persona_builder_api::dto::InterviewStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::draft_interview(request)
}

#[tauri::command(async)]
pub fn save_interview_answers(
    request: persona_builder_api::dto::InterviewAnswersRequest,
) -> Result<persona_builder_api::dto::InterviewStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::save_interview_answers(request)
}

#[tauri::command(async)]
pub fn keep_interview_answers(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::InterviewKept, persona_builder_api::dto::Failure> {
    persona_builder_api::keep_interview_answers(request)
}

#[tauri::command(async)]
pub fn show_report(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ReportView, persona_builder_api::dto::Failure> {
    persona_builder_api::show_report(request)
}

#[tauri::command(async)]
pub fn review_status(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::ReviewStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::review_status(request)
}

#[tauri::command(async)]
pub fn keep_answer(
    request: persona_builder_api::dto::KeepAnswerRequest,
) -> Result<persona_builder_api::dto::ReviewStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::keep_answer(request)
}

#[tauri::command(async)]
pub fn draft_refresh(
    request: persona_builder_api::dto::SlugRequest,
) -> Result<persona_builder_api::dto::RefreshDrafted, persona_builder_api::dto::Failure> {
    persona_builder_api::draft_refresh(request)
}

#[tauri::command(async)]
pub fn answer_refresh(
    request: persona_builder_api::dto::RefreshAnswerRequest,
) -> Result<persona_builder_api::dto::ReviewStatus, persona_builder_api::dto::Failure> {
    persona_builder_api::answer_refresh(request)
}

#[tauri::command(async)]
pub fn apply_refresh(
    request: persona_builder_api::dto::RefreshApplyRequest,
    on_progress: tauri::ipc::Channel<persona_builder_api::dto::BuildProgress>,
) -> Result<persona_builder_api::dto::RefreshApplied, persona_builder_api::dto::Failure> {
    persona_builder_api::apply_refresh(request, &mut |p| {
        let _ = on_progress.send(p);
    })
}
