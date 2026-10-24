use persona_builder_save::keychain::{Keychain, MemoryStore, TokenStore, storable, store_line};
use persona_builder_save::{secret::Secret, text};

#[test]
fn store_line_has_exact_bytes() {
    let token = Secret::new("invented._~:+/=-".into());
    let line = store_line("persona-builder.test", &token).unwrap();
    assert_eq!(
        line.expose(),
        "add-generic-password -U -s persona-builder.test -a persona-builder -w \"invented._~:+/=-\"\n"
    );
}

#[test]
fn storable_accepts_alphabet_and_length_boundaries() {
    for value in [
        "a".to_owned(),
        "AZaz09._~:+/=-".to_owned(),
        "gho_0123456789abcdefghijklmnopqrstuvwxyz".into(),
        "a".repeat(1024),
    ] {
        assert!(storable(&Secret::new(value)));
    }
}

#[test]
fn storable_refuses_unsafe_values() {
    for value in [
        "".to_owned(),
        "a".repeat(1025),
        "a b".into(),
        "a\"b".into(),
        "a\\b".into(),
        "a\nb".into(),
        "a\rb".into(),
        "a\0b".into(),
        "é".into(),
    ] {
        assert!(!storable(&Secret::new(value)));
    }
}

#[test]
fn store_line_refuses_invalid_tokens() {
    for value in ["", "a b", "a\"b", "a\\b", "a\nb"] {
        let error = store_line("persona-builder.test", &Secret::new(value.into())).unwrap_err();
        assert_eq!(error.to_string(), text::KEYCHAIN_FAILED);
    }
}

#[test]
fn service_accepts_lowercase_alphabet_and_boundaries() {
    for service in [
        "a".to_owned(),
        "persona-builder.test-09".into(),
        "a".repeat(64),
    ] {
        assert!(Keychain::with_service(&service).is_ok());
    }
}

#[test]
fn service_refuses_invalid_values() {
    for service in [
        "".to_owned(),
        "Persona".into(),
        "a b".into(),
        "a".repeat(65),
        "a_b".into(),
        "a\nb".into(),
        "é".into(),
    ] {
        let error = Keychain::with_service(&service)
            .err()
            .expect("invalid service");
        assert_eq!(error.to_string(), text::KEYCHAIN_FAILED);
    }
}

#[test]
fn memory_round_trip_replace_and_forget() {
    let store = MemoryStore::new();
    assert!(store.load().unwrap().is_none());
    store.save(&Secret::new("invented-first".into())).unwrap();
    let first = store.load().unwrap().unwrap();
    assert_eq!(first.expose(), "invented-first");
    store.save(&Secret::new("invented-second".into())).unwrap();
    assert_eq!(first.expose(), "invented-first");
    assert_eq!(store.load().unwrap().unwrap().expose(), "invented-second");
    store.forget().unwrap();
    assert!(store.load().unwrap().is_none());
    store.forget().unwrap();
}

#[test]
fn default_memory_store_starts_empty() {
    assert!(MemoryStore::default().load().unwrap().is_none());
}

#[test]
#[ignore = "writes to the live macOS Keychain; run by hand"]
fn live_keychain_round_trip() {
    let store = Keychain::with_service("persona-builder.test").unwrap();
    store
        .save(&Secret::new("invented-live-test".into()))
        .unwrap();
    assert_eq!(
        store.load().unwrap().unwrap().expose(),
        "invented-live-test"
    );
    store.forget().unwrap();
    assert!(store.load().unwrap().is_none());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn keychain_is_mac_only() {
    let store = Keychain::github();
    assert_eq!(store.load().unwrap_err().to_string(), text::MAC_ONLY);
    assert_eq!(
        store
            .save(&Secret::new("invented".into()))
            .unwrap_err()
            .to_string(),
        text::MAC_ONLY
    );
    assert_eq!(store.forget().unwrap_err().to_string(), text::MAC_ONLY);
}
