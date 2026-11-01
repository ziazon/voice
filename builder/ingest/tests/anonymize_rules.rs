use serde_json::json;
use std::path::Path;
use voice_ingest::anonymize::{Mapping, Rules};

#[test]
fn rules_validate_without_disclosing_private_values() {
    for (value, suffix) in [
        (json!({"unknown":[]}), ".unknown is not a known key"),
        (
            json!({"terms":["x"]}),
            ".terms[0] must be 2 to 200 characters on one line",
        ),
        (
            json!({"terms":[{"term":"Pebble Freight","kind":"Company"}]}),
            ".terms[0].kind must match ^[a-z][a-z-]{0,31}$",
        ),
        (
            json!({"terms":["Pebble Freight"," pebble   freight "]}),
            ".terms[1] repeats anonymizer.terms[0]",
        ),
        (
            json!({"patterns":["(?=private)"]}),
            ".patterns[0] is not a valid regex, or it uses look-around or a backreference, which Voice can't read",
        ),
        (
            json!({"allow":["xy"]}),
            ".allow[0] must be 3 to 200 characters on one line",
        ),
        (
            json!({"never_copy":["family/private"]}),
            ".never_copy[0] must match ^[A-Za-z0-9._*-]{1,100}$",
        ),
        (
            json!({"drop_files":["private/notes"]}),
            ".drop_files[0] must be a file name of 1 to 255 bytes with no slash or control character",
        ),
        (
            json!({"projects":["relative-private"]}),
            ".projects[0] must start with ~/ or /, have no . or .. component, and be at most 1024 bytes",
        ),
    ] {
        let error = Rules::from_value(Some(&value), Path::new("/fictional-home"))
            .err()
            .expect("refused");
        assert_eq!(
            error.to_string(),
            format!("member config: private.json: anonymizer{suffix}")
        );
    }
}

#[test]
fn rules_expand_projects_and_glob_only_stars() {
    let rules = Rules::from_value(
        Some(&json!({"projects":["~/work/pebble"],"never_copy":["family-*","a.b"]})),
        Path::new("/fictional-home"),
    )
    .unwrap();
    assert_eq!(
        rules.projects(),
        &[Path::new("/fictional-home/work/pebble")]
    );
    assert!(rules.never_copy("FAMILY-notes"));
    assert!(!rules.never_copy("axb"));
}

#[test]
fn punctuation_only_terms_are_refused() {
    for term in ["--", "— —"] {
        let result =
            Rules::from_value(Some(&json!({"terms":[term]})), Path::new("/fictional-home"));
        assert_eq!(
            result.err().expect("must refuse").to_string(),
            "member config: private.json: anonymizer.terms[0] must hold a letter or a digit"
        );
    }
}

#[test]
fn combining_mark_only_terms_are_refused() {
    let result = Rules::from_value(
        Some(&json!({"terms":["\u{05B0}\u{05B0}"]})),
        Path::new("/fictional-home"),
    );
    assert_eq!(
        result.err().expect("must refuse").to_string(),
        "member config: private.json: anonymizer.terms[0] must hold a letter or a digit"
    );
}

#[test]
fn mapping_round_trip_keeps_retired_numbers() {
    let mut map = Mapping::default();
    assert_eq!(map.placeholder("company", "Pebble Freight"), "<company-1>");
    let mut map = Mapping::from_json(&map.to_json()).unwrap();
    assert_eq!(map.placeholder("company", "Other Company"), "<company-2>");
    assert_eq!(
        map.placeholder("company", " PEBBLE  Freight "),
        "<company-1>"
    );
    for bytes in [
        b"{}".as_slice(),
        b"{\"version\":2,\"kinds\":{}}",
        b"{\"version\":1,\"kinds\":{\"company\":[42]}}",
    ] {
        assert_eq!(
            Mapping::from_json(bytes).err().unwrap().to_string(),
            "the anonymizer's placeholder file can't be read. Move it away and run anonymize again."
        );
    }
}

