pub mod common;

use common::{FakeHttp, reply};
use persona_builder_save::github::{
    Endpoints, Github, HttpResponse, RepoRef, github_repo_of, suggested_repo_name, valid_owner,
    valid_repo_name,
};
use persona_builder_save::{secret::Secret, text};
use serde_json::{Value, json};
use std::time::Duration;

fn client(http: &FakeHttp) -> Github<'_> {
    Github::new(http, Endpoints::github(), "Iv1.invented").unwrap()
}
fn device() -> Value {
    json!({"device_code":"invented-device", "user_code":"ABCD-1234", "verification_uri":"https://github.com/login/device"})
}
fn repo() -> RepoRef {
    RepoRef {
        owner: "rowan-vale".into(),
        name: "wren-assistant".into(),
    }
}
fn token() -> Secret {
    Secret::new("invented-token".into())
}
fn private() -> Value {
    json!({"private":true, "visibility":"private"})
}
fn created() -> Value {
    json!({"owner":{"login":"rowan-vale"}, "name":"wren-assistant"})
}
fn assert_api(request: &common::RecordedRequest) {
    assert_eq!(request.bearer.as_deref(), Some("invented-token"));
    assert!(
        request
            .headers
            .contains(&("Accept", "application/vnd.github+json"))
    );
    assert!(
        request
            .headers
            .contains(&("X-GitHub-Api-Version", "2026-03-10"))
    );
    assert!(request.form.is_empty());
}
fn assert_login(request: &common::RecordedRequest) {
    assert!(request.bearer.is_none());
    assert!(request.headers.contains(&("Accept", "application/json")));
    assert!(request.json.is_none());
}

#[test]
fn start_sign_in_sends_form_and_returns_defaults() {
    let http = FakeHttp::new(vec![reply(200, device())]);
    let code = client(&http).start_sign_in().unwrap();
    assert_eq!(code.user_code, "ABCD-1234");
    assert_eq!(code.verification_uri, "https://github.com/login/device");
    assert_eq!(code.interval, 5);
    assert_eq!(code.expires_in, 900);
    let requests = http.requests.borrow();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://github.com/login/device/code");
    assert_eq!(
        request.form,
        vec![
            ("client_id", "Iv1.invented".into()),
            ("scope", "repo".into())
        ]
    );
    assert_login(request);
    http.assert_drained();
}

#[test]
fn start_sign_in_accepts_explicit_boundaries() {
    for (interval, expires) in [(1, 1), (60, 1800)] {
        let mut body = device();
        body["interval"] = json!(interval);
        body["expires_in"] = json!(expires);
        let http = FakeHttp::new(vec![reply(200, body)]);
        let code = client(&http).start_sign_in().unwrap();
        assert_eq!(code.interval, interval);
        assert_eq!(code.expires_in, expires);
        http.assert_drained();
    }
}

macro_rules! bad_device {
    ($name:ident, $field:literal, $value:expr) => {
        #[test]
        fn $name() {
            let mut body = device();
            body[$field] = $value;
            let http = FakeHttp::new(vec![reply(200, body)]);
            assert_eq!(
                client(&http).start_sign_in().err().unwrap().to_string(),
                text::sign_in_failed("unexpected reply")
            );
            http.assert_drained();
        }
    };
}
#[test]
fn device_code_missing() {
    let mut body = device();
    body.as_object_mut().unwrap().remove("device_code");
    let http = FakeHttp::new(vec![reply(200, body)]);
    assert_eq!(
        client(&http).start_sign_in().err().unwrap().to_string(),
        text::sign_in_failed("unexpected reply")
    );
    http.assert_drained();
}
bad_device!(device_code_empty, "device_code", json!(""));
bad_device!(device_code_too_long, "device_code", json!("a".repeat(257)));
bad_device!(device_code_control, "device_code", json!("a\nb"));
bad_device!(user_code_lowercase, "user_code", json!("abcd"));
bad_device!(user_code_empty, "user_code", json!(""));
bad_device!(user_code_too_long, "user_code", json!("A".repeat(33)));
bad_device!(
    verification_other_host,
    "verification_uri",
    json!("https://example.com/device")
);
bad_device!(
    verification_host_suffix,
    "verification_uri",
    json!("https://github.com.example/device")
);
bad_device!(interval_zero, "interval", json!(0));
bad_device!(interval_61, "interval", json!(61));
bad_device!(interval_wrong_type, "interval", json!("5"));
bad_device!(expires_zero, "expires_in", json!(0));
bad_device!(expires_too_large, "expires_in", json!(1801));

