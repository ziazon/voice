pub mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::{FakeHttp, reply};
use serde_json::{Value, json};
use voice_save::{
    github::{Endpoints, Github, RepoRef},
    secret::Secret,
    text,
};

fn client(http: &FakeHttp) -> Github<'_> {
    Github::new(http, Endpoints::github(), "Iv1.invented").unwrap()
}
fn token() -> Secret {
    Secret::new("invented-token".into())
}
fn repo(name: &str) -> Value {
    json!({"owner":{"login":"rowan-vale"},"name":name,"private":true,"description":"My AI assistant, saved by Voice"})
}
fn reference() -> RepoRef {
    RepoRef {
        owner: "rowan-vale".into(),
        name: "wren-assistant".into(),
    }
}
fn folders(names: &[&str]) -> Value {
    Value::Array(
        names
            .iter()
            .map(|n| json!({"type":"dir","name":n}))
            .collect(),
    )
}
fn member(slug: &str, name: &str) -> Value {
    json!({"type":"file","encoding":"base64","content":STANDARD.encode(serde_json::to_vec(&json!({"slug":slug,"name":name})).unwrap())})
}
#[test]
fn discovers_private_candidates_and_sorts_with_exact_query() {
    let mut suffix = repo("ari-assistant");
    suffix["description"] = json!(null);
    let mut public = repo("public-assistant");
    public["private"] = json!(false);
    let mut bad = repo("bad-assistant");
    bad["owner"]["login"] = json!("bad/name");
    let mut unrelated = repo("unrelated");
    unrelated["description"] = json!("Other");
    let http = FakeHttp::new(vec![
        reply(
            200,
            json!([
                repo("z-copy"),
                suffix,
                public,
                bad,
                unrelated,
                repo("empty"),
                repo("file")
            ]),
        ),
        reply(200, folders(&["wren"])),
        reply(200, member("wren", "Wren")),
        reply(200, folders(&["ari"])),
        reply(200, member("ari", "Ari")),
        reply(404, json!({})),
        reply(200, json!({"type":"file"})),
    ]);
    let found = client(&http).saved_assistants(&token()).unwrap();
    assert_eq!(
        found
            .iter()
            .map(|a| (a.repo.full(), a.slug.as_str(), a.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("rowan-vale/ari-assistant".into(), "ari", "Ari"),
            ("rowan-vale/z-copy".into(), "wren", "Wren")
        ]
    );
    assert_eq!(
        http.requests.borrow()[0].url,
        "https://api.github.com/user/repos?visibility=private&affiliation=owner,collaborator,organization_member&sort=pushed&per_page=50&page=1"
    );
    assert_eq!(
        http.requests.borrow()[1].url,
        "https://api.github.com/repos/rowan-vale/z-copy/contents/crew?ref=main"
    );
    assert_eq!(
        http.requests.borrow()[2].url,
        "https://api.github.com/repos/rowan-vale/z-copy/contents/crew/wren/member.json?ref=main"
    );
    http.assert_drained();
}
#[test]
fn malformed_entries_skip_only_the_entry_and_decode_newlines() {
    let mut wrong_type = member("wrong-type", "Wren");
    wrong_type["type"] = json!("symlink");
    let mut wrong_encoding = member("wrong-encoding", "Wren");
    wrong_encoding["encoding"] = json!("raw");
    let mut invalid_base64 = member("bad-base64", "Wren");
    invalid_base64["content"] = json!("!!!");
    let mut large = member("too-big", "Wren");
    large["content"] = json!(
        STANDARD.encode(
            serde_json::to_vec(
                &json!({"slug":"too-big","name":"Wren","padding":"a".repeat(65536)})
            )
            .unwrap()
        )
    );
    let mut good = member("wren", "Wren");
    good["content"] = json!(format!("{}\n", good["content"].as_str().unwrap()));
    let http = FakeHttp::new(vec![
        reply(
            200,
            folders(&[
                "../escape",
                "Wren",
                "mismatch",
                "bad-name",
                "wrong-type",
                "wrong-encoding",
                "bad-base64",
                "too-big",
                "wren",
            ]),
        ),
        reply(200, member("elsewhere", "Wren")),
        reply(200, member("bad-name", "bad@name")),
        reply(200, wrong_type),
        reply(200, wrong_encoding),
        reply(200, invalid_base64),
        reply(200, large),
        reply(200, good),
    ]);
    let found = client(&http).saved_in(&token(), &reference()).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].slug, "wren");
    http.assert_drained();
}
#[test]
fn pagination_stops_after_two_pages_and_only_follows_full_first_page() {
    let mut unrelated = repo("other");
    unrelated["description"] = json!(null);
    for count in [0, 49, 50] {
        let mut responses = vec![reply(200, Value::Array(vec![unrelated.clone(); count]))];
        if count == 50 {
            responses.push(reply(200, Value::Array(vec![unrelated.clone(); 50])));
        }
        let http = FakeHttp::new(responses);
        assert!(client(&http).saved_assistants(&token()).unwrap().is_empty());
        assert_eq!(
            http.requests.borrow().len(),
            if count == 50 { 2 } else { 1 }
        );
        if count == 50 {
            assert!(http.requests.borrow()[1].url.ends_with("page=2"));
        }
        http.assert_drained();
    }
}
#[test]
fn contents_are_limited_to_twenty_canonical_directories() {
    let names = (0..25)
        .map(|n| format!("assistant-{n}"))
        .collect::<Vec<_>>();
    let refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let mut responses = vec![reply(200, folders(&refs))];
    responses.extend(names.iter().take(20).map(|n| reply(200, member(n, "Wren"))));
    let http = FakeHttp::new(responses);
    assert_eq!(
        client(&http)
            .saved_in(&token(), &reference())
            .unwrap()
            .len(),
        20
    );
    assert_eq!(http.requests.borrow().len(), 21);
    http.assert_drained();
}
#[test]
fn sign_in_and_repo_list_failures_use_fixed_sentences() {
    for status in [401, 403, 500] {
        let http = FakeHttp::new(vec![reply(status, json!({}))]);
        assert_eq!(
            client(&http)
                .saved_assistants(&token())
                .err()
                .unwrap()
                .to_string(),
            if status == 401 {
                text::SIGN_IN_FIRST
            } else {
                text::GITHUB_UNREACHABLE
            }
        );
    }
    for responses in [
        vec![reply(401, json!({}))],
        vec![reply(200, folders(&["wren"])), reply(401, json!({}))],
    ] {
        let http = FakeHttp::new(responses);
        assert_eq!(
            client(&http)
                .saved_in(&token(), &reference())
                .err()
                .unwrap()
                .to_string(),
            text::SIGN_IN_FIRST
        );
    }
}

