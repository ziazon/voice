pub mod common;
use common::Scratch;
use persona_builder_save::record::{SaveRecord, read_record, write_record};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

#[test]
fn each_variant_round_trips_and_local_bytes_are_pinned() {
    let scratch = Scratch::new("record-variants");
    for record in [
        SaveRecord::Local,
        SaveRecord::Existing {
            repo: scratch.0.join("ai"),
            remote: "origin".into(),
            branch: "persona/wren".into(),
        },
        SaveRecord::Github {
            owner: "rowan-vale".into(),
            name: "wren-assistant".into(),
            branch: "main".into(),
        },
    ] {
        write_record(&scratch.0, &record).unwrap();
        assert_eq!(read_record(&scratch.0).unwrap(), Some(record));
    }
    write_record(&scratch.0, &SaveRecord::Local).unwrap();
    assert_eq!(
        fs::read(scratch.0.join("save.json")).unwrap(),
        b"{\n  \"version\": 1,\n  \"mode\": \"local\"\n}\n"
    );
}

#[test]
fn creates_private_state_and_record_without_leftover_temp_file() {
    let scratch = Scratch::new("record-modes");
    let state = scratch.0.join("parents/state");
    assert_eq!(read_record(&state).unwrap(), None);
    write_record(&state, &SaveRecord::Local).unwrap();
    assert_eq!(
        fs::metadata(&state).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(state.join("save.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(fs::read_dir(state).unwrap().count(), 1);
}

#[test]
fn refuses_linked_record_without_reading_or_changing_its_target() {
    let scratch = Scratch::new("record-link");
    let target = scratch.0.join("target");
    fs::write(&target, b"{\"version\":1,\"mode\":\"local\"}").unwrap();
    symlink(&target, scratch.0.join("save.json")).unwrap();
    assert!(read_record(&scratch.0).is_err());
    assert_eq!(
        fs::read(target).unwrap(),
        b"{\"version\":1,\"mode\":\"local\"}"
    );
}

#[test]
fn refuses_linked_state_for_reads_and_writes() {
    let scratch = Scratch::new("state-link");
    let state = scratch.0.join("state");
    let link = scratch.0.join("link");
    fs::create_dir(&state).unwrap();
    symlink(&state, &link).unwrap();
    assert_eq!(
        read_record(&link).unwrap_err().to_string(),
        "state folder is a link"
    );
    assert_eq!(
        write_record(&link, &SaveRecord::Local)
            .unwrap_err()
            .to_string(),
        "state folder is a link"
    );
    assert!(!state.join("save.json").exists());
}

#[test]
fn refuses_bad_versions_unknown_keys_bad_values_and_oversized_records() {
    let scratch = Scratch::new("invalid-record");
    for bytes in [
        r#"{"version":2,"mode":"local"}"#,
        r#"{"mode":"local"}"#,
        r#"{"version":1,"mode":"local","unexpected":true}"#,
        r#"{"version":1,"mode":"elsewhere"}"#,
        r#"{"version":"1","mode":"local"}"#,
        r#"{"version":1,"mode":"github","owner":42,"name":"x","branch":"main"}"#,
        r#"{"version":1,"mode":"existing","repo":"/x","remote":"origin"}"#,
        "not json",
    ] {
        fs::write(scratch.0.join("save.json"), bytes).unwrap();
        assert_eq!(
            read_record(&scratch.0).unwrap_err().to_string(),
            "save.json: not a save record"
        );
    }
    fs::write(
        scratch.0.join("save.json"),
        format!("{{\"version\":1,\"mode\":\"local\"}}{}", " ".repeat(65536)),
    )
    .unwrap();
    assert!(read_record(&scratch.0).is_err());
}

#[test]
fn temporary_file_collision_never_follows_a_link_or_overwrites_a_file() {
    let scratch = Scratch::new("record-temp");
    let target = scratch.0.join("target");
    fs::write(&target, "untouched").unwrap();
    let temporary = scratch
        .0
        .join(format!("save.json.tmp-{}", std::process::id()));
    symlink(&target, &temporary).unwrap();
    assert!(write_record(&scratch.0, &SaveRecord::Local).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "untouched");
    fs::remove_file(&temporary).unwrap();
    fs::write(&temporary, "already exists").unwrap();
    assert!(write_record(&scratch.0, &SaveRecord::Local).is_err());
    assert_eq!(fs::read_to_string(temporary).unwrap(), "already exists");
    assert!(!scratch.0.join("save.json").exists());
}
