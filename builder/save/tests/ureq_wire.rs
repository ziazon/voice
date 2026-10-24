use persona_builder_save::github::{Http, HttpRequest, HttpResponse, Method, UreqHttp};
use persona_builder_save::{secret::Secret, text};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

struct Server {
    url: String,
    thread: Option<JoinHandle<Vec<u8>>>,
}
impl Server {
    fn new(status: &str, headers: &str, body: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
            body.len()
        );
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "no HTTP request arrived");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            };
            // Accepted sockets inherit the listener's non-blocking flag on macOS.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut raw = Vec::new();
            let header_end = loop {
                let mut byte = [0];
                assert_eq!(
                    stream.read(&mut byte).unwrap(),
                    1,
                    "request ended before headers"
                );
                raw.push(byte[0]);
                if raw.ends_with(b"\r\n\r\n") {
                    break raw.len();
                }
                assert!(raw.len() < 65536, "oversized request headers");
            };
            let headers = String::from_utf8(raw.clone()).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            raw.resize(header_end + content_length, 0);
            stream.read_exact(&mut raw[header_end..]).unwrap();
            // A client enforcing the response limit can close before reading the full body.
            let _ = stream
                .write_all(reply.as_bytes())
                .and_then(|()| stream.write_all(&body));
            drop(stream);
            thread::sleep(Duration::from_millis(100));
            match listener.accept() {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                _ => panic!("a redirect or retry made a second request"),
            }
            raw
        });
        Self {
            url,
            thread: Some(thread),
        }
    }
    fn request(mut self) -> String {
        String::from_utf8(self.thread.take().unwrap().join().unwrap()).unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn request(url: &str) -> HttpRequest<'_> {
    HttpRequest {
        method: Method::Get,
        url: url.into(),
        headers: vec![],
        form: vec![],
        json: None,
        bearer: None,
    }
}
fn header<'a>(raw: &'a str, name: &str) -> Option<&'a str> {
    raw.split("\r\n\r\n")
        .next()
        .unwrap()
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim())
        })
}
fn successful(response: HttpResponse) {
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ok");
}

#[test]
fn bearer_only_in_authorization_header_and_user_agent() {
    let http = UreqHttp::default();
    let server = Server::new("200 OK", "X-Invented: yes\r\n", b"ok".to_vec());
    let token = Secret::new("invented-wire-token".into());
    let mut outgoing = request(&server.url);
    outgoing.bearer = Some(&token);
    let response = http.send(&outgoing).unwrap();
    assert!(
        response
            .headers
            .contains(&("x-invented".into(), "yes".into()))
    );
    successful(response);
    let raw = server.request();
    assert_eq!(
        header(&raw, "Authorization"),
        Some("Bearer invented-wire-token")
    );
    assert_eq!(raw.matches("invented-wire-token").count(), 1);
    assert_eq!(header(&raw, "User-Agent"), Some("persona-builder/0.1.0"));
    assert!(raw.starts_with("GET / HTTP/1.1\r\n"));
    assert!(raw.ends_with("\r\n\r\n"));
}
#[test]
fn form_is_encoded_on_wire() {
    let http = UreqHttp::default();
    let server = Server::new("200 OK", "", b"ok".to_vec());
    let mut outgoing = request(&server.url);
    outgoing.method = Method::Post;
    outgoing.headers = vec![("Accept", "application/json")];
    outgoing.form = vec![
        ("client_id", "Iv1.invented".into()),
        ("scope", "repo read:org".into()),
    ];
    successful(http.send(&outgoing).unwrap());
    let raw = server.request();
    assert!(raw.starts_with("POST / HTTP/1.1\r\n"));
    assert_eq!(
        header(&raw, "Content-Type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(header(&raw, "Accept"), Some("application/json"));
    assert_eq!(header(&raw, "Authorization"), None);
    assert_eq!(
        raw.split_once("\r\n\r\n").unwrap().1,
        "client_id=Iv1.invented&scope=repo+read%3Aorg"
    );
}
#[test]
fn json_is_json_on_wire() {
    let http = UreqHttp::default();
    let server = Server::new("200 OK", "", b"ok".to_vec());
    let mut outgoing = request(&server.url);
    outgoing.method = Method::Post;
    outgoing.json = Some(json!({"name":"wren-assistant","private":true}));
    successful(http.send(&outgoing).unwrap());
    let raw = server.request();
    assert!(raw.starts_with("POST / HTTP/1.1\r\n"));
    assert_eq!(header(&raw, "Content-Type"), Some("application/json"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(raw.split_once("\r\n\r\n").unwrap().1).unwrap(),
        json!({"name":"wren-assistant","private":true})
    );
}
#[test]
fn redirect_is_returned_without_following() {
    let http = UreqHttp::default();
    let server = Server::new("302 Found", "Location: /second\r\n", b"moved".to_vec());
    let response = http.send(&request(&server.url)).unwrap();
    assert_eq!(response.status, 302);
    assert_eq!(response.body, b"moved");
    assert!(
        response
            .headers
            .contains(&("location".into(), "/second".into()))
    );
    assert!(server.request().starts_with("GET / HTTP/1.1\r\n"));
}
#[test]
fn not_found_is_a_response() {
    let http = UreqHttp::default();
    let server = Server::new("404 Not Found", "", b"missing".to_vec());
    let response = http.send(&request(&server.url)).unwrap();
    assert_eq!(response.status, 404);
    assert_eq!(response.body, b"missing");
    server.request();
}
#[test]
fn oversized_body_is_unreachable() {
    let http = UreqHttp::default();
    let server = Server::new("200 OK", "", vec![b'x'; 1_048_577]);
    let error = http
        .send(&request(&server.url))
        .err()
        .expect("body over limit");
    assert_eq!(error.to_string(), text::GITHUB_UNREACHABLE);
    server.request();
}
#[test]
fn closed_port_is_unreachable() {
    let http = UreqHttp::default();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = http.send(&request(&url)).err().expect("closed port");
    assert_eq!(error.to_string(), text::GITHUB_UNREACHABLE);
}
