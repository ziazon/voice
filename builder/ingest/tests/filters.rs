use persona_builder::member;
use persona_builder_ingest::filters::{Filters, Kind};
fn config() -> member::Filters {
    member::Filters {
        excluded_folders: "private".into(),
        personal: r"\bmy doctor\b".into(),
        secret: "token".into(),
        pasted_prefixes: vec!["You are ".into()],
        denied_paths: Vec::new(),
        extra: Default::default(),
    }
}
#[test]
fn invalid_patterns_are_refused_for_the_right_key() {
    for key in ["excluded_folders", "personal", "secret"] {
        for pattern in ["(unclosed", "(?=x)", r"(a)\1"] {
            let mut c = config();
            match key {
                "excluded_folders" => c.excluded_folders = pattern.into(),
                "personal" => c.personal = pattern.into(),
                _ => c.secret = pattern.into(),
            }
            let error = Filters::compile(&c).err();
            assert_eq!(
                error.map(|e| e.to_string()),
                Some(format!(
                    "member config: filters.{key} is not a valid regex, or it uses look-around or a backreference, which Persona Builder can't read"
                ))
            );
        }
    }
}
#[test]
fn matches_are_case_insensitive_and_secret_wins() {
    let f = Filters::compile(&config()).unwrap();
    assert_eq!(f.kind("MY DOCTOR"), Some(Kind::Personal));
    assert_eq!(f.kind("my doctor TOKEN"), Some(Kind::Secret));
    assert_eq!(f.kind("ordinary"), None);
    assert!(f.excluded("PRIVATE notes"));
    assert_eq!(f.pasted_prefixes(), &["You are "]);
    assert_eq!(Kind::Personal.as_str(), "personal");
    assert_eq!(Kind::Secret.as_str(), "secret");
}
#[test]
fn lines_count_newlines_and_coalesce_with_secret_winning() {
    let f = Filters::compile(&config()).unwrap();
    assert_eq!(
        f.lines("é my doctor my doctor\nclean\nTOKEN my doctor\nmy doctor\n"),
        vec![(1, Kind::Personal), (3, Kind::Secret), (4, Kind::Personal)]
    );
}
#[test]
fn lines_advance_between_matches_and_coalesce_repeated_matches() {
    let f = Filters::compile(&config()).unwrap();
    for text in [
        "my doctor\nclean\nmy doctor my doctor",
        "token\nclean\ntoken token",
    ] {
        let kind = if text.starts_with("token") {
            Kind::Secret
        } else {
            Kind::Personal
        };
        assert_eq!(f.lines(text), vec![(1, kind), (3, kind)]);
    }
}
