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
            commands::show_assistant_folder,
            commands::consent_text,
            commands::agree_consent,
            commands::claude_check,
            commands::claude_sign_in,
            commands::claude_sign_in_cancel,
            commands::save_options,
            commands::open_link,
            commands::github_sign_in_start,
            commands::github_sign_in_finish,
            commands::github_sign_in_cancel,
            commands::github_sign_out,
            commands::save_choose,
            commands::save_push,
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
        ])
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                persona_builder_api::claude_sign_in_shutdown(Duration::from_secs(1));
            }
        }),
        Err(error) => {
            eprintln!("Persona Builder couldn't start: {error}");
            std::process::exit(1);
        }
    }
}
