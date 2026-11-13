#[allow(dead_code)]
mod common;
use common::Scratch;
use std::fs;
use voice_save::{
    computer::valid_computer_name,
    record::{Role, SaveRecord, read_record, write_record},
};
#[test]
fn member_records_and_strict_keys() {
    let home = Scratch::new("open-record");
    for builder in [None, Some("Studio".into())] {
        let record = SaveRecord::Github {
            owner: "rowan".into(),
            name: "wren-assistant".into(),
            branch: "main".into(),
            role: Role::Member,
            builder,
        };
        write_record(&home.0, &record).unwrap();
        assert_eq!(read_record(&home.0).unwrap(), Some(record));
    }
    for extra in [
        r#", "role":"builder""#,
        r#", "builder":"Studio""#,
        r#", "role":"member", "builder":"bad@name""#,
        r#", "role":"member", "builder":" Studio""#,
        r#", "role":"member", "builder":"Studio ""#,
        r#", "other":true"#,
    ] {
        fs::write(home.0.join("save.json"),format!(r#"{{"version":1,"mode":"github","owner":"rowan","name":"wren-assistant","branch":"main"{extra}}}"#)).unwrap();
        assert!(read_record(&home.0).is_err());
    }
    let record = SaveRecord::Github {
        owner: "rowan".into(),
        name: "wren-assistant".into(),
        branch: "main".into(),
        role: Role::Builder,
        builder: None,
    };
    write_record(&home.0, &record).unwrap();
    assert_eq!(fs::read(home.0.join("save.json")).unwrap(), b"{\n  \"version\": 1,\n  \"mode\": \"github\",\n  \"owner\": \"rowan\",\n  \"name\": \"wren-assistant\",\n  \"branch\": \"main\"\n}\n");
}
#[test]
fn computer_names_have_safe_trailer_text() {
    for name in ["Studio", "Wren’s computer"] {
        assert!(valid_computer_name(name));
    }
    for name in [
        " Studio", "Studio ", "", " ", "a\nb", "a@b", "a\\b", "a$b", "a`b", "Studio\n", "\tStudio",
    ] {
        assert!(!valid_computer_name(name));
    }
    assert!(!valid_computer_name(&"a".repeat(65)));
}

#[test]
fn opening_sentences_are_recognized() {
    use voice_save::text;
    for sentence in [
        text::member_computer("Wren", Some("Studio")),
        text::member_computer("Wren", None),
        text::fetch_failed("git fetch failed"),
        text::saved_copy_refused("rowan/wren-assistant", "link"),
        text::saved_copy_private("rowan/wren-assistant", "identity.md:1"),
        text::saved_copy_large("rowan/wren-assistant"),
        text::state_in_use("wren"),
    ] {
        assert!(text::is_said(&sentence), "{sentence}");
    }
    assert!(!text::is_said("raw invented-token error"));
}
