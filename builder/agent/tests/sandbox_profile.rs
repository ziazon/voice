#![cfg(target_os = "macos")]

mod common;

use common::Scratch;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const PROFILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/sandbox/claude-code.sb");
const SKIP: &str =
    "sandbox_profile: skipped, already inside a sandbox (macOS refuses a sandbox inside one)";
const CANARIES: &[&str] = &[
    ".ssh/x",
    ".aws/x",
    ".gnupg/x",
    ".env",
    ".env.local",
    ".environment",
    ".envrc",
    ".npmrc",
    ".netrc",
    ".pgpass",
    "real.pem",
    "real.key",
    "real.p12",
    "real.keychain",
    "real.keychain-db",
    ".credentials.json",
    ".config/gh/x",
    ".docker/config.json",
    "Library/Keychains/x",
];

fn available() -> bool {
    let output = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .output()
        .expect("run sandbox-exec availability check");
    if output.status.success() {
        return true;
    }
    assert!(
        std::env::var("PERSONA_BUILDER_REQUIRE_SANDBOX").as_deref() != Ok("1"),
        "{SKIP}"
    );
    eprintln!("{SKIP}");
    false
}

struct Sandbox {
    program: Scratch,
    profile: Scratch,
    run: Scratch,
    home: Scratch,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            program: Scratch::new("program"),
            profile: Scratch::new("profile"),
            run: Scratch::new("run"),
            home: Scratch::new("home"),
        }
    }
    fn command(&self, port: u16, sign_in: &str, omit: Option<&str>) -> Command {
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.env_clear().args(["-f", PROFILE]);
        for (name, value) in [
            ("PROGRAM_DIR", self.program.0.display().to_string()),
            ("PROFILE_DIR", self.profile.0.display().to_string()),
            ("RUN_DIR", self.run.0.display().to_string()),
            ("PROXY_PORT", port.to_string()),
            ("SIGN_IN", sign_in.to_owned()),
        ] {
            if omit != Some(name) {
                cmd.args(["-D", &format!("{name}={value}")]);
            }
        }
        cmd.current_dir(&self.run.0);
        cmd
    }
    fn run(&self, program: &str, args: &[&str]) -> Output {
        self.command(1, "no", None)
            .arg(program)
            .args(args)
            .output()
            .unwrap()
    }
    fn cat(&self, path: &Path, allowed: bool) {
        let output = self.run("/bin/cat", &[path.to_str().unwrap()]);
        assert_eq!(
            output.status.success(),
            allowed,
            "cat {}: {output:?}",
            path.display()
        );
        if allowed {
            assert_eq!(output.stdout, b"canary");
        }
    }
    fn touch(&self, path: &Path, allowed: bool) {
        let output = self.run("/usr/bin/touch", &[path.to_str().unwrap()]);
        assert_eq!(
            output.status.success(),
            allowed,
            "touch {}: {output:?}",
            path.display()
        );
        if allowed {
            assert!(path.is_file());
        }
    }
}

fn canary(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "canary").unwrap();
    path
}

#[test]
fn reads_only_the_three_folders() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    for root in [&sb.run.0, &sb.profile.0, &sb.program.0] {
        sb.cat(&canary(root, "allowed"), true);
    }
}

#[test]
fn refuses_the_rest_of_home_and_links_to_it() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    let outside = canary(&sb.home.0, "outside");
    sb.cat(&outside, false);
    let link = sb.run.0.join("link");
    symlink(outside, &link).unwrap();
    sb.cat(&link, false);
}

#[test]
fn writes_only_the_profile_and_run_folders() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    for (root, allowed) in [
        (&sb.run.0, true),
        (&sb.profile.0, true),
        (&sb.program.0, false),
        (&sb.home.0, false),
    ] {
        sb.touch(&root.join("new"), allowed);
    }
}

