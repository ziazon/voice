include!("src/command_names.rs");

fn main() {
    if let Err(error) = tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    ) {
        eprintln!("Persona Builder couldn't build: {error}");
        std::process::exit(1);
    }
}
