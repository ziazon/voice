mod common;
use common::*;
use persona_builder::{
    ceilings::{Ceilings, Source},
    error::Result,
};
use persona_builder_agent::{
    guard::{self, Guard, Sealed},
    ledger::{Cap, Session, local_day},
    provider::{Provider, Reach, Reply},
    self_hosted::{FILE, SelfHosted, Server, own_network},
    text,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[test]
fn own_network_accepts_only_the_specified_ranges() {
    for ip in [
        "127.0.0.1",
        "10.1.2.3",
        "172.16.0.1",
        "172.31.255.255",
        "192.168.1.10",
        "169.254.1.1",
        "::1",
        "fd00::1",
        "fe80::1",
        "::ffff:192.168.1.10",
        "fc00::1",
        "febf::1",
    ] {
        assert!(own_network(ip.parse().unwrap()), "{ip}");
    }
    for ip in [
        "8.8.8.8",
        "172.32.0.1",
        "100.64.0.1",
        "0.0.0.0",
        "255.255.255.255",
        "224.0.0.1",
        "::",
        "2001:db8::1",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
        "fec0::1",
        "172.15.255.255",
        "192.169.0.1",
    ] {
        assert!(!own_network(ip.parse().unwrap()), "{ip}");
    }
}

#[test]
fn server_validates_and_normalises_addresses_and_models() {
    for (url, saved) in [
        ("http://127.0.0.1:11434", "http://127.0.0.1:11434"),
        ("http://localhost:1234/", "http://localhost:1234"),
        ("https://studio.lan:8443", "https://studio.lan:8443"),
        ("http://[::1]:8080", "http://[::1]:8080"),
        (
            "http://Model-Box.local:11434",
            "http://model-box.local:11434",
        ),
    ] {
        assert_eq!(Server::new(url, "qwen3:8b", false).unwrap().url(), saved);
    }
    for url in [
        "ftp://127.0.0.1",
        "http://user:pw@127.0.0.1:1",
        "http://127.0.0.1:11434/v1",
        "http://127.0.0.1:11434?x=1",
        "http://127.0.0.1:11434#x",
        "http://127.0.0.1:0",
        "http://127.0.0.1:65536",
        "http://",
        "http://-box.lan:1",
        "http://box .lan:1",
        "http://box\n.lan:1",
        "http://localhost:+1",
        "http://localhost:",
        "http://localhost//",
        "http://[::1]junk",
        "http://127.1",
        "http://999.1.1.1",
        "http://127.00.0.1",
        "http://box..lan",
        "http://box.lan.",
        "http://mödél.lan",
    ] {
        assert_eq!(
            Server::new(url, "qwen3:8b", false).unwrap_err().to_string(),
            text::BAD_SERVER_URL,
            "{url:?}"
        );
    }
    let long = format!("http://{}", "a".repeat(249));
    assert_eq!(long.len(), 256);
    assert_eq!(
        Server::new(&long, "qwen", false).unwrap_err().to_string(),
        text::BAD_SERVER_URL
    );
    for url in ["http://8.8.8.8:80", "http://[2001:db8::1]:80"] {
        assert_eq!(
            Server::new(url, "qwen", true).unwrap_err().to_string(),
            text::SERVER_NOT_OWN
        );
    }
    for model in [
        "qwen3:8b",
        "library/llama3.2:latest",
        "lmstudio-community/gemma-3-12b@q4_k_m",
    ] {
        assert_eq!(
            Server::new("http://localhost", model, false)
                .unwrap()
                .model(),
            model
        );
    }
    for model in ["", " qwen", "a b", ".hidden", &"a".repeat(129)] {
        assert_eq!(
            Server::new("http://localhost", model, false)
                .unwrap_err()
                .to_string(),
            text::BAD_SERVER_MODEL
        );
    }
}

#[test]
fn saved_server_round_trips_privately_without_creating_missing_state() {
    let home = Scratch::new("server-save");
    assert_eq!(Server::load(&home.0, "wren").unwrap(), None);
    assert!(SelfHosted::load(&home.0, "wren").unwrap().is_none());
    assert!(!home.0.join(".claude").exists());
    let _session = session(&home.0);
    assert_eq!(Server::load(&home.0, "wren").unwrap(), None);
    assert!(!Server::remove(&home.0, "wren").unwrap());
    let server = Server::new("http://Model-Box.local:11434/", "qwen3:8b", false).unwrap();
    server.save(&home.0, "wren").unwrap();
    assert_eq!(Server::load(&home.0, "wren").unwrap(), Some(server.clone()));
    assert_eq!(
        SelfHosted::load(&home.0, "wren").unwrap().unwrap().server(),
        &server
    );
    assert_eq!(
        fs::metadata(state(&home.0).join(FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(Server::remove(&home.0, "wren").unwrap());
    assert_eq!(Server::load(&home.0, "wren").unwrap(), None);
}

#[test]
fn damaged_server_settings_name_the_file() {
    let home = Scratch::new("server-damaged");
    let _session = session(&home.0);
    let path = state(&home.0).join(FILE);
    for raw in [
        r#"{"url":"http://localhost","model":"qwen","network":false,"extra":1}"#,
        "not json",
        r#"{"url":"http://8.8.8.8","model":"qwen","network":true}"#,
    ] {
        persona_builder_ingest::private::write_private(&home.0, "wren", FILE, raw.as_bytes())
            .unwrap();
        assert_eq!(
            Server::load(&home.0, "wren").unwrap_err().to_string(),
            text::server_damaged(&path)
        );
    }
}

#[test]
fn provider_reports_the_server_model_and_claude_keeps_the_asked_model() {
    let server = SelfHosted::new(Server::new("http://localhost", "qwen3:8b", false).unwrap());
    assert_eq!(server.id(), "self-hosted");
    assert_eq!(server.name(), "A model server on your own machines");
    assert_eq!(server.reach(), Reach::OwnMachines);
    assert_eq!(server.model("sonnet"), "qwen3:8b");
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    assert_eq!(stub.claude().model("sonnet"), "sonnet");
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: String,
    body: Value,
}
struct HttpServer {
    url: String,
    requests: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl HttpServer {
    fn new(status: u16, body: String, headers: String, respond: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let (tx, requests) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let (mut socket, _) = match listener.accept() {
                    Ok(found) => found,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("accept: {error}"),
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let end = loop {
                    let mut byte = [0];
                    if socket.read_exact(&mut byte).is_err() {
                        return;
                    }
                    bytes.push(byte[0]);
                    if bytes.ends_with(b"\r\n\r\n") {
                        break bytes.len();
                    }
                };
                let head = String::from_utf8(bytes).unwrap().to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut payload = vec![0; length];
                socket.read_exact(&mut payload).unwrap();
                let mut line = head[..end].split_whitespace();
                tx.send(Request {
                    method: line.next().unwrap().into(),
                    path: line.next().unwrap().into(),
                    headers: head,
                    body: if payload.is_empty() {
                        Value::Null
                    } else {
                        serde_json::from_slice(&payload).unwrap()
                    },
                })
                .unwrap();
                if !respond {
                    while !stopped.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(5));
                    }
                    return;
                }
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes());
            }
        });
        Self {
            url,
            requests,
            stop,
            worker: Some(worker),
        }
    }
    fn replying(status: u16, body: Value) -> Self {
        Self::new(status, body.to_string(), String::new(), true)
    }
    fn silent() -> Self {
        Self::new(200, String::new(), String::new(), false)
    }
    fn provider(&self) -> SelfHosted {
        SelfHosted::new(Server::new(&self.url, "qwen3:8b", false).unwrap())
    }
    fn request(&self) -> Request {
        self.requests.recv_timeout(Duration::from_secs(3)).unwrap()
    }
    fn no_request(&self) {
        assert!(matches!(
            self.requests.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
    }
}
impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn answer() -> Value {
    json!({"choices":[{"message":{"content":"{\"labels\":[\"scope\"]}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":7}})
}
fn ask(server: &SelfHosted, home: &Scratch) -> Result<Reply> {
    Guard::new(vec![server], Ceilings::defaults())
        .call(&session(&home.0), &call("records", &schema()))
}
fn failure(server: &SelfHosted, expected: &str, outcome: &str) {
    let home = Scratch::new("server-failure");
    assert_eq!(ask(server, &home).unwrap_err().to_string(), expected);
    let lines = ledger_lines(&home.0);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["outcome"], outcome);
    assert_eq!(lines[0]["cost_usd"], 0.0);
    assert_eq!(lines[0]["cost_from"], "self-hosted");
}

#[test]
fn listener_call_sends_only_the_brief_and_records_the_server_model() {
    let http = HttpServer::replying(200, answer());
    let home = Scratch::new("server-call");
    let reply = ask(&http.provider(), &home).unwrap();
    assert_eq!(reply.output, json!({"labels":["scope"]}));
    assert_eq!(reply.usage.input_tokens, 12);
    assert_eq!(reply.usage.output_tokens, 7);
    assert_eq!(reply.usage.cache_creation_input_tokens, 0);
    assert_eq!(reply.usage.cache_read_input_tokens, 0);
    assert_eq!(reply.cost_usd, 0.0);
    let request = http.request();
    assert_eq!(request.method, "post");
    assert_eq!(request.path, "/v1/chat/completions");
    assert!(
        request
            .headers
            .contains("content-type: application/json\r\n")
    );
    assert!(request.headers.contains("accept: application/json\r\n"));
    assert_eq!(
        request
            .body
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["model", "messages", "stream", "response_format"])
    );
    assert_eq!(
        request.body,
        json!({"model":"qwen3:8b", "messages":[{"role":"system","content":"Label each record."},{"role":"user","content":"records"}],"stream":false,"response_format":{"type":"json_schema","json_schema":{"name":"reply","strict":true,"schema":schema()}}})
    );
    let lines = ledger_lines(&home.0);
    assert_eq!(lines.len(), 1);
    for (key, value) in [
        ("agent", json!("self-hosted")),
        ("model", json!("qwen3:8b")),
        ("outcome", json!("ok")),
        ("cost_usd", json!(0.0)),
        ("cost_from", json!("self-hosted")),
        ("input_tokens", json!(12)),
        ("output_tokens", json!(7)),
    ] {
        assert_eq!(lines[0][key], value);
    }
    assert!(lines[0].get("plan").is_none());
}

struct Forward<'a>(&'a SelfHosted);
impl Provider for Forward<'_> {
    fn id(&self) -> &'static str {
        "fake"
    }
    fn name(&self) -> &'static str {
        "Fake"
    }
    fn reach(&self) -> Reach {
        Reach::OwnMachines
    }
    fn call(&self, session: &Session, sealed: &Sealed) -> Result<Reply> {
        self.0.call(session, sealed)
    }
}
#[test]
fn listener_seal_is_checked_before_any_request_or_spending() {
    let http = HttpServer::replying(200, answer());
    let server = http.provider();
    let forward = Forward(&server);
    let home = Scratch::new("server-seal");
    let error = Guard::new(vec![&forward], Ceilings::defaults())
        .call(&session(&home.0), &call("records", &schema()))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "this brief wasn't sealed for this provider, so nothing was sent"
    );
    http.no_request();
    assert!(ledger_lines(&home.0).is_empty());
}
#[test]
fn listener_redirect_is_not_followed() {
    let target = HttpServer::replying(200, answer());
    let http = HttpServer::new(
        302,
        String::new(),
        format!("Location: {}/v1/chat/completions\r\n", target.url),
        true,
    );
    failure(&http.provider(), text::SERVER_FAILED, "error");
    http.request();
    target.no_request();
}
#[test]
fn listener_failed_answers_have_the_specified_outcomes() {
    let mut tools = answer();
    tools["choices"][0]["message"]["tool_calls"] = json!([{"id":"tool-1"}]);
    let mut length = answer();
    length["choices"][0]["finish_reason"] = json!("length");
    for (status, body, outcome) in [
        (500, answer().to_string(), "error"),
        (200, "not json".into(), "unreadable"),
        (200, "[]".into(), "unreadable"),
        (200, "{}".into(), "unreadable"),
        (
            200,
            json!({"choices":[{"message":{"content":"not json"}}]}).to_string(),
            "no-output",
        ),
        (
            200,
            json!({"choices":[{"message":{"content":"42"}}]}).to_string(),
            "no-output",
        ),
        (200, tools.to_string(), "error"),
        (200, length.to_string(), "error"),
        (200, "x".repeat(16 * 1024 * 1024 + 1), "unreadable"),
    ] {
        let http = HttpServer::new(status, body, String::new(), true);
        failure(&http.provider(), text::SERVER_FAILED, outcome);
        http.request();
    }
}
#[test]
fn listener_closed_port_and_timeout_have_plain_errors_and_ledger_lines() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    failure(
        &SelfHosted::new(Server::new(&url, "qwen", false).unwrap()),
        text::SERVER_UNREACHABLE,
        "unreachable",
    );
    let http = HttpServer::silent();
    failure(
        &http.provider().with_timeout(Duration::from_secs(1)),
        &text::server_cut_off(1),
        "cut-off",
    );
    http.request();
}
#[test]
fn listener_stop_and_daily_limit_prevent_requests() {
    let http = HttpServer::replying(200, answer());
    let home = Scratch::new("server-stop");
    let stopped = http.provider().with_stop(Arc::new(AtomicBool::new(true)));
    assert_eq!(
        ask(&stopped, &home).unwrap_err().to_string(),
        text::SERVER_STOPPED
    );
    assert!(ledger_lines(&home.0).is_empty());
    let cap_home = Scratch::new("server-cap");
    seed_ledger(
        &cap_home.0,
        &[spent_line(&local_day(fixed_time()), Cap::DEFAULT_DAILY_USD)],
    );
    let session = session(&cap_home.0);
    let error = Guard::new(vec![&http.provider()], Ceilings::defaults())
        .call(&session, &call("records", &schema()))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        text::cap_reached(Cap::DEFAULT_DAILY_USD, Cap::DEFAULT_DAILY_USD)
    );
    assert_eq!(ledger_lines(&cap_home.0).len(), 1);
    http.no_request();
}
#[test]
fn listener_check_requires_a_listed_model_and_a_valid_answer() {
    for (status, body, expected) in [
        (200, json!({"data":[{"id":"qwen3:8b"}]}), None),
        (
            200,
            json!({"data":[{"id":"other"}]}),
            Some(text::no_model("qwen3:8b")),
        ),
        (404, json!({}), Some(text::SERVER_FAILED.into())),
        (200, json!({}), Some(text::SERVER_FAILED.into())),
    ] {
        let http = HttpServer::replying(status, body);
        match expected {
            None => http.provider().check().unwrap(),
            Some(message) => assert_eq!(http.provider().check().unwrap_err().to_string(), message),
        }
        let request = http.request();
        assert_eq!(request.method, "get");
        assert_eq!(request.path, "/v1/models");
    }
}
#[test]
fn listener_provider_order_routes_mail_home_and_history_to_claude() {
    let http = HttpServer::replying(200, answer());
    let server = http.provider();
    let stub = Stub::replying(&ok_reply(&json!({"from":"claude"})), 0);
    let claude = stub.claude();
    let home = Scratch::new("server-routing");
    let session = session(&home.0);
    let guard = Guard::new(
        guard::providers(&claude, Some(&server)),
        Ceilings::defaults(),
    );
    let schema = schema();
    guard
        .call(
            &session,
            &persona_builder_agent::provider::Brief {
                sources: &[Source::Mail],
                ..call("records", &schema)
            },
        )
        .unwrap();
    http.request();
    assert!(!stub.ran());
    let reply = guard.call(&session, &call("records", &schema)).unwrap();
    assert_eq!(reply.output, json!({"from":"claude"}));
    assert!(stub.ran());
    http.no_request();
    let error = Guard::new(guard::providers(&claude, None), Ceilings::defaults())
        .route(&[Source::Mail])
        .err()
        .unwrap();
    assert!(error.to_string().starts_with("Nothing was sent."));
    assert!(error.to_string().ends_with(text::SELF_HOSTED_HINT));
}