#[test]
fn start_sign_in_refuses_status_and_malformed_json() {
    for response in [
        reply(403, device()),
        reply(200, json!([])),
        HttpResponse {
            status: 200,
            headers: vec![],
            body: b"not json".to_vec(),
        },
    ] {
        let http = FakeHttp::new(vec![response]);
        assert_eq!(
            client(&http).start_sign_in().err().unwrap().to_string(),
            text::sign_in_failed("unexpected reply")
        );
    }
}

fn finish(
    replies: Vec<HttpResponse>,
    expires: u64,
    expected_sleeps: &[u64],
    expected: std::result::Result<&str, String>,
) {
    let mut start = device();
    start["expires_in"] = json!(expires);
    let http = FakeHttp::new(std::iter::once(reply(200, start)).chain(replies).collect());
    let github = client(&http);
    let code = github.start_sign_in().unwrap();
    let mut sleeps = Vec::new();
    let result = github.finish_sign_in(&code, &mut |duration: Duration| {
        sleeps.push(duration.as_secs())
    });
    match expected {
        Ok(value) => assert_eq!(result.unwrap().expose(), value),
        Err(message) => assert_eq!(result.unwrap_err().to_string(), message),
    }
    assert_eq!(sleeps, expected_sleeps);
    let requests = http.requests.borrow();
    assert_eq!(requests.len(), expected_sleeps.len() + 1);
    for request in &requests[1..] {
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://github.com/login/oauth/access_token");
        assert_eq!(
            request.form,
            vec![
                ("client_id", "Iv1.invented".into()),
                ("device_code", "invented-device".into()),
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:device_code".into()
                )
            ]
        );
        assert_login(request);
    }
    http.assert_drained();
}

#[test]
fn pending_pending_token() {
    finish(
        vec![
            reply(200, json!({"error":"authorization_pending"})),
            reply(200, json!({"error":"authorization_pending"})),
            reply(200, json!({"access_token":"invented-token"})),
        ],
        900,
        &[5, 5, 5],
        Ok("invented-token"),
    );
}
#[test]
fn slow_down_default_increment() {
    finish(
        vec![
            reply(200, json!({"error":"slow_down"})),
            reply(200, json!({"access_token":"invented-token"})),
        ],
        900,
        &[5, 10],
        Ok("invented-token"),
    );
}
#[test]
fn slow_down_larger_interval() {
    finish(
        vec![
            reply(200, json!({"error":"slow_down","interval":30})),
            reply(200, json!({"access_token":"invented-token"})),
        ],
        900,
        &[5, 30],
        Ok("invented-token"),
    );
}
#[test]
fn slow_down_smaller_interval_does_not_reduce_delay() {
    finish(
        vec![
            reply(200, json!({"error":"slow_down","interval":6})),
            reply(200, json!({"access_token":"invented-token"})),
        ],
        900,
        &[5, 10],
        Ok("invented-token"),
    );
}
#[test]
fn finish_sign_in_error_texts() {
    for (code, expected) in [
        ("access_denied", text::SIGN_IN_CANCELLED.to_owned()),
        ("expired_token", text::SIGN_IN_EXPIRED.to_owned()),
        (
            "device_flow_disabled",
            text::sign_in_failed("device_flow_disabled"),
        ),
        ("bad CODE", text::sign_in_failed("unexpected reply")),
        ("", text::sign_in_failed("unexpected reply")),
    ] {
        finish(
            vec![reply(200, json!({"error":code}))],
            900,
            &[5],
            Err(expected),
        );
    }
}
#[test]
fn expiry_stops_before_third_sleep_or_request() {
    finish(
        vec![
            reply(200, json!({"error":"authorization_pending"})),
            reply(200, json!({"error":"authorization_pending"})),
        ],
        12,
        &[5, 5],
        Err(text::SIGN_IN_EXPIRED.into()),
    );
}
#[test]
fn finish_refuses_malformed_tokens() {
    for value in [
        "".to_owned(),
        "a b".into(),
        "a\"b".into(),
        "a\nb".into(),
        "é".into(),
        "a".repeat(256),
    ] {
        finish(
            vec![reply(200, json!({"access_token":value}))],
            900,
            &[5],
            Err(text::sign_in_failed("unexpected reply")),
        );
    }
}
#[test]
fn finish_accepts_token_alphabet_and_boundaries() {
    for value in [
        "a".to_owned(),
        "AZaz09._~:+/=-".into(),
        "gho_0123456789abcdefghijklmnopqrstuvwxyz".into(),
        "a".repeat(255),
    ] {
        finish(
            vec![reply(200, json!({"access_token":value}))],
            900,
            &[5],
            Ok(&value),
        );
    }
}
#[test]
fn finish_refuses_bad_status_body_and_error_codes() {
    for response in [
        reply(500, json!({"access_token":"invented-token"})),
        reply(200, json!({})),
        reply(200, json!({"error":"a".repeat(41)})),
        reply(200, json!({"error":42})),
        HttpResponse {
            status: 200,
            headers: vec![],
            body: b"not json".to_vec(),
        },
    ] {
        finish(
            vec![response],
            900,
            &[5],
            Err(text::sign_in_failed("unexpected reply")),
        );
    }
}

