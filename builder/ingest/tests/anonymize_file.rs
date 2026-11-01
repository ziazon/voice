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
        (format!("rowan@{}.{}", "pebble", "co").as_str(), "email"),
        ("212-555-0199", "phone"),
        ("1234567", "long number"),
        ("~rowan/notes", "home folder"),
        (&format!("pebble.{}", "internal"), "private host"),
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
        &format!(
            "<!-- private -->\nsecret text\n<!-- /private -->\nrowan@{}.{}",
            "pebble", "co"
        ),
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
    let email = format!("rowan@{}.{}", "pebble", "co");
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
        email.as_str(),
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
        (
            "https://rowan:",
            format!("private-canary@{}.{}", "pebble", "co"),
        ),
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
fn allow_substrings_without_boundaries_are_not_masked() {
    let r = rules(json!({"allow":["ababa"],"patterns":["ba$"]}));
    assert!(matches!(
        anonymize::file(
            Path::new("SKILL.md"),
            b"abababa",
            &r,
            &mut Mapping::default()
        ),
        FileResult::Refused(_)
    ));
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

fn refused_with(text: &str, config: serde_json::Value, kind: &str, line: usize) {
    let result = anonymize::file(
        Path::new("SKILL.md"),
        text.as_bytes(),
        &rules(config),
        &mut Mapping::default(),
    );
    assert!(
        matches!(&result, FileResult::Refused(found) if found.iter().any(|f| f.kind == kind && f.line == line)),
        "{result:?}"
    );
}

#[test]
fn verifier_01_detectors_and_patterns_before_terms() {
    for (text, term, kind) in [
        (format!("jsmith@{}.{}", "pebble", "co"), "Pebble", "email"),
        (
            format!("rowan@{}.{}", "pebble-mail", "co"),
            "Rowan",
            "email",
        ),
        (format!("pebble.{}", "internal"), "Pebble", "private host"),
        ("pebble/private".into(), "Pebble", "your pattern"),
    ] {
        refused_with(
            &format!("<!-- private -->\nhidden\n<!-- /private -->\n{text}"),
            json!({"terms":[term], "patterns":["pebble/private"]}),
            kind,
            4,
        );
    }
    assert_eq!(
        kept(
            "pebble/private",
            &rules(
                json!({"terms":["Pebble"], "patterns":["pebble/private"], "allow":["pebble/private"]})
            )
        ),
        "<term-1>/private"
    );
}

#[test]
fn verifier_02_compact_and_punctuated_substrings() {
    for (term, text) in [
        ("Rowan Vale", "RowanVale"),
        ("Pebble Freight", "PebbleFreight"),
        ("pebble-io", "mypebble-io.test"),
        ("pebble.test", "mypebble.test.example"),
    ] {
        refused_with(text, json!({"terms":[term]}), "your term", 1);
    }
}

#[test]
fn repeat_verifier_punctuation_rescans() {
    let cases = [
        ("Pebble Freight", "mypebble-freight.com"),
        ("Pebble Freight", "xPebble_Freight"),
        ("Rowan Vale", "github.com/xrowan-vale/x"),
        ("pebble-io", "mypebble_io"),
        ("pebble-io", "mypebbleio"),
        ("Pebble Freight", "ｍｙｐｅｂｂｌｅ－ｆｒｅｉｇｈｔ"),
    ];
    let mut missed = Vec::new();
    for (term, text) in cases {
        let result = anonymize::file(
            Path::new("SKILL.md"),
            format!("Public\n{text}").as_bytes(),
            &rules(json!({"terms":[term]})),
            &mut Mapping::default(),
        );
        if !matches!(result, FileResult::Refused(ref found) if found.iter().any(|f| f.kind == "your term" && f.line == 2))
        {
            missed.push((text, result));
        }
    }
    assert!(missed.is_empty(), "missed term rescans: {missed:?}");
}

#[test]
fn repeat_verifier_sharp_s_rescans_both_ways() {
    for (term, text) in [("Strasse", "Straße 5"), ("Straße", "myStrasse")] {
        refused_with(text, json!({"terms":[term]}), "your term", 1);
    }
}

#[test]
fn short_terms_have_no_unbounded_rescan() {
    assert_eq!(kept("xabx", &rules(json!({"terms":["ab"]}))), "xabx");
    assert_eq!(kept("xabcx", &rules(json!({"terms":["abc"]}))), "xabcx");
}

#[test]
fn repeat_verifier_partial_allow_cannot_hide_term() {
    refused_with(
        "Public\nRowan\nVale",
        json!({"terms":["Rowan Vale"], "allow":["vale"]}),
        "your term",
        2,
    );
}

#[test]
fn repeat_verifier_partial_allow_cannot_hide_pattern() {
    refused_with(
        "Public\npebble-12",
        json!({"patterns":["pebble-[0-9]+"], "allow":["pebble"]}),
        "your pattern",
        2,
    );
}

#[test]
fn repeat_verifier_multiple_allows_cannot_hide_phone() {
    refused_with(
        "212 555 0199",
        json!({"allow":["212", "555 0199"]}),
        "phone",
        1,
    );
}

#[test]
fn whole_allow_and_builtin_masks_still_exempt_hits() {
    let text = "212 555 0199";
    assert_eq!(kept(text, &rules(json!({"allow":[text]}))), text);
    let text = "git@github.com:example/repo";
    assert_eq!(kept(text, &Rules::builtin()), text);
}

#[test]
fn repeat_verifier_partial_allow_fullwidth_offsets() {
    refused_with(
        "é\nｍｙｐｅｂｂｌｅ－ｆｒｅｉｇｈｔ\nUseful",
        json!({"terms":["Pebble Freight"], "allow":["ｆｒｅｉｇｈｔ"]}),
        "your term",
        2,
    );
    let text = "é\nｍｙｐｅｂｂｌｅ－ｆｒｅｉｇｈｔ\nUseful";
    assert_eq!(
        kept(
            text,
            &rules(
                json!({"terms":["Pebble Freight"], "allow":["ｍｙｐｅｂｂｌｅ－ｆｒｅｉｇｈｔ"]})
            )
        ),
        text
    );
}

#[test]
fn verifier_03_paths_are_scanned_and_report_redacts_components() {
    use persona_builder_ingest::skills::Source;
    for path in [
        "references/rowan-vale-contract.md",
        "references/RowanVale.md",
        "references/rowan-vale.LOCAL.md",
    ] {
        let result = anonymize::skill(
            &[
                ("SKILL.md".into(), Source::Bytes(b"Useful".to_vec())),
                (path.into(), Source::Bytes(b"Useful".to_vec())),
            ],
            &rules(json!({"terms":["Rowan Vale"]})),
            &mut Mapping::default(),
        );
        assert!(
            result.refused.iter().any(|f| f.kind == "file name"),
            "{result:?}"
        );
        let report = anonymize::report("code-notes", &result);
        assert!(
            report.contains("code-notes/references/* (file name)"),
            "{report}"
        );
        assert!(!report.to_lowercase().contains("rowan"), "{report}");
    }
}

#[test]
fn verifier_04_accents_and_sharp_s() {
    refused_with(
        "Cafe\u{301} Rowan",
        json!({"terms":["Café Rowan"]}),
        "accent mark",
        1,
    );
    for text in ["STRASSE", "mySTRASSE", "Strasse\nNotes", "StrasseNotes"] {
        refused_with(
            text,
            json!({"terms":["straße", "straße notes"]}),
            "your term",
            1,
        );
    }
}

#[test]
fn verifier_05_home_case() {
    let text = "/users/rowanvale/x /HOME/rowanvale/x /uSeRs/sHaReD/x";
    assert_eq!(kept(text, &Rules::builtin()), "~/x ~/x /uSeRs/sHaReD/x");
}

#[test]
fn verifier_07_case_colliding_kept_paths() {
    use persona_builder_ingest::skills::Source;
    let files = ["SKILL.md", "README.md", "readme.md"]
        .map(|p| (p.into(), Source::Bytes(b"Useful".to_vec())));
    let result = anonymize::skill(&files, &Rules::builtin(), &mut Mapping::default());
    assert!(
        result
            .refused
            .iter()
            .any(|f| f.kind == "same name in another case"),
        "{result:?}"
    );
}

#[test]
fn verifier_08_allow_has_boundaries() {
    refused_with(
        &format!("jsmith@{}.{}", "corp", "com"),
        json!({"allow":["com"]}),
        "email",
        1,
    );
    refused_with(
        "Rowanberry",
        json!({"terms":["Rowan"], "allow":["berry"]}),
        "your term",
        1,
    );
}

#[test]
fn verifier_09_reserved_fixture_policy() {
    // An exempt address stays exempt; a found address is assembled at runtime.
    assert_eq!(
        kept("rowan@example.com", &Rules::builtin()),
        "rowan@example.com"
    );
    finding(&format!("rowan@{}.{}", "pebble", "co"), "email", 1);
    let tests = [
        include_str!("anonymize_file.rs"),
        include_str!("../../cli/tests/anonymize.rs"),
    ]
    .join("\n");
    for forbidden in [
        format!("rowan@{}.{}", "pebble", "com"),
        format!("canary@{}.{}", "pebble-mail", "com"),
    ] {
        assert!(!tests.contains(&forbidden), "non-reserved fixture literal");
    }
}

#[test]
fn verifier_10_many_matches_have_bounded_line_lookup() {
    let text = "~a/ ".repeat((1 << 20) / 4);
    let start = std::time::Instant::now();
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = anonymize::file(
            Path::new("SKILL.md"),
            text.as_bytes(),
            &Rules::builtin(),
            &mut Mapping::default(),
        );
        let _ = send.send(result);
    });
    // Bound a regression too: a failed debug run must not spend minutes in the old quadratic loop.
    let result = receive
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("1 MiB scan exceeded ten seconds");
    assert!(
        start.elapsed() < std::time::Duration::from_secs(10),
        "took {:?}",
        start.elapsed()
    );
    assert!(matches!(result, FileResult::Refused(found) if found.len() == 1 && found[0].line == 1));
}

