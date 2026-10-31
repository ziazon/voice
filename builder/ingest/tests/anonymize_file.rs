use persona_builder_ingest::anonymize::{self, FileResult, LeftOut, Mapping, Rules};
use serde_json::json;
use std::path::{Path, PathBuf};

fn rules(value: serde_json::Value) -> Rules {
    Rules::from_value(Some(&value), Path::new("/fictional-home")).unwrap()
}
fn kept(text: &str, rules: &Rules) -> String {
    match anonymize::file(
        Path::new("SKILL.md"),
        text.as_bytes(),
        rules,
        &mut Mapping::default(),
    ) {
        FileResult::Kept(text) => text,
        other => panic!("{other:?}"),
    }
}
fn finding(text: &str, kind: &str, line: usize) {
    match anonymize::file(
        Path::new("SKILL.md"),
        text.as_bytes(),
        &Rules::builtin(),
        &mut Mapping::default(),
    ) {
        FileResult::Refused(found) => assert!(
            found.iter().any(|f| f.kind == kind && f.line == line),
            "{found:?}"
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn detectors_and_controls() {
    for (text, kind) in [
        ("rowan@pebble.com", "email"),
        ("212-555-0199", "phone"),
        ("1234567", "long number"),
        ("~rowan/notes", "home folder"),
        ("pebble.internal", "private host"),
        ("192.168.1.2", "private address"),
        ("PEBBLE-42", "ticket id"),
    ] {
        finding(text, kind, 1);
    }
    finding(&format!("ghp_{}", "a".repeat(36)), "secret", 1);
    let controls = "rowan@example.com git@github.com:org/repo.git UTF-8 SHA-256 settings.local.json app.internal.ts 2026-10-31 v1.13.1 localhost:8080 127.0.0.1 123456 password store token: ${{ secrets.X }}";
    assert_eq!(kept(controls, &Rules::builtin()), controls);
}

#[test]
fn terms_are_consistent_longest_first_and_inserted_placeholders_are_protected() {
    let r = rules(
        json!({"terms":["Pebble",{"term":"Pebble Freight","kind":"company"},{"term":"Other Company","kind":"company"},"company"]}),
    );
    assert_eq!(
        kept(
            "Pebble Freight PEBBLE-freight Pebble_Freight Other Company Pebble company",
            &r
        ),
        "<company-1> <company-1> <company-1> <company-2> <term-1> <term-2>"
    );
    assert_eq!(kept("Peb\u{200b}ble Freight", &r), "<company-1>");
    {
        let text = "Pebbles";
        assert!(matches!(
            anonymize::file(
                Path::new("SKILL.md"),
                text.as_bytes(),
                &r,
                &mut Mapping::default()
            ),
            FileResult::Refused(_)
        ));
    }
    assert!(
        matches!(anonymize::file(Path::new("SKILL.md"), b"Pebble\nFreight", &rules(json!({"terms":["Pebble Freight"]})), &mut Mapping::default()), FileResult::Refused(found) if found[0].line == 1)
    );
    assert_eq!(
        kept(
            "rowanberry",
            &rules(json!({"terms":["Rowan"],"allow":["rowanberry"]}))
        ),
        "rowanberry"
    );
}

#[test]
fn source_placeholder_with_private_term_does_not_leak() {
    assert!(
        !kept("<pebble-1>", &rules(json!({"terms":["Pebble"]})))
            .to_lowercase()
            .contains("pebble")
    );
}

#[test]
fn source_placeholder_does_not_hide_long_number() {
    finding("<acct-12345678>", "long number", 1);
}

#[test]
fn source_placeholder_kind_is_replaced_like_other_text() {
    assert!(!kept("<company-1>", &rules(json!({"terms":["company"]}))).contains("company"));
}

#[test]
fn sections_preserve_source_lines_and_validate_markers() {
    assert_eq!(
        kept(
            "before\n<!-- PRIVATE -->\nprivate text\n<!-- /private -->\nafter\n",
            &Rules::builtin()
        ),
        "before\nafter\n"
    );
    finding("<!-- private -->\nopen", "private marker", 1);
    finding("<!-- /private -->", "private marker", 1);
    finding(
        "<!-- private -->\n<!-- private -->\n<!-- /private -->",
        "private marker",
        2,
    );
    finding(
        "<!-- private -->\nsecret text\n<!-- /private -->\nrowan@pebble.com",
        "email",
        4,
    );
}

#[test]
fn homes_and_left_out_reasons() {
    assert_eq!(
        kept(
            "/Users/rowan/x /home/rowan C:\\Users\\rowan\\x /Users/Shared/x",
            &Rules::builtin()
        ),
        "~/x ~ ~\\x /Users/Shared/x"
    );
    let r = rules(json!({"drop_files":["NOTES-*.md"]}));
    for (path, bytes, reason) in [
        ("LOCAL.md", vec![], LeftOut::PrivateFile),
        ("references/LEARNINGS.md", vec![], LeftOut::PrivateFile),
        (".env", vec![], LeftOut::Hidden),
        ("certs/a.pem", vec![], LeftOut::Denied),
        ("notes-private.md", vec![], LeftOut::PrivateFile),
        ("huge.md", vec![b'a'; 2 << 20], LeftOut::TooLarge),
        ("bad.md", vec![255], LeftOut::NotText),
    ] {
        assert!(
            matches!(anonymize::file(Path::new(path), &bytes, &r, &mut Mapping::default()), FileResult::LeftOut(actual) if actual == reason)
        );
    }
}

#[test]
fn report_never_contains_canaries_or_rules() {
    let email = "rowan@pebble.com";
    let phone = "212-555-0199";
    let token = format!("ghp_{}", "a".repeat(36));
    let r = rules(
        json!({"terms":["Pebble Freight"],"patterns":["pebble-[0-9]+"],"allow":["rowanberry"]}),
    );
    let text = format!("{email}\n{phone}\n{token}\nPebble Freight\npebble-42");
    let result = anonymize::skill(
        &[(
            PathBuf::from("SKILL.md"),
            persona_builder_ingest::skills::Source::Bytes(text.into_bytes()),
        )],
        &r,
        &mut Mapping::default(),
    );
    let report = anonymize::report("pebble-notes", &result);
    for canary in [
        email,
        phone,
        &token,
        "Pebble Freight",
        "pebble-[0-9]+",
        "rowanberry",
        "pebble-42",
    ] {
        assert!(!report.contains(canary));
    }
}

#[test]
fn every_secret_family_and_reserved_email_host() {
    for (prefix, suffix) in [
        ("sk-", "a".repeat(20)),
        ("sk_live_", "a".repeat(16)),
        ("sk_test_", "a".repeat(16)),
        ("gho_", "a".repeat(20)),
        ("ghu_", "a".repeat(20)),
        ("ghs_", "a".repeat(20)),
        ("ghr_", "a".repeat(20)),
        ("github_pat_", "a".repeat(20)),
        ("xoxb-", "a".repeat(10)),
        ("xapp-", "a".repeat(10)),
        ("AKIA", "A".repeat(16)),
        ("eyJ", format!("{}.{}", "a".repeat(10), "b".repeat(10))),
        ("AIza", "a".repeat(30)),
        ("hf_", "a".repeat(30)),
        ("glpat-", "a".repeat(20)),
        ("npm_", "a".repeat(30)),
        ("-----BEGIN ", "RSA PRIVATE KEY-----".into()),
        ("password: ", "a".repeat(8)),
        ("https://rowan:", "private-canary@pebble.com".into()),
    ] {
        finding(&format!("{prefix}{suffix}"), "secret", 1);
    }
    for host in [
        "example.org",
        "example.net",
        "pebble.example",
        "pebble.test",
        "pebble.invalid",
        "pebble.localhost",
    ] {
        let text = format!("rowan@{host}");
        assert_eq!(kept(&text, &Rules::builtin()), text);
    }
    for address in [
        "10.1.2.3",
        "172.16.1.2",
        "172.31.1.2",
        "100.64.1.2",
        "100.127.1.2",
        "169.254.1.2",
    ] {
        finding(address, "private address", 1);
    }
}

#[test]
fn every_overlapping_allow_occurrence_is_masked() {
    let r = rules(json!({"allow":["ababa"],"patterns":["ba$"]}));
    assert_eq!(kept("abababa", &r), "abababa");
}

#[test]
fn zero_width_eof_findings_report_the_source_eof_line() {
    let r = rules(json!({"patterns":[r"\z"]}));
    for (text, expected) in [
        ("safe\nsecond\n", 3),
        ("safe\n<!-- private -->\nprivate\n<!-- /private -->\n", 5),
    ] {
        let result = anonymize::file(
            Path::new("SKILL.md"),
            text.as_bytes(),
            &r,
            &mut Mapping::default(),
        );
        assert!(
            matches!(result,FileResult::Refused(found) if found.len()==1 && found[0].line==expected && found[0].kind=="your pattern")
        );
    }
}

#[test]
fn private_host_extension_exception_accepts_unicode_letters_and_digits() {
    for text in ["settings.local.é", "settings.local.٣"] {
        assert_eq!(kept(text, &Rules::builtin()), text);
    }
}