#[test]
fn account_with_repo_scope_and_bearer() {
    let mut response = reply(200, json!({"login":"rowan-vale","id":4242}));
    response
        .headers
        .push(("x-oauth-scopes".into(), "repo, read:org".into()));
    let http = FakeHttp::new(vec![response]);
    let account = client(&http).account(&token()).unwrap();
    assert_eq!(account.login, "rowan-vale");
    assert_eq!(account.id, 4242);
    let requests = http.requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].url, "https://api.github.com/user");
    assert!(requests[0].json.is_none());
    assert_api(&requests[0]);
    http.assert_drained();
}
#[test]
fn account_requires_exact_repo_scope() {
    for scope in [
        Some("public_repo"),
        Some("repo:status"),
        Some("repo-extra"),
        Some(""),
        None,
    ] {
        let mut response = reply(200, json!({"login":"rowan-vale","id":4242}));
        if let Some(scope) = scope {
            response
                .headers
                .push(("x-oauth-scopes".into(), scope.into()));
        }
        let http = FakeHttp::new(vec![response]);
        assert_eq!(
            client(&http).account(&token()).err().unwrap().to_string(),
            text::NO_REPO_PERMISSION
        );
    }
}
#[test]
fn account_trims_scope_items() {
    let mut response = reply(200, json!({"login":"rowan-vale","id":4242}));
    response
        .headers
        .push(("x-oauth-scopes".into(), " read:org , repo ".into()));
    let http = FakeHttp::new(vec![response]);
    assert_eq!(client(&http).account(&token()).unwrap().id, 4242);
}
#[test]
fn account_401_requires_sign_in() {
    let http = FakeHttp::new(vec![reply(401, json!({}))]);
    assert_eq!(
        client(&http).account(&token()).err().unwrap().to_string(),
        text::SIGN_IN_FIRST
    );
}
#[test]
fn account_invalid_body_or_status_is_unreachable() {
    for (status, body) in [
        (200, json!({"login":"bad/name","id":4242})),
        (200, json!({"login":"rowan-vale","id":0})),
        (200, json!({"login":"rowan-vale","id":-1})),
        (200, json!({"login":"rowan-vale","id":1.5})),
        (200, json!({})),
        (403, json!({})),
        (500, json!({})),
    ] {
        let mut response = reply(status, body);
        response
            .headers
            .push(("x-oauth-scopes".into(), "repo".into()));
        let http = FakeHttp::new(vec![response]);
        assert_eq!(
            client(&http).account(&token()).err().unwrap().to_string(),
            text::GITHUB_UNREACHABLE
        );
    }
}

