//! Alone in its own test binary: it sets environment variables, which no other thread may read meanwhile.
mod common;

use common::*;
use serde_json::json;

#[test]
fn the_session_markers_do_not_reach_claude_and_the_rest_does() {
    // SAFETY: this binary's only test sets these before it starts any thread or child.
    unsafe {
        std::env::set_var("CLAUDECODE", "1");
        std::env::set_var("CLAUDE_CODE_ENTRYPOINT", "cli");
        std::env::set_var("PB_AGENT_TEST_KEEP", "kept");
    }
    let home = Scratch::new("env");
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let session = common::session(&home.0);
    let schema = schema();
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    assert_eq!(stub.file("claudecode").trim(), "unset");
    assert_eq!(stub.file("entrypoint").trim(), "unset");
    assert_eq!(stub.file("keep"), "kept");

    let check = Stub::new(
        "case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":true}';; esac",
    );
    check.claude().check(&home.0).unwrap();
    assert_eq!(check.file("claudecode").trim(), "unset");
    assert_eq!(check.file("entrypoint").trim(), "unset");
}
