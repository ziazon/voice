//! Alone in its own test binary: beside the cap tests, which spawn children on other threads, the lock
//! sometimes still looked held just after the session dropped it (about half of parallel runs).
mod common;

use common::*;
use persona_builder_agent::ledger::{Cap, Session};

#[test]
fn one_session_per_assistant_at_a_time() {
    let home = Scratch::new("cap-lock");
    let first = session(&home.0);
    let error = Session::open(&home.0, "wren", Cap::default())
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("already working on this assistant"),
        "{error}"
    );
    let other = Session::open(&home.0, "robin", Cap::default()).unwrap();
    drop(first);
    Session::open(&home.0, "wren", Cap::default()).unwrap();
    drop(other);
}

#[test]
fn open_existing_creates_nothing_and_shares_the_lock() {
    let home = Scratch::new("existing-lock");
    let error = Session::open_existing(&home.0, "wren", Cap::default())
        .err()
        .expect("a missing state folder must be refused");
    assert!(matches!(
        error,
        persona_builder::error::Error::Io { source, .. }
            if source.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(!home.0.join(".claude/state/wren").exists());

    let first = Session::open(&home.0, "wren", Cap::default()).unwrap();
    let error = Session::open_existing(&home.0, "wren", Cap::default())
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("already working on this assistant"),
        "{error}"
    );
    drop(first);
    let _session = Session::open_existing(&home.0, "wren", Cap::default()).unwrap();
    assert!(home.0.join(".claude/state/wren/agent").exists());
}
