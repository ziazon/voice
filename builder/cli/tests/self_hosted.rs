use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_core::crew::{Decisions, NewMember, new_member_embedded};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    home: PathBuf,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "cli-self-hosted-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let dir = home.join("ari");
        new_member_embedded(
            &dir,
            &NewMember {
                decisions: Decisions::Python,
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/assistants",
                date: "2026-10-31",
            },
        )
        .unwrap();
        Self { home, dir }
    }
    fn run(&self, command: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_voice"))
            .env("HOME", self.home.join("home"))
            .env("CLAUDE_CONFIG_DIR", self.home.join("home/.claude"))
            .args(["self-hosted", command])
            .arg(&self.dir)
            .args(args)
            .output()
            .unwrap()
    }
    fn state_home(&self) -> PathBuf {
        self.home.join("home")
    }
    fn settings(&self) -> PathBuf {
        self.state_home().join(".claude/state/ari/self-hosted.json")
    }
    fn state(&self) {
        fs::create_dir_all(self.state_home()).unwrap();
        drop(voice_ingest::private::lock_state(&self.state_home(), "ari").unwrap());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

use std::os::unix::fs::PermissionsExt;
use voice_agent::{self_hosted::Server, text};

#[test]
fn show_and_off_use_the_saved_server_without_network_calls() {
    let f = Fixture::new();
    let shown = f.run("show", &[]);
    assert!(
        shown.status.success(),
        "{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    assert_eq!(shown.stdout, format!("{}\n", text::NO_SERVER).as_bytes());
    assert!(!f.state_home().exists());
    f.state();
    let server = Server::new("http://Model-Box.local:11434/", "qwen3:8b", false).unwrap();
    {
        let _lock = voice_ingest::private::lock_existing_state(&f.state_home(), "ari").unwrap();
        server.save(&f.state_home(), "ari").unwrap();
    }
    let shown = f.run("show", &[]);
    assert!(shown.status.success());
    assert_eq!(
        shown.stdout,
        format!("{}\n", text::server(&server)).as_bytes()
    );
    for _ in 0..2 {
        let off = f.run("off", &[]);
        assert!(
            off.status.success(),
            "{}",
            String::from_utf8_lossy(&off.stderr)
        );
        assert_eq!(off.stdout, format!("{}\n", text::NO_SERVER).as_bytes());
        assert!(!f.settings().exists());
    }
}

#[test]
fn set_refuses_a_public_literal_without_saving_anything() {
    let f = Fixture::new();
    let output = f.run(
        "set",
        &[
            "--url",
            "http://8.8.8.8:80",
            "--model",
            "qwen3:8b",
            "--network",
        ],
    );
    assert!(!output.status.success());
    assert_eq!(
        output.stderr,
        format!("voice: {}\n", text::SERVER_NOT_OWN).as_bytes()
    );
    assert!(!f.settings().exists());
    assert!(!f.state_home().exists());
}

#[test]
fn off_requires_an_existing_private_folder() {
    let f = Fixture::new();
    let output = f.run("off", &[]);
    assert!(!output.status.success());
    assert_eq!(
        output.stderr,
        format!("voice: {}\n", text::NO_STATE_FOR_SERVER).as_bytes()
    );
    assert!(!f.state_home().exists());
}

fn model_server(model: &str) -> (String, std::thread::JoinHandle<()>) {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://localhost:{}", listener.local_addr().unwrap().port());
    let body = json!({"data":[{"id":model}]}).to_string();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            assert!(request.len() < 8192);
        }
        assert!(request.starts_with(b"GET /v1/models HTTP/1.1\r\n"));
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    (url, worker)
}

#[test]
fn listener_set_saves_private_normalised_settings_and_show_off_round_trip() {
    for network in [false, true] {
        let f = Fixture::new();
        f.state();
        let (url, worker) = model_server("qwen3:8b");
        let normalised_input = format!("{}/", url.to_uppercase().replacen("HTTP://", "http://", 1));
        let mut args = vec!["--url", &normalised_input, "--model", "qwen3:8b"];
        if network {
            args.push("--network");
        }
        let output = f.run("set", &args);
        worker.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let server = Server::new(&url, "qwen3:8b", network).unwrap();
        assert_eq!(
            output.stdout,
            format!("{}\n", text::server(&server)).as_bytes()
        );
        assert_eq!(
            fs::metadata(f.settings()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let saved: Value = serde_json::from_slice(&fs::read(f.settings()).unwrap()).unwrap();
        assert_eq!(
            saved,
            json!({"url":url,"model":"qwen3:8b","network":network})
        );
        assert_eq!(f.run("show", &[]).stdout, output.stdout);
        assert!(f.run("off", &[]).status.success());
        assert!(!f.settings().exists());
    }
}

#[test]
fn listener_set_does_not_save_a_model_the_server_does_not_list() {
    let f = Fixture::new();
    f.state();
    let (url, worker) = model_server("other");
    let output = f.run("set", &["--url", &url, "--model", "qwen3:8b"]);
    worker.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        output.stderr,
        format!("voice: {}\n", text::no_model("qwen3:8b")).as_bytes()
    );
    assert!(!f.settings().exists());
}

#[test]
fn set_requires_network_opt_in_for_private_literals() {
    let f = Fixture::new();
    let output = f.run(
        "set",
        &["--url", "http://192.168.1.20:1", "--model", "qwen"],
    );
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "voice: Nothing was sent. Your model server's address isn't this computer, and you haven't said to use your network. To use another computer on your own network, set it up again with --network.\n"
    );
    assert!(!f.settings().exists());
    assert!(!f.state_home().exists());
}

#[test]
fn show_prints_exact_computer_scope() {
    let f = Fixture::new();
    f.state();
    let _lock = voice_ingest::private::lock_existing_state(&f.state_home(), "ari").unwrap();
    Server::new("http://localhost:1234", "qwen", false)
        .unwrap()
        .save(&f.state_home(), "ari")
        .unwrap();
    let output = f.run("show", &[]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Model server: http://localhost:1234, model qwen, on this computer. Steps with data you keep on your own machines go to it, and before each step Voice checks the address still leads to this computer.\n"
    );
}

#[test]
fn show_prints_exact_network_scope() {
    let f = Fixture::new();
    f.state();
    let _lock = voice_ingest::private::lock_existing_state(&f.state_home(), "ari").unwrap();
    Server::new("http://localhost:1234", "qwen", true)
        .unwrap()
        .save(&f.state_home(), "ari")
        .unwrap();
    let output = f.run("show", &[]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Model server: http://localhost:1234, model qwen, on your network. Steps with data you keep on your own machines go to it. Before each step Voice checks the address is on a private network, but it can't tell your network from someone else's: on other Wi-Fi, a stranger's computer could answer at the same address. Before you use other Wi-Fi, turn it off with voice self-hosted off.\n"
    );
}
