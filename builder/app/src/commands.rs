#[tauri::command]
pub fn app_info() -> voice_api::dto::AppInfo {
    voice_api::app_info()
}

#[tauri::command(async)]
pub fn list_assistants() -> Result<Vec<voice_api::dto::Assistant>, voice_api::dto::Failure> {
    voice_api::list_assistants()
}

#[tauri::command(async)]
pub fn new_assistant(
    request: voice_api::dto::NewAssistantRequest,
) -> Result<voice_api::dto::Assistant, voice_api::dto::Failure> {
    voice_api::new_assistant(request)
}

#[tauri::command(async)]
pub fn show_assistant_folder(
    request: voice_api::dto::SlugRequest,
) -> Result<(), voice_api::dto::Failure> {
    voice_api::show_assistant_folder(request)
}

#[tauri::command(async)]
pub fn consent_text() -> Result<voice_api::dto::ConsentText, voice_api::dto::Failure> {
    voice_api::consent_text()
}

#[tauri::command(async)]
pub fn agree_consent(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::Consent, voice_api::dto::Failure> {
    voice_api::agree_consent(request)
}

#[tauri::command(async)]
pub fn claude_check() -> Result<voice_api::dto::ClaudeStatus, voice_api::dto::Failure> {
    voice_api::claude_check()
}

#[tauri::command(async)]
pub fn claude_sign_in(
    request: voice_api::dto::ClaudeSignInRequest,
) -> Result<voice_api::dto::ClaudeStatus, voice_api::dto::Failure> {
    voice_api::claude_sign_in(request)
}

#[tauri::command(async)]
pub fn claude_sign_out() -> Result<voice_api::dto::ClaudeStatus, voice_api::dto::Failure> {
    voice_api::claude_sign_out()
}

#[tauri::command(async)]
pub fn claude_sign_in_cancel() -> Result<(), voice_api::dto::Failure> {
    voice_api::claude_sign_in_cancel()
}

#[tauri::command(async)]
pub fn save_options(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::SaveOptions, voice_api::dto::Failure> {
    voice_api::save_options(request)
}

#[tauri::command(async)]
pub fn open_link(request: voice_api::dto::LinkRequest) -> Result<(), voice_api::dto::Failure> {
    voice_api::open_link(request)
}

#[tauri::command(async)]
pub fn github_sign_in_start() -> Result<voice_api::dto::SignInCode, voice_api::dto::Failure> {
    voice_api::github_sign_in_start()
}

#[tauri::command(async)]
pub fn github_sign_in_finish() -> Result<voice_api::dto::Said, voice_api::dto::Failure> {
    voice_api::github_sign_in_finish()
}

#[tauri::command(async)]
pub fn github_sign_in_cancel() -> Result<(), voice_api::dto::Failure> {
    voice_api::github_sign_in_cancel()
}

#[tauri::command(async)]
pub fn github_sign_out() -> Result<voice_api::dto::Said, voice_api::dto::Failure> {
    voice_api::github_sign_out()
}

#[tauri::command(async)]
pub fn save_choose(
    request: voice_api::dto::SaveChoiceRequest,
) -> Result<voice_api::dto::Said, voice_api::dto::Failure> {
    voice_api::save_choose(request)
}

#[tauri::command(async)]
pub fn save_push(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::Said, voice_api::dto::Failure> {
    voice_api::save_push(request)
}

#[tauri::command(async)]
pub fn claude_code_folders(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ClaudeCodeFolders, voice_api::dto::Failure> {
    voice_api::claude_code_folders(request)
}

#[tauri::command(async)]
pub fn read_claude_code(
    request: voice_api::dto::ReadRequest,
    on_progress: tauri::ipc::Channel<voice_api::dto::ReadProgress>,
) -> Result<voice_api::dto::ReadCounts, voice_api::dto::Failure> {
    voice_api::read_claude_code(request, &mut |progress| {
        let _ = on_progress.send(progress);
    })
}

#[tauri::command(async)]
pub fn choose_claudeai_export(
    request: voice_api::dto::ChooseExportRequest,
) -> Result<Option<voice_api::dto::ExportStatus>, voice_api::dto::Failure> {
    let slug = request.slug;
    voice_api::can_read(voice_api::dto::SlugRequest { slug: slug.clone() })?;
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
        Some(path) => voice_api::use_claudeai_export(&slug, &path, request.folder).map(Some),
    }
}

#[tauri::command(async)]
pub fn claudeai_export_status(
    request: voice_api::dto::SlugRequest,
) -> Result<Option<voice_api::dto::ExportStatus>, voice_api::dto::Failure> {
    voice_api::claudeai_export_status(request)
}

#[tauri::command(async)]
pub fn list_claudeai_chats(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ChatList, voice_api::dto::Failure> {
    voice_api::list_claudeai_chats(request)
}

#[tauri::command(async)]
pub fn mark_claudeai_chats(
    request: voice_api::dto::MarkChatsRequest,
) -> Result<voice_api::dto::ChatList, voice_api::dto::Failure> {
    voice_api::mark_claudeai_chats(request)
}

#[tauri::command(async)]
pub fn read_claudeai(
    request: voice_api::dto::SlugRequest,
    on_progress: tauri::ipc::Channel<voice_api::dto::ReadProgress>,
) -> Result<voice_api::dto::ClaudeaiCounts, voice_api::dto::Failure> {
    voice_api::read_claudeai(request, &mut |progress| {
        let _ = on_progress.send(progress);
    })
}

#[tauri::command(async)]
pub fn merge_histories(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::MergeCounts, voice_api::dto::Failure> {
    voice_api::merge_histories(request)
}

#[tauri::command(async)]
pub fn claude_code_status(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ClaudeCodeStatus, voice_api::dto::Failure> {
    voice_api::claude_code_status(request)
}

#[tauri::command(async)]
pub fn turn_on(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::Said, voice_api::dto::Failure> {
    voice_api::turn_on(request)
}

#[tauri::command(async)]
pub fn turn_off(
    request: voice_api::dto::TurnOffRequest,
) -> Result<voice_api::dto::Said, voice_api::dto::Failure> {
    voice_api::turn_off(request)
}

#[tauri::command(async)]
pub fn claude_ai_files(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ClaudeAiFiles, voice_api::dto::Failure> {
    voice_api::claude_ai_files(request)
}

#[tauri::command(async)]
pub fn make_claude_ai_files(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ClaudeAiFiles, voice_api::dto::Failure> {
    voice_api::make_claude_ai_files(request)
}

#[tauri::command(async)]
pub fn show_claude_ai_files(
    request: voice_api::dto::SlugRequest,
) -> Result<(), voice_api::dto::Failure> {
    voice_api::show_claude_ai_files(request)
}

#[tauri::command(async)]
pub fn forget_preview(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ForgetPreview, voice_api::dto::Failure> {
    voice_api::forget_preview(request)
}

#[tauri::command(async)]
pub fn forget(
    request: voice_api::dto::ForgetRequest,
) -> Result<voice_api::dto::Said, voice_api::dto::Failure> {
    voice_api::forget(request)
}

#[tauri::command(async)]
pub fn build_assistant(
    request: voice_api::dto::SlugRequest,
    on_progress: tauri::ipc::Channel<voice_api::dto::BuildProgress>,
) -> Result<voice_api::dto::BuildDone, voice_api::dto::Failure> {
    voice_api::build_assistant(request, &mut |p| {
        let _ = on_progress.send(p);
    })
}

#[tauri::command(async)]
pub fn stop_build(request: voice_api::dto::SlugRequest) -> Result<(), voice_api::dto::Failure> {
    voice_api::stop_build(request)
}

#[tauri::command(async)]
pub fn spend_today(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::Spend, voice_api::dto::Failure> {
    voice_api::spend_today(request)
}

#[tauri::command(async)]
pub fn spending_limit(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::SpendingLimit, voice_api::dto::Failure> {
    voice_api::spending_limit(request)
}

#[tauri::command(async)]
pub fn set_spending_limit(
    request: voice_api::dto::SpendingLimitRequest,
) -> Result<voice_api::dto::SpendingLimit, voice_api::dto::Failure> {
    voice_api::set_spending_limit(request)
}

#[tauri::command(async)]
pub fn interview_status(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::InterviewStatus, voice_api::dto::Failure> {
    voice_api::interview_status(request)
}

#[tauri::command(async)]
pub fn draft_interview(
    request: voice_api::dto::DraftInterviewRequest,
) -> Result<voice_api::dto::InterviewStatus, voice_api::dto::Failure> {
    voice_api::draft_interview(request)
}

#[tauri::command(async)]
pub fn save_interview_answers(
    request: voice_api::dto::InterviewAnswersRequest,
) -> Result<voice_api::dto::InterviewStatus, voice_api::dto::Failure> {
    voice_api::save_interview_answers(request)
}

#[tauri::command(async)]
pub fn keep_interview_answers(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::InterviewKept, voice_api::dto::Failure> {
    voice_api::keep_interview_answers(request)
}

#[tauri::command(async)]
pub fn show_report(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ReportView, voice_api::dto::Failure> {
    voice_api::show_report(request)
}

#[tauri::command(async)]
pub fn review_status(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::ReviewStatus, voice_api::dto::Failure> {
    voice_api::review_status(request)
}

#[tauri::command(async)]
pub fn keep_answer(
    request: voice_api::dto::KeepAnswerRequest,
) -> Result<voice_api::dto::ReviewStatus, voice_api::dto::Failure> {
    voice_api::keep_answer(request)
}

#[tauri::command(async)]
pub fn draft_refresh(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::RefreshDrafted, voice_api::dto::Failure> {
    voice_api::draft_refresh(request)
}

#[tauri::command(async)]
pub fn answer_refresh(
    request: voice_api::dto::RefreshAnswerRequest,
) -> Result<voice_api::dto::ReviewStatus, voice_api::dto::Failure> {
    voice_api::answer_refresh(request)
}

#[tauri::command(async)]
pub fn apply_refresh(
    request: voice_api::dto::RefreshApplyRequest,
    on_progress: tauri::ipc::Channel<voice_api::dto::BuildProgress>,
) -> Result<voice_api::dto::RefreshApplied, voice_api::dto::Failure> {
    voice_api::apply_refresh(request, &mut |p| {
        let _ = on_progress.send(p);
    })
}

#[tauri::command(async)]
pub fn convene_panel(
    request: voice_api::dto::ConvenePanelRequest,
    on_progress: tauri::ipc::Channel<voice_api::dto::PanelProgress>,
) -> Result<voice_api::dto::PanelRan, voice_api::dto::Failure> {
    voice_api::convene_panel(request, &mut |p| {
        let _ = on_progress.send(p);
    })
}

#[tauri::command(async)]
pub fn resume_panel(
    request: voice_api::dto::ResumePanelRequest,
    on_progress: tauri::ipc::Channel<voice_api::dto::PanelProgress>,
) -> Result<voice_api::dto::PanelRan, voice_api::dto::Failure> {
    voice_api::resume_panel(request, &mut |p| {
        let _ = on_progress.send(p);
    })
}

#[tauri::command(async)]
pub fn stop_panel(request: voice_api::dto::PanelIdRequest) -> Result<(), voice_api::dto::Failure> {
    voice_api::stop_panel(request)
}

#[tauri::command(async)]
pub fn list_panels() -> Result<Vec<voice_api::dto::PanelSummary>, voice_api::dto::Failure> {
    voice_api::list_panels()
}

#[tauri::command(async)]
pub fn panel(
    request: voice_api::dto::PanelIdRequest,
) -> Result<voice_api::dto::PanelDetail, voice_api::dto::Failure> {
    voice_api::panel(request)
}

#[tauri::command(async)]
pub fn list_skills(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::Skills, voice_api::dto::Failure> {
    voice_api::list_skills(request)
}

#[tauri::command(async)]
pub fn agree_skills(
    request: voice_api::dto::SkillsRequest,
) -> Result<voice_api::dto::Skills, voice_api::dto::Failure> {
    voice_api::agree_skills(request)
}

#[tauri::command(async)]
pub fn withdraw_skills(
    request: voice_api::dto::SkillsRequest,
) -> Result<voice_api::dto::SkillsWithdrawn, voice_api::dto::Failure> {
    voice_api::withdraw_skills(request)
}

#[tauri::command(async)]
pub fn refresh_skills(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::SkillsRefreshed, voice_api::dto::Failure> {
    voice_api::refresh_skills(request)
}

#[tauri::command(async)]
pub fn list_permissions(
    request: voice_api::dto::SlugRequest,
) -> Result<voice_api::dto::Permissions, voice_api::dto::Failure> {
    voice_api::list_permissions(request)
}

#[tauri::command(async)]
pub fn give_permission(
    request: voice_api::dto::GivePermissionRequest,
) -> Result<voice_api::dto::PermissionsChanged, voice_api::dto::Failure> {
    voice_api::give_permission(request)
}

#[tauri::command(async)]
pub fn turn_off_permission(
    request: voice_api::dto::PermissionRequest,
) -> Result<voice_api::dto::PermissionsChanged, voice_api::dto::Failure> {
    voice_api::turn_off_permission(request)
}

#[tauri::command(async)]
pub fn choose_calendar_export(
    request: voice_api::dto::ChooseCalendarRequest,
) -> Result<Option<voice_api::dto::CalendarExport>, voice_api::dto::Failure> {
    let slug = request.slug;
    voice_api::check_calendar_choice(voice_api::dto::SlugRequest { slug: slug.clone() })?;
    let mut dialog = rfd::FileDialog::new().set_title("Choose the calendar you exported");
    if let Some(home) = std::env::var_os("HOME") {
        let downloads = std::path::PathBuf::from(home).join("Downloads");
        if downloads.is_dir() {
            dialog = dialog.set_directory(downloads);
        }
    }
    let chosen = if request.folder {
        dialog.pick_folder()
    } else {
        dialog.add_filter("Calendar", &["ics"]).pick_file()
    };
    match chosen {
        None => Ok(None),
        Some(path) => voice_api::use_calendar_export(&slug, &path).map(Some),
    }
}

#[tauri::command(async)]
pub fn audit_log(
    request: voice_api::dto::AuditRequest,
) -> Result<voice_api::dto::AuditLog, voice_api::dto::Failure> {
    voice_api::audit_log(request)
}

#[tauri::command(async)]
pub fn ask_assistant(
    request: voice_api::dto::AskRequest,
) -> Result<voice_api::dto::AskAnswer, voice_api::dto::Failure> {
    voice_api::ask(request)
}