#[test]
fn all_entry_shapes_and_limits_are_private_errors() {
    let prefix = "member config: private.json: anonymizer";
    let home = Path::new("/fictional-home");
    for (key, limit) in [
        ("terms", 2000),
        ("patterns", 500),
        ("allow", 500),
        ("never_copy", 500),
        ("drop_files", 100),
        ("projects", 100),
    ] {
        for value in [json!(null), json!("private-canary")] {
            let error = Rules::from_value(Some(&json!({key:value})), home)
                .err()
                .unwrap()
                .to_string();
            assert_eq!(error, format!("{prefix}.{key} must be a list"));
            assert!(!error.contains("private-canary"));
        }
        let value = json!({key:vec!["private-canary";limit+1]});
        assert_eq!(
            Rules::from_value(Some(&value), home)
                .err()
                .unwrap()
                .to_string(),
            format!("{prefix}.{key} must have at most {limit} entries")
        );
        if key != "terms" {
            assert_eq!(
                Rules::from_value(Some(&json!({key:[42]})), home)
                    .err()
                    .unwrap()
                    .to_string(),
                format!("{prefix}.{key}[0] must be a string")
            );
        }
    }
    let cases = vec![
        (json!([]), " must be an object"),
        (
            json!({"terms":[42]}),
            ".terms[0] must be a string or an object with term and kind",
        ),
        (
            json!({"terms":[{"term":"private-canary","kind":"term","extra":true}]}),
            ".terms[0] must contain only term and kind",
        ),
        (
            json!({"terms":[{"kind":"term"}]}),
            ".terms[0].term must be a string",
        ),
        (
            json!({"terms":[{"term":"private-canary"}]}),
            ".terms[0].kind must match ^[a-z][a-z-]{0,31}$",
        ),
        (
            json!({"terms":["a".repeat(201)]}),
            ".terms[0] must be 2 to 200 characters on one line",
        ),
        (
            json!({"terms":["private\ncanary"]}),
            ".terms[0] must be 2 to 200 characters on one line",
        ),
        (
            json!({"terms":[{"term":"private-canary","kind":"a".repeat(33)}]}),
            ".terms[0].kind must match ^[a-z][a-z-]{0,31}$",
        ),
        (
            json!({"patterns":["a".repeat(1001)]}),
            ".patterns[0] must be at most 1000 bytes",
        ),
        (
            json!({"patterns":[r"(private)\1"]}),
            ".patterns[0] is not a valid regex, or it uses look-around or a backreference, which Voice can't read",
        ),
        (
            json!({"allow":["a".repeat(201)]}),
            ".allow[0] must be 3 to 200 characters on one line",
        ),
        (
            json!({"allow":["private\ncanary"]}),
            ".allow[0] must be 3 to 200 characters on one line",
        ),
        (
            json!({"never_copy":[""]}),
            ".never_copy[0] must match ^[A-Za-z0-9._*-]{1,100}$",
        ),
        (
            json!({"never_copy":["a".repeat(101)]}),
            ".never_copy[0] must match ^[A-Za-z0-9._*-]{1,100}$",
        ),
        (
            json!({"drop_files":["a".repeat(256)]}),
            ".drop_files[0] must be a file name of 1 to 255 bytes with no slash or control character",
        ),
        (
            json!({"drop_files":["private\ncanary"]}),
            ".drop_files[0] must be a file name of 1 to 255 bytes with no slash or control character",
        ),
        (
            json!({"drop_files":[""]}),
            ".drop_files[0] must be a file name of 1 to 255 bytes with no slash or control character",
        ),
        (
            json!({"projects":["~/work/../private-canary"]}),
            ".projects[0] must start with ~/ or /, have no . or .. component, and be at most 1024 bytes",
        ),
        (
            json!({"projects":[format!("/{}","a".repeat(1024))]}),
            ".projects[0] must start with ~/ or /, have no . or .. component, and be at most 1024 bytes",
        ),
    ];
    for (value, suffix) in cases {
        let error = Rules::from_value(Some(&value), home)
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, format!("{prefix}{suffix}"));
        assert!(!error.contains("private-canary"));
    }
}

#[path = "../../core/tests/common/mod.rs"]
mod common;
#[test]
fn for_member_reads_private_lists() {
    let scratch = common::Scratch::new("anonymizer-member");
    let path = scratch.0.join("member.json");
    std::fs::write(&path, include_bytes!("../../../crew/_template/member.json")).unwrap();
    std::fs::write(
        scratch.0.join("private.json"),
        br#"{"anonymizer":{"never_copy":["family-*"],"projects":["~/work/pebble"]}}"#,
    )
    .unwrap();
    let member = voice_core::member::read_member(&path).unwrap();
    let rules = Rules::for_member(&member, &scratch.0).unwrap();
    assert!(rules.never_copy("family-notes"));
    assert_eq!(rules.projects(), &[scratch.0.join("work/pebble")]);
}

#[test]
fn mapping_round_trip_accepts_expanded_unicode_lowercase() {
    let term = "İ".repeat(200);
    Rules::from_value(Some(&json!({"terms":[term]})), Path::new("/fictional-home")).unwrap();
    let mut mapping = Mapping::default();
    assert_eq!(mapping.placeholder("term", &term), "<term-1>");
    let mut restored = Mapping::from_json(&mapping.to_json()).unwrap();
    assert_eq!(restored.placeholder("term", &term), "<term-1>");
}

#[test]
fn thirteen_word_term_is_refused_by_word_cap() {
    let value = json!({"terms":["zindle vorpet glimnar frindle plomvek snorbil drempik flontra brelvic quarnet spindal wexnor truvnik"]});
    let error = Rules::from_value(Some(&value), Path::new("/fictional-home"))
        .err()
        .expect("word cap");
    assert_eq!(
        error.to_string(),
        "member config: private.json: anonymizer.terms[0] must have at most 12 words"
    );
}

#[test]
fn two_hundred_three_run_terms_load_within_five_seconds() {
    let terms: Vec<_> = (0..200)
        .map(|n| format!("zindle{n}-vorpet-glimnar"))
        .collect();
    let value = json!({"terms":terms});
    let started = std::time::Instant::now();
    Rules::from_value(Some(&value), Path::new("/fictional-home")).unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "loading took {:?}",
        started.elapsed()
    );
}
