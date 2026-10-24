pub mod common;
use common::Scratch;
use persona_builder_save::record::read_record;
use serde_json::json;
use std::fs;

macro_rules! rejects {
    ($name:ident, $mode:literal, $field:literal, $value:expr) => {
        #[test]
        fn $name() {
            let scratch = Scratch::new(stringify!($name));
            let mut record = match $mode {
                "existing" => json!({"version": 1, "mode": "existing", "repo": scratch.0,
                    "remote": "origin", "branch": "persona/wren"}),
                _ => json!({"version": 1, "mode": "github", "owner": "example-owner",
                    "name": "assistant", "branch": "main"}),
            };
            record[$field] = json!($value);
            fs::write(scratch.0.join("save.json"), record.to_string()).unwrap();
            let result = read_record(&scratch.0);
            assert!(matches!(&result, Err(error) if error.to_string() == "save.json: not a save record"),
                "expected invalid save record, got {result:?}");
        }
    };
}

rejects!(remote_option, "existing", "remote", "--upload-pack=x");
rejects!(remote_dash, "existing", "remote", "-x");
rejects!(remote_space, "existing", "remote", "a b");
rejects!(remote_empty, "existing", "remote", "");
rejects!(remote_dot_start, "existing", "remote", ".origin");
rejects!(remote_too_long, "existing", "remote", "a".repeat(101));
rejects!(existing_main, "existing", "branch", "main");
rejects!(github_persona, "github", "branch", "persona/x");
rejects!(branch_traversal, "existing", "branch", "persona/../x");
rejects!(owner_slash, "github", "owner", "a/b");
rejects!(name_git_suffix, "github", "name", "x.git");
rejects!(repo_relative, "existing", "repo", "relative/path");

#[test]
fn valid_records_of_each_mode_still_read() {
    let scratch = Scratch::new("valid-record-fields");
    for record in [
        json!({"version": 1, "mode": "local"}),
        json!({"version": 1, "mode": "existing", "repo": scratch.0,
            "remote": "A0._-", "branch": "persona/wren-2"}),
        json!({"version": 1, "mode": "existing", "repo": scratch.0,
            "remote": "a".repeat(100), "branch": "persona/0"}),
        json!({"version": 1, "mode": "github", "owner": "example-owner",
            "name": "assistant", "branch": "main"}),
    ] {
        fs::write(scratch.0.join("save.json"), record.to_string()).unwrap();
        let parsed = read_record(&scratch.0).unwrap().unwrap();
        let mut expected = record;
        expected.as_object_mut().unwrap().remove("version");
        assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
    }
}
