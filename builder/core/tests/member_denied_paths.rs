mod common;

use common::{Scratch, repo};
use persona_builder::member::{Member, read_member, write_member};
use serde_json::json;
use std::fs;

#[test]
fn absent_denied_paths_preserves_member_read_and_write() {
    let home = Scratch::new("member-no-denied-paths");
    let original = fs::read(repo().join("crew/_template/member.json")).unwrap();
    let path = home.0.join("member.json");
    fs::write(&path, &original).unwrap();
    let member = read_member(&path).unwrap();
    assert!(member.filters.denied_paths.is_empty());
    write_member(&path, &member).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn private_denied_paths_append_after_member_entries() {
    let home = Scratch::new("member-merged-denied-paths");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(repo().join("crew/_template/member.json")).unwrap())
            .unwrap();
    value["filters"]["denied_paths"] = json!(["diary*", "~/Private/"]);
    let path = home.0.join("member.json");
    fs::write(&path, value.to_string()).unwrap();
    fs::write(
        home.0.join("private.json"),
        json!({"filters":{"denied_paths":["journal*", "~/Personal/"]}}).to_string(),
    )
    .unwrap();
    let member = read_member(&path).unwrap();
    assert_eq!(
        member.filters.denied_paths,
        ["diary*", "~/Private/", "journal*", "~/Personal/"]
    );
}

#[test]
fn non_list_denied_paths_is_refused_in_member_and_private_config() {
    let home = Scratch::new("member-bad-denied-paths");
    let base: serde_json::Value =
        serde_json::from_slice(&fs::read(repo().join("crew/_template/member.json")).unwrap())
            .unwrap();
    for bad in [json!("diary*"), json!([7])] {
        let mut value = base.clone();
        value["filters"]["denied_paths"] = bad.clone();
        assert_eq!(
            Member::from_value(value, "fixture")
                .unwrap_err()
                .to_string(),
            "member config: fixture: filters.denied_paths must be a list of strings"
        );
        let path = home.0.join("member.json");
        fs::write(&path, base.to_string()).unwrap();
        let private = home.0.join("private.json");
        fs::write(
            &private,
            json!({"filters":{"denied_paths":bad}}).to_string(),
        )
        .unwrap();
        assert_eq!(
            read_member(&path).unwrap_err().to_string(),
            format!(
                "member config: {}: filters.denied_paths must be a list of strings",
                private.display()
            )
        );
    }
}