#[test]
fn sorts_slugs_within_a_repo_and_ignores_non_directory_entries() {
    let http = FakeHttp::new(vec![
        reply(200, json!([repo("wren-assistant")])),
        reply(
            200,
            json!([{"type":"file","name":"ignored"}, {"type":"dir","name":"wren"}, {"type":"dir","name":"ari"}]),
        ),
        reply(200, member("wren", "Wren")),
        reply(200, member("ari", "Ari")),
    ]);
    let found = client(&http).saved_assistants(&token()).unwrap();
    assert_eq!(
        found.iter().map(|a| a.slug.as_str()).collect::<Vec<_>>(),
        vec!["ari", "wren"]
    );
    http.assert_drained();
}

#[test]
fn malformed_metadata_and_missing_files_do_not_hide_a_valid_assistant() {
    let mut bad_json = member("bad-json", "Wren");
    bad_json["content"] = json!(STANDARD.encode(b"not json"));
    let mut array = member("array", "Wren");
    array["content"] = json!(STANDARD.encode(b"[]"));
    let mut no_name = member("no-name", "Wren");
    no_name["content"] = json!(STANDARD.encode(br#"{"slug":"no-name"}"#));
    let http = FakeHttp::new(vec![
        reply(
            200,
            folders(&["bad-json", "array", "no-name", "missing", "wren"]),
        ),
        reply(200, bad_json),
        reply(200, array),
        reply(200, no_name),
        reply(404, json!({})),
        reply(200, member("wren", "Wren")),
    ]);
    let found = client(&http).saved_in(&token(), &reference()).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Wren");
    http.assert_drained();
}

#[test]
fn rejects_unbounded_list_body_and_invalid_repo_without_request() {
    let http = FakeHttp::new(vec![voice_save::github::HttpResponse {
        status: 200,
        headers: vec![],
        body: vec![b' '; 1_048_577],
    }]);
    assert_eq!(
        client(&http)
            .saved_assistants(&token())
            .unwrap_err()
            .to_string(),
        text::GITHUB_UNREACHABLE
    );
    let http = FakeHttp::new(vec![]);
    let invalid = RepoRef {
        owner: "bad/name".into(),
        name: "wren-assistant".into(),
    };
    assert_eq!(
        client(&http)
            .saved_in(&token(), &invalid)
            .unwrap_err()
            .to_string(),
        text::NOT_ON_GITHUB
    );
    assert!(http.requests.borrow().is_empty());
}
