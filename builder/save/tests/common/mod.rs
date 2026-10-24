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

/// PB6-only fixtures; existing helpers above remain unchanged.
#[allow(dead_code)] // Integration test crates use different subsets of these fixtures.
pub mod pb6 {
    use super::*;
    use persona_builder::crew::{NewMember, new_member};
    use std::path::Path;
    use std::process::{Command, Output};
    use std::sync::OnceLock;

    fn executable(name: &str) -> PathBuf {
        // The dispatch explicitly requires resolving the real git with command -v.
        let output = Command::new("/bin/sh")
            .args(["-c", &format!("command -v {name}")])
            .output()
            .unwrap();
        assert!(output.status.success());
        PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
    }
    pub fn real_git() -> &'static Path {
        static GIT: OnceLock<PathBuf> = OnceLock::new();
        GIT.get_or_init(|| executable("git"))
    }
    fn real_python() -> &'static Path {
        static PYTHON: OnceLock<PathBuf> = OnceLock::new();
        PYTHON.get_or_init(|| {
            let output = Command::new(executable("python3"))
                .args(["-c", "import sys; print(sys.executable)"])
                .output()
                .unwrap();
            assert!(output.status.success());
            PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
        })
    }
    pub fn git_output(dir: &Path, args: &[&str]) -> Output {
        Command::new(real_git())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap()
    }
    pub fn git(dir: &Path, args: &[&str]) -> String {
        let out = git_output(dir, args);
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim_end().into()
    }
    pub fn init(dir: &Path, bare: bool) {
        fs::create_dir_all(dir).unwrap();
        if bare {
            git(dir, &["init", "-q", "--bare", "--initial-branch=main"]);
        } else {
            git(dir, &["init", "-q", "--initial-branch=main"]);
        }
        git(dir, &["config", "user.name", "Rowan Vale"]);
        git(dir, &["config", "user.email", "rowan@example.invalid"]);
    }
    pub fn seed(dir: &Path) {
        fs::write(dir.join("keep.md"), "keep\n").unwrap();
        git(dir, &["add", "keep.md"]);
        git(dir, &["commit", "-qm", "Initial"]);
    }
    pub fn file_url(dir: &Path) -> String {
        format!("file://{}", dir.canonicalize().unwrap().display())
    }

    pub struct Wrapper {
        pub program: PathBuf,
        pub log: PathBuf,
    }
    impl Wrapper {
        pub fn new(dir: &Path, fail_push: bool) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let program = dir.join("git-wrapper");
            let log = dir.join("git-log.jsonl");
            let python = real_python();
            let source = format!(
                r##"#!{} -I
import os, sys, json, base64
# macOS CoreFoundation adds this inside the wrapper's own Python process; our code never passes it.
env_names = sorted(name for name in os.environ if name != '__CF_USER_TEXT_ENCODING')
with open({}, 'a', encoding='utf-8') as log:
    log.write(json.dumps({{'argv': sys.argv[1:], 'env': env_names}}) + '\n')
if {} and 'push' in sys.argv[1:]:
    header = os.environ.get('GIT_CONFIG_VALUE_0', '')
    credential = header.rsplit(' ', 1)[-1]
    token = base64.b64decode(credential).decode().split(':', 1)[1]
    sys.stderr.write('old detail\n\nfatal: ' + token + ' ' + credential + '\n\n')
    sys.exit(1)
if 'GIT_INDEX_FILE' in os.environ:
    scratch = os.path.dirname(os.environ['GIT_INDEX_FILE'])
    assert os.stat(scratch).st_mode & 0o777 == 0o700
    witness = os.path.join(os.path.dirname(__file__), 'index-witness')
    if not os.path.lexists(witness):
        os.symlink(scratch, witness)
os.execv({}, [{}, *sys.argv[1:]])
"##,
                python.display(),
                serde_json::to_string(&log).unwrap(),
                if fail_push { "True" } else { "False" },
                serde_json::to_string(real_git()).unwrap(),
                serde_json::to_string(real_git()).unwrap()
            );
            fs::write(&program, source).unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
            Self { program, log }
        }
        pub fn calls(&self) -> Vec<serde_json::Value> {
            match fs::read_to_string(&self.log) {
                Ok(log) => log
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
                Err(error) => panic!("read wrapper log: {error}"),
            }
        }
        pub fn clear(&self) {
            fs::write(&self.log, "").unwrap();
        }
    }
    pub fn member(dir: &Path) -> persona_builder::member::Member {
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template");
        let member = new_member(
            &template,
            dir,
            &NewMember {
                name: "Wren",
                pronouns: "she/her",
                principal: "Rowan Vale",
                principal_pronouns: "they/them",
                repo_shown: "~/ai",
                date: "2026-10-24",
            },
        )
        .unwrap();
        fs::write(dir.join("private.json"), "{}\n").unwrap();
        fs::write(dir.join(".DS_Store"), "ignored\n").unwrap();
        member
    }
}
