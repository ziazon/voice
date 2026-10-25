use std::path::Path;

#[test]
fn api_has_no_tauri_dependencies() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    for line in manifest.lines() {
        assert!(!line.contains("tauri"), "Tauri dependency in API: {line}");
    }
}