#[test]
fn verifier_11_term_complexity_is_an_error() {
    let term = (0..30)
        .map(|n| format!("w{n}"))
        .collect::<Vec<_>>()
        .join(" ");
    let result = Rules::from_value(Some(&json!({"terms":[term]})), Path::new("/fictional-home"));
    assert_eq!(
        result.err().expect("must refuse").to_string(),
        "member config: private.json: anonymizer.terms[0] must have at most 12 words"
    );
    let complex = std::iter::repeat_n("letter", 12)
        .collect::<Vec<_>>()
        .join(" ");
    let result = Rules::from_value(
        Some(&json!({"terms":[complex]})),
        Path::new("/fictional-home"),
    );
    assert_eq!(
        result
            .err()
            .expect("oversized term regex must be refused")
            .to_string(),
        "member config: private.json: anonymizer.terms[0] is too complex to search for"
    );
}

#[test]
fn verifier_14_fullwidth_rescan() {
    for (text, term) in [
        ("Ｒｏｗａｎ", "Rowan"),
        ("ＰｅｂｂｌｅＦｒｅｉｇｈｔ", "Pebble Freight"),
        ("ＳＴＲＡＳＳＥ", "straße"),
    ] {
        refused_with(
            &format!("public\n{text}"),
            json!({"terms":[term]}),
            "your term",
            2,
        );
    }
}

#[test]
fn verifier_04_accent_report_explains_how_to_rewrite() {
    let result = anonymize::skill(
        &[(
            "SKILL.md".into(),
            persona_builder_ingest::skills::Source::Bytes("Cafe\u{301} Rowan".as_bytes().to_vec()),
        )],
        &rules(json!({"terms":["Café Rowan"]})),
        &mut Mapping::default(),
    );
    let report = anonymize::report("notes", &result);
    assert!(report.contains("notes/SKILL.md:1 (accent mark)"));
    assert!(
        report.contains("Rewrite this line with single accented letters."),
        "{report}"
    );
    refused_with(
        "Cafe\u{034f} Rowan",
        json!({"terms":["Café Rowan"]}),
        "accent mark",
        1,
    );
}