#[test]
fn create_private_repo_posts_exact_json_then_confirms() {
    let http = FakeHttp::new(vec![reply(201, created()), reply(200, private())]);
    assert_eq!(
        client(&http)
            .create_private_repo(&token(), "wren-assistant")
            .unwrap(),
        repo()
    );
    let requests = http.requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].url, "https://api.github.com/user/repos");
    assert_eq!(
        requests[0].json,
        Some(
            json!({"name":"wren-assistant","private":true,"description":"My AI assistant, saved by Persona Builder","auto_init":false,"has_issues":false,"has_projects":false,"has_wiki":false})
        )
    );
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].url,
        "https://api.github.com/repos/rowan-vale/wren-assistant"
    );
    assert!(requests[1].json.is_none());
    for request in requests.iter() {
        assert_api(request);
    }
    http.assert_drained();
}
#[test]
fn create_refuses_public_or_internal_readback() {
    for body in [
        json!({"private":false,"visibility":"public"}),
        json!({"private":true,"visibility":"internal"}),
    ] {
        let http = FakeHttp::new(vec![reply(201, created()), reply(200, body)]);
        assert_eq!(
            client(&http)
                .create_private_repo(&token(), "wren-assistant")
                .unwrap_err()
                .to_string(),
            text::repo_public("rowan-vale/wren-assistant")
        );
        http.assert_drained();
    }
}
#[test]
fn create_refuses_unconfirmed_readback() {
    let http = FakeHttp::new(vec![reply(201, created()), reply(404, json!({}))]);
    assert_eq!(
        client(&http)
            .create_private_repo(&token(), "wren-assistant")
            .unwrap_err()
            .to_string(),
        text::repo_unconfirmed("rowan-vale/wren-assistant")
    );
    http.assert_drained();
}
#[test]
fn create_maps_error_statuses() {
    for (status, expected) in [
        (422, text::repo_name_taken("wren-assistant")),
        (401, text::SIGN_IN_FIRST.into()),
        (403, text::NO_REPO_PERMISSION.into()),
        (500, text::GITHUB_UNREACHABLE.into()),
    ] {
        let http = FakeHttp::new(vec![reply(status, json!({}))]);
        assert_eq!(
            client(&http)
                .create_private_repo(&token(), "wren-assistant")
                .unwrap_err()
                .to_string(),
            expected
        );
        http.assert_drained();
    }
}
#[test]
fn create_bad_name_sends_nothing() {
    for name in ["", ".", "..", "x.git", "a/b"] {
        let http = FakeHttp::new(vec![]);
        assert_eq!(
            client(&http)
                .create_private_repo(&token(), name)
                .unwrap_err()
                .to_string(),
            text::BAD_REPO_NAME
        );
        assert!(http.requests.borrow().is_empty());
    }
}
#[test]
fn create_refuses_malformed_repo_identity_without_readback() {
    for body in [
        json!({}),
        json!({"owner":{"login":"bad/name"},"name":"wren-assistant"}),
        json!({"owner":{"login":"rowan-vale"},"name":"x.git"}),
    ] {
        let http = FakeHttp::new(vec![reply(201, body)]);
        assert_eq!(
            client(&http)
                .create_private_repo(&token(), "wren-assistant")
                .unwrap_err()
                .to_string(),
            text::GITHUB_UNREACHABLE
        );
        assert_eq!(http.requests.borrow().len(), 1);
    }
}
#[test]
fn confirm_private_requires_both_private_fields() {
    for body in [
        json!({"private":false,"visibility":"private"}),
        json!({"private":true,"visibility":"public"}),
        json!({"private":true,"visibility":"internal"}),
        json!({"private":true}),
        json!({"visibility":"private"}),
        json!({"private":"true","visibility":"private"}),
        json!({}),
    ] {
        let http = FakeHttp::new(vec![reply(200, body)]);
        assert_eq!(
            client(&http)
                .confirm_private(&token(), &repo())
                .unwrap_err()
                .to_string(),
            text::repo_public("rowan-vale/wren-assistant")
        );
    }
}
#[test]
fn confirm_private_accepts_private() {
    let http = FakeHttp::new(vec![reply(200, private())]);
    client(&http).confirm_private(&token(), &repo()).unwrap();
    let requests = http.requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].url,
        "https://api.github.com/repos/rowan-vale/wren-assistant"
    );
    assert!(requests[0].json.is_none());
    assert_api(&requests[0]);
}
#[test]
fn confirm_private_refuses_every_other_status() {
    for status in [201, 301, 302, 401, 403, 404, 422, 500] {
        let http = FakeHttp::new(vec![reply(status, private())]);
        assert_eq!(
            client(&http)
                .confirm_private(&token(), &repo())
                .unwrap_err()
                .to_string(),
            text::repo_unconfirmed("rowan-vale/wren-assistant")
        );
    }
}