#[test]
fn denies_every_secret_in_both_letter_cases() {
    if !available() {
        return;
    }
    // Separate roots avoid case-insensitive filesystems reusing the first spelling.
    for upper in [false, true] {
        let sb = Sandbox::new();
        for name in CANARIES {
            let name = if upper {
                name.to_uppercase()
            } else {
                (*name).to_owned()
            };
            let path = canary(&sb.run.0, &name);
            sb.cat(&path, false);
            sb.touch(&path, false);
            if persona_builder::deny::DenyList::builtin()
                .denied_relative(path.parent().unwrap().strip_prefix(&sb.run.0).unwrap())
            {
                sb.touch(
                    &path
                        .parent()
                        .unwrap()
                        .join(if upper { "NEW" } else { "new" }),
                    false,
                );
            }
        }
    }
}

#[test]
fn canaries_match_the_core_deny_list() {
    if !available() {
        return;
    }
    let deny = persona_builder::deny::DenyList::builtin();
    for name in CANARIES {
        for spelling in [(*name).to_owned(), name.to_uppercase()] {
            assert!(deny.denied_relative(Path::new(&spelling)), "{spelling}");
        }
    }
}

#[test]
fn near_misses_stay_readable() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    for name in [
        "monkey.txt",
        "keys.md",
        "a.keyring",
        "docker/config.json",
        ".docker/other.json",
        "envelope.txt",
    ] {
        sb.cat(&canary(&sb.run.0, name), true);
    }
}

#[test]
fn only_the_profiles_own_login_is_allowed() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    for (root, name, allowed) in [
        (&sb.profile.0, ".credentials.json", true),
        (&sb.run.0, ".credentials.json", false),
        (&sb.profile.0, "sub/.credentials.json", false),
    ] {
        let path = canary(root, name);
        sb.cat(&path, allowed);
        sb.touch(&path, allowed);
    }
}

#[test]
fn network_reaches_only_the_proxy_port() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = proxy.local_addr().unwrap().port();
    proxy.set_nonblocking(true).unwrap();
    let responder = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(7);
        loop {
            match proxy.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut request = [0; 4096];
                    assert!(stream.read(&mut request).unwrap() > 0);
                    stream.write_all(b"HTTP/1.0 200 OK\r\n\r\nok").unwrap();
                    return;
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(e) => panic!("proxy did not get a request: {e}"),
            }
        }
    });
    let output = sb
        .command(port, "no", None)
        .args([
            "/usr/bin/curl",
            "-sS",
            "-m5",
            &format!("http://127.0.0.1:{port}/"),
        ])
        .output()
        .unwrap();
    responder.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"ok");
    for host in ["127.0.0.1", "[::1]"] {
        let listener = TcpListener::bind(if host == "127.0.0.1" {
            "127.0.0.1:0"
        } else {
            "[::1]:0"
        })
        .unwrap();
        let other = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let output = sb
            .command(port, "no", None)
            .args([
                "/usr/bin/curl",
                "-sS",
                "-m5",
                &format!("http://{host}:{other}/"),
            ])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn only_sign_in_can_listen() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    for sign_in in ["no", "yes"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let mut child = sb
            .command(1, sign_in, None)
            .args(["/usr/bin/nc", "-l", "127.0.0.1", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        thread::sleep(Duration::from_millis(500));
        let status = child.try_wait().unwrap();
        if status.is_none() {
            child.kill().unwrap();
        }
        let output = child.wait_with_output().unwrap();
        if sign_in == "yes" {
            assert!(status.is_none(), "{output:?}");
        } else {
            assert!(status.is_some_and(|status| !status.success()), "{output:?}");
            assert!(!output.stderr.is_empty());
        }
    }
}

#[test]
fn all_parameters_are_required_before_the_program_runs() {
    if !available() {
        return;
    }
    let sb = Sandbox::new();
    for name in [
        "PROGRAM_DIR",
        "PROFILE_DIR",
        "RUN_DIR",
        "PROXY_PORT",
        "SIGN_IN",
    ] {
        let marker = sb.run.0.join("ran");
        let output = sb
            .command(1, "no", Some(name))
            .arg("/usr/bin/touch")
            .arg(&marker)
            .output()
            .unwrap();
        assert!(!output.status.success(), "missing {name}: {output:?}");
        assert!(!marker.exists(), "program ran without {name}");
    }
}
