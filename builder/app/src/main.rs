mod commands;
mod navigation;

fn main() {
    if let Err(error) = tauri::Builder::default()
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
            commands::claude_code_folders,
            commands::read_claude_code,
            commands::choose_claudeai_export,
            commands::claudeai_export_status,
            commands::list_claudeai_chats,
            commands::mark_claudeai_chats,
            commands::read_claudeai,
            commands::merge_histories,
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("Persona Builder couldn't start: {error}");
        std::process::exit(1);
    }
}
