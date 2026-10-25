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
            commands::save_options,
            commands::open_link,
            commands::github_sign_in_start,
            commands::github_sign_in_finish,
            commands::github_sign_in_cancel,
            commands::github_sign_out,
            commands::save_choose,
            commands::save_push,
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("Persona Builder couldn't start: {error}");
        std::process::exit(1);
    }
}
