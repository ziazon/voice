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
    assert!(error.to_string().contains("already building"), "{error}");
    let other = Session::open(&home.0, "robin", Cap::default()).unwrap();
    drop(first);
    Session::open(&home.0, "wren", Cap::default()).unwrap();
    drop(other);
}
