#[path = "../../core/tests/common/mod.rs"]
mod common;

use common::Scratch;
use persona_builder_api::{
    agree_consent_in,
    dto::{NewAssistantRequest, SlugRequest},
    new_assistant_in, use_claudeai_export_in,
};
use std::fs;

#[test]
fn chosen_export_under_ssh_shows_denial_instead_of_export_bad() {
    let home = Scratch::new("deny-api-export");
    new_assistant_in(
        &home.0,
        NewAssistantRequest {
            name: "Pebble".into(),
            pronouns: "they/them".into(),
            principal: "Rowan Vale".into(),
            principal_pronouns: "they/them".into(),
        },
    )
    .unwrap();
    agree_consent_in(
        &home.0,
        SlugRequest {
            slug: "pebble".into(),
        },
    )
    .unwrap();
    fs::create_dir(home.0.join(".ssh")).unwrap();
    let path = home.0.join(".ssh/export.zip");
    fs::write(&path, "canary").unwrap();
    let error = use_claudeai_export_in(&home.0, "pebble", &path).unwrap_err();
    assert_eq!(
        error.message,
        "~/.ssh/export.zip is a kind of file that holds passwords or keys, so Persona Builder never opens it. Choose another file."
    );
}
