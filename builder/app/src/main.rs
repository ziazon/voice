mod commands;
mod navigation;

fn main() {
    if let Err(error) = tauri::Builder::default()
        .plugin(
            tauri::plugin::Builder::<_, ()>::new("nav-guard")
                .on_navigation(|_, url| navigation::is_allowed_navigation(url, tauri::is_dev()))
                .build(),
        )
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
    {
        eprintln!("Persona Builder couldn't start: {error}");
        std::process::exit(1);
    }
}