#[test]
fn remote_address_table() {
    for url in [
        "https://github.com/rowan-vale/wren-assistant",
        "https://github.com/rowan-vale/wren-assistant.git",
        "https://github.com/rowan-vale/wren-assistant/",
        "https://github.com/rowan-vale/wren-assistant.git/",
        "https://user@github.com/rowan-vale/wren-assistant",
        "https://user:password@github.com/rowan-vale/wren-assistant.git",
        "https://GITHUB.COM/rowan-vale/wren-assistant",
        "git@github.com:rowan-vale/wren-assistant",
        "git@GITHUB.COM:rowan-vale/wren-assistant.git/",
        "ssh://git@github.com/rowan-vale/wren-assistant.git",
        "ssh://git@GITHUB.COM/rowan-vale/wren-assistant/",
    ] {
        assert_eq!(github_repo_of(url), Some(repo()), "{url}");
    }
    for url in [
        "http://github.com/rowan-vale/wren-assistant",
        "https://github.com.example/rowan-vale/wren-assistant",
        "https://www.github.com/rowan-vale/wren-assistant",
        "https://gist.github.com/rowan-vale/wren-assistant",
        "https://github.com:443/rowan-vale/wren-assistant",
        "https://github.com/rowan-vale/wren-assistant/extra",
        "https://github.com/rowan-vale/wren-assistant?x=1",
        "https://github.com/rowan-vale/wren-assistant#fragment",
        "file:///rowan-vale/wren-assistant",
        "/rowan-vale/wren-assistant",
        "../wren-assistant",
        "git@example.com:rowan-vale/wren-assistant",
        "ssh://git@github.com:22/rowan-vale/wren-assistant",
        "ssh://other@github.com/rowan-vale/wren-assistant",
        "https://github.com/rowan-vale/",
        "https://github.com//wren-assistant",
        "https://github.com/rowan-vale/x.git.git",
        "https://github.com/rowan-vale/..",
        "https://github.com/rowan-vale/wren-assistant//",
        "https://github.com/rowan-vale/wren%20assistant",
    ] {
        assert_eq!(github_repo_of(url), None, "{url}");
    }
}
#[test]
fn repo_name_edges() {
    for name in ["x".to_owned(), "AZaz09._-".into(), "a".repeat(100)] {
        assert!(valid_repo_name(&name));
    }
    for name in [
        "".to_owned(),
        ".".into(),
        "..".into(),
        "x.git".into(),
        "a".repeat(101),
        "a b".into(),
        "a/b".into(),
        "é".into(),
        "a\nb".into(),
    ] {
        assert!(!valid_repo_name(&name));
    }
}
#[test]
fn owner_edges() {
    for owner in ["x".to_owned(), "AZaz09-".into(), "a".repeat(39)] {
        assert!(valid_owner(&owner));
    }
    for owner in [
        "".to_owned(),
        "a".repeat(40),
        "a_b".into(),
        "a.b".into(),
        "a/b".into(),
        "é".into(),
        "a\nb".into(),
    ] {
        assert!(!valid_owner(&owner));
    }
}
#[test]
fn repo_display_and_suggestion() {
    assert_eq!(repo().full(), "rowan-vale/wren-assistant");
    assert_eq!(suggested_repo_name("wren"), "wren-assistant");
}
#[test]
fn endpoints_refuse_unsafe_bases_in_either_position() {
    for base in [
        "http://example.com",
        "https://github.com/path",
        "https://user@github.com",
        "https://github.com:8443",
        "https://github.com?x=1",
        "https://github.com#fragment",
        "http://localhost:4567",
        "http://127.0.0.1:4567/path",
    ] {
        for result in [
            Endpoints::new(base, "https://api.github.com"),
            Endpoints::new("https://github.com", base),
        ] {
            assert_eq!(
                result.err().expect("bad endpoint").to_string(),
                "not a GitHub address"
            );
        }
    }
}
#[test]
fn endpoints_accept_loopback_and_trailing_slash() {
    assert!(Endpoints::new("http://127.0.0.1:4567", "http://127.0.0.1:4567").is_ok());
    assert!(Endpoints::new("https://github.com/", "https://api.github.com/").is_ok());
}
#[test]
fn endpoints_strip_one_trailing_slash() {
    let http = FakeHttp::new(vec![reply(200, device())]);
    let github = Github::new(
        &http,
        Endpoints::new("https://github.com/", "https://api.github.com/").unwrap(),
        "Iv1.invented",
    )
    .unwrap();
    github.start_sign_in().unwrap();
    assert_eq!(
        http.requests.borrow()[0].url,
        "https://github.com/login/device/code"
    );
}
#[test]
fn client_id_validates_alphabet_and_boundaries() {
    let http = FakeHttp::new(vec![]);
    for value in ["a".to_owned(), "AZaz09.".into(), "a".repeat(64)] {
        assert!(Github::new(&http, Endpoints::github(), &value).is_ok());
    }
    for value in [
        "".to_owned(),
        "a".repeat(65),
        "a-b".into(),
        "a b".into(),
        "a\nb".into(),
        "é".into(),
    ] {
        assert_eq!(
            Github::new(&http, Endpoints::github(), &value)
                .err()
                .expect("bad client id")
                .to_string(),
            text::NOT_SET_UP
        );
    }
    assert!(http.requests.borrow().is_empty());
}
