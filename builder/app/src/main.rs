mod commands;
mod navigation;

use std::time::Duration;

fn main() {
    let app = tauri::Builder::default()
        .plugin(
            tauri::plugin::Builder::<_, ()>::new("nav-guard")
                .on_navigation(|_, url| navigation::is_allowed_navigation(url, tauri::is_dev()))
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_assistants,
            commands::new_assistant,
            commands::find_existing,
            commands::bring_in_assistant,
            commands::show_assistant_folder,
            commands::consent_text,
            commands::agree_consent,
            commands::claude_check,
            commands::claude_sign_in,
            commands::claude_sign_in_cancel,
            commands::claude_sign_out,
            commands::save_options,
            commands::open_link,
            commands::github_sign_in_start,
            commands::github_sign_in_finish,
            commands::github_sign_in_cancel,
            commands::github_sign_out,
            commands::save_choose,
            commands::save_push,
            commands::saved_on_github,
            commands::open_saved,
            commands::take_newest,
            commands::claude_code_folders,
            commands::read_claude_code,
            commands::choose_claudeai_export,
            commands::claudeai_export_status,
            commands::list_claudeai_chats,
            commands::mark_claudeai_chats,
            commands::read_claudeai,
            commands::merge_histories,
            commands::claude_code_status,
            commands::turn_on,
            commands::turn_off,
            commands::claude_ai_files,
            commands::make_claude_ai_files,
            commands::show_claude_ai_files,
            commands::forget_preview,
            commands::forget,
            commands::build_assistant,
            commands::stop_build,
            commands::spend_today,
            commands::spending_limit,
            commands::set_spending_limit,
            commands::interview_status,
            commands::draft_interview,
            commands::save_interview_answers,
            commands::keep_interview_answers,
            commands::show_report,
            commands::review_status,
            commands::keep_answer,
            commands::draft_refresh,
            commands::answer_refresh,
            commands::apply_refresh,
            commands::convene_panel,
            commands::resume_panel,
            commands::stop_panel,
            commands::list_panels,
            commands::panel,
            commands::list_skills,
            commands::agree_skills,
            commands::withdraw_skills,
            commands::refresh_skills,
            commands::list_permissions,
            commands::give_permission,
            commands::turn_off_permission,
            commands::choose_calendar_export,
            commands::audit_log,
            commands::ask_assistant,
        ])
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                voice_api::claude_sign_in_shutdown(Duration::from_secs(1));
            }
        }),
        Err(error) => {
            eprintln!("Voice couldn't start: {error}");
            std::process::exit(1);
        }
    }
}
