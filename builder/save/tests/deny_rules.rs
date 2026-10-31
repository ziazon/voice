#[path = "../../core/tests/common/mod.rs"]
mod common;

use common::Scratch;
use persona_builder_save::git::folder_files;
use std::{fs, path::PathBuf};

#[test]
fn saving_refuses_env_and_pem_files_with_the_sentence() {
    for relative in [".env", "keys/site.pem"] {
        let folder = Scratch::new("deny-save");
        let path = folder.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "canary").unwrap();
        assert_eq!(
            folder_files(&folder.0).unwrap_err().to_string(),
            format!(
                "{relative} is a kind of file that holds passwords or keys, so Persona Builder won't save this folder. Move it out of the assistant's folder, then save again."
            )
        );
    }
}

#[test]
fn saving_notes_is_allowed_even_under_a_denied_parent() {
    let home = Scratch::new("deny-save-control");
    let folder = home.0.join(".ssh/assistant");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("notes.md"), "canary").unwrap();
    assert_eq!(
        folder_files(&folder).unwrap(),
        vec![PathBuf::from("notes.md")]
    );
}