#[test]
fn private_literal_requires_network_opt_in() {
    let result = Server::new("http://192.168.1.20:11434", "qwen", false);
    assert!(
        Server::new("http://192.168.1.20:11434", "qwen", true)
            .unwrap()
            .network()
    );
    assert!(
        !Server::new("http://localhost:1234", "qwen", false)
            .unwrap()
            .network()
    );
    assert!(
        result.is_err(),
        "private address accepted without network consent"
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        "Nothing was sent. Your model server's address isn't this computer, and you haven't said to use your network. To use another computer on your own network, set it up again with --network."
    );
}

#[test]
fn saved_settings_without_network_are_damaged() {
    let home = Scratch::new("missing-network");
    let _session = session(&home.0);
    persona_builder_ingest::private::write_private(
        &home.0,
        "wren",
        FILE,
        br#"{"url":"http://localhost","model":"qwen"}"#,
    )
    .unwrap();
    let result = Server::load(&home.0, "wren");
    assert!(
        result.is_err(),
        "settings without network consent were loaded"
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        text::server_damaged(&state(&home.0).join(FILE))
    );
}

#[test]
fn saved_server_round_trips_both_network_scopes() {
    let home = Scratch::new("network-round-trip");
    let _session = session(&home.0);
    for network in [true, false] {
        let server = Server::new("http://localhost:1234", "qwen", network).unwrap();
        assert_eq!(server.network(), network);
        server.save(&home.0, "wren").unwrap();
        let saved: Value =
            serde_json::from_slice(&fs::read(state(&home.0).join(FILE)).unwrap()).unwrap();
        assert_eq!(saved["network"], json!(network));
        assert_eq!(Server::load(&home.0, "wren").unwrap(), Some(server));
    }
}
