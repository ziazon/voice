use persona_builder::error::Result;
use persona_builder_save::github::{Http, HttpRequest, HttpResponse, Method};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "persona-builder-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create scratch directory: {error}"),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
pub struct RecordedRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(&'static str, &'static str)>,
    pub form: Vec<(&'static str, String)>,
    pub json: Option<serde_json::Value>,
    pub bearer: Option<String>,
}

pub struct FakeHttp {
    replies: RefCell<VecDeque<HttpResponse>>,
    pub requests: RefCell<Vec<RecordedRequest>>,
}
impl FakeHttp {
    pub fn new(replies: Vec<HttpResponse>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            requests: RefCell::new(Vec::new()),
        }
    }
    pub fn assert_drained(&self) {
        assert!(self.replies.borrow().is_empty(), "unused scripted reply");
    }
}
impl Http for FakeHttp {
    fn send(&self, request: &HttpRequest<'_>) -> Result<HttpResponse> {
        self.requests.borrow_mut().push(RecordedRequest {
            method: match request.method {
                Method::Get => "GET",
                Method::Post => "POST",
            },
            url: request.url.clone(),
            headers: request.headers.clone(),
            form: request.form.clone(),
            json: request.json.clone(),
            bearer: request.bearer.map(|token| token.expose().to_owned()),
        });
        Ok(self
            .replies
            .borrow_mut()
            .pop_front()
            .expect("unexpected HTTP request"))
    }
}
pub fn reply(status: u16, body: serde_json::Value) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![],
        body: serde_json::to_vec(&body).unwrap(),
    }
}
