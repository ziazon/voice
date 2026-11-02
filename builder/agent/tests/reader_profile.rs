#![cfg(target_os = "macos")]

mod common;

use common::Scratch;
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    net::{TcpListener, UdpSocket},
    os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::Duration,
};
use voice_agent::{
    reader::{MAX_SOURCES, Reader, Source, usable},
    text,
};
use voice_core::{deny::DenyList, error::Error};

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
        std::env::var("VOICE_REQUIRE_SANDBOX").as_deref() != Ok("1"),
        "{SKIP}"
    );
    eprintln!("{SKIP}");
    false
}

struct Folders {
    home: Scratch,
    member: Scratch,
    state: Scratch,
    runs: Scratch,
}

impl Folders {
    fn new() -> Self {
        let folders = Self {
            home: Scratch::new("reader-home"),
            member: Scratch::new("reader-member"),
            state: Scratch::new("reader-state"),
            runs: Scratch::new("reader-runs"),
        };
        fs::set_permissions(&folders.state.0, fs::Permissions::from_mode(0o700)).unwrap();
        folders
    }
    fn reader(&self, program: &str, sources: &[Source<'_>]) -> Reader {
        Reader::new(
            Path::new(program),
            &self.home.0,
            &self.member.0,
            &self.state.0,
            &self.runs.0,
            sources,
            &DenyList::builtin(),
        )
        .unwrap()
    }
}

fn run(reader: &Reader, args: &[&str]) -> Output {
    reader.command(args).output().unwrap()
}

fn canary(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "canary").unwrap();
    path
}

fn refused(output: Output) {
    assert!(!output.status.success(), "{output:?}");
    assert!(
        !output.stdout.windows(6).any(|word| word == b"canary"),
        "{output:?}"
    );
}

fn readable(reader: &Reader, path: &Path) {
    let output = run(reader, &[path.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"canary");
}

#[test]
fn sources_are_readable_nothing_else_is() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let one = Scratch::new("reader-one");
    let outside = Scratch::new("reader-outside");
    let f = canary(&source.0, "sub/f");
    let g = canary(&one.0, "one/g");
    let h = canary(&one.0, "one/h");
    let other = canary(&outside.0, "other");
    let sources = [Source::Folder(&source.0), Source::File(&g)];
    let cat = folders.reader("/bin/cat", &sources);
    readable(&cat, &f);
    readable(&cat, &g);
    refused(run(&cat, &[h.to_str().unwrap()]));
    refused(run(&cat, &[other.to_str().unwrap()]));
    let ls = folders.reader("/bin/ls", &sources);
    assert!(run(&ls, &[source.0.to_str().unwrap()]).status.success());
    refused(run(&ls, &[g.parent().unwrap().to_str().unwrap()]));
}

#[test]
fn paths_resolve() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let one = Scratch::new("reader-one");
    let f = canary(&source.0, "sub/f");
    let g = canary(&one.0, "one/g");
    let reader = folders.reader(
        "/bin/realpath",
        &[Source::Folder(&source.0), Source::File(&g)],
    );
    for path in [&f, &g] {
        let output = run(&reader, &[path.to_str().unwrap()]);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, format!("{}\n", path.display()).as_bytes());
    }
}

#[test]
fn writes_go_only_to_state_and_run() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let outside = Scratch::new("reader-outside");
    let reader = folders.reader("/usr/bin/touch", &[Source::Folder(&source.0)]);
    for (root, allowed) in [
        (folders.state.0.as_path(), true),
        (reader.run_folder(), true),
        (&folders.member.0, false),
        (&source.0, false),
        (&outside.0, false),
    ] {
        let path = root.join("new");
        let output = run(&reader, &[path.to_str().unwrap()]);
        assert_eq!(output.status.success(), allowed, "{output:?}");
        assert_eq!(path.exists(), allowed);
    }
}

#[test]
fn secrets_stay_closed_inside_source_and_state() {
    if !available() {
        return;
    }
    // Separate roots keep the two spellings distinct on case-insensitive volumes.
    for upper in [false, true] {
        let folders = Folders::new();
        let source = Scratch::new("reader-source");
        let reader = folders.reader("/bin/cat", &[Source::Folder(&source.0)]);
        for root in [&source.0, &folders.state.0] {
            for name in CANARIES {
                let name = if upper {
                    name.to_uppercase()
                } else {
                    (*name).to_owned()
                };
                let path = canary(root, &name);
                refused(run(&reader, &[path.to_str().unwrap()]));
            }
        }
    }
}

#[test]
fn no_network() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let socket_folder = Scratch::new("reader-socket");
    let reader = folders.reader("/usr/bin/nc", &[Source::Folder(&socket_folder.0)]);
    let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
    tcp.set_nonblocking(true).unwrap();
    let port = tcp.local_addr().unwrap().port().to_string();
    refused(run(&reader, &["-z", "-w1", "127.0.0.1", &port]));
    assert_eq!(
        tcp.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let udp = UdpSocket::bind(("127.0.0.1", port.parse::<u16>().unwrap())).unwrap();
    udp.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let mut child = reader
        .command(&["-u", "-w1", "127.0.0.1", &port])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // A denied connection can close stdin before the canary is written.
    let _ = child.stdin.take().unwrap().write_all(b"x");
    child.wait().unwrap();
    let error = udp.recv_from(&mut [0; 64]).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    let path = socket_folder.0.join("socket");
    let unix = UnixListener::bind(&path).unwrap();
    unix.set_nonblocking(true).unwrap();
    refused(run(&reader, &["-U", path.to_str().unwrap()]));
    assert_eq!(
        unix.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn no_other_program() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let reader = folders.reader("/bin/bash", &[]);
    let output = run(&reader, &["-c", "/bin/echo started"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("started"));
}

#[test]
fn trustd_is_refused() {
    if !available() {
        return;
    }
    match Command::new("cc").arg("--version").output() {
        Ok(output) if output.status.success() => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("sandbox_profile: skipped, cc is not installed");
            return;
        }
        other => panic!("cc availability check failed: {other:?}"),
    }
    let folders = Folders::new();
    let compiled = Scratch::new("reader-program");
    let source = compiled.0.join("trustd.c");
    let program = compiled.0.join("trustd");
    fs::write(
        &source,
        r#"#include <mach/mach.h>
#include <servers/bootstrap.h>
int main(void) {
    mach_port_t port;
    kern_return_t agent = bootstrap_look_up(bootstrap_port, "com.apple.trustd.agent", &port);
    kern_return_t system = bootstrap_look_up(bootstrap_port, "com.apple.trustd", &port);
    return agent == KERN_SUCCESS || system == KERN_SUCCESS ? 0 : 1;
}
"#,
    )
    .unwrap();
    let output = Command::new("cc")
        .arg(&source)
        .arg("-o")
        .arg(&program)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let output = Command::new(&program).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let reader = folders.reader(program.to_str().unwrap(), &[]);
    let output = run(&reader, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
}

#[test]
fn hard_links_are_pinned() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let outside = Scratch::new("reader-outside");
    let file = canary(&outside.0, "outside");
    let link = source.0.join("link");
    fs::hard_link(&file, &link).unwrap();
    let cat = folders.reader("/bin/cat", &[Source::Folder(&source.0)]);
    // Seatbelt checks the path, not the file: the reader's refusal (VO11.2b) is the
    // control. If this fails, macOS checks the file now; update the spike and SECURITY.md.
    readable(&cat, &link);
    let ln = folders.reader("/bin/ln", &[Source::Folder(&source.0)]);
    let a = canary(&folders.state.0, "a");
    let b = folders.state.0.join("b");
    let output = run(&ln, &[a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert!(b.exists());
    let made = folders.state.0.join("made");
    refused(run(&ln, &[file.to_str().unwrap(), made.to_str().unwrap()]));
    assert!(!made.exists());
    for (file, name) in [
        (canary(&source.0, "inside"), "made-source"),
        (canary(&folders.member.0, "inside"), "made-member"),
    ] {
        let made = folders.state.0.join(name);
        refused(run(&ln, &[file.to_str().unwrap(), made.to_str().unwrap()]));
        assert!(!made.exists());
    }
}

#[test]
fn a_path_cannot_change_the_rules() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let outside = Scratch::new("reader-outside");
    let other = canary(&outside.0, "other");
    let paths = [
        source.0.join("a\") (allow default) ;\""),
        source.0.join("line\nbreak"),
    ];
    for path in &paths {
        fs::create_dir(path).unwrap();
    }
    let reader = folders.reader(
        "/bin/cat",
        &[Source::Folder(&paths[0]), Source::Folder(&paths[1])],
    );
    for path in &paths {
        readable(&reader, &canary(path, "f"));
    }
    refused(run(&reader, &[other.to_str().unwrap()]));
    let profile = reader.profile();
    let (before, after) = include_str!("../sandbox/reader.sb")
        .split_once(";; sources")
        .unwrap();
    assert!(profile.starts_with(before));
    assert!(profile.ends_with(after));
    let middle = &profile[before.len()..profile.len() - after.len()];
    assert!(!middle.contains('/'));
    for line in middle.lines() {
        assert!(
            line.starts_with("; ")
                || line == "(allow file-read*"
                || line == "(allow file-read-metadata"
                || [
                    "    (subpath (param \"SOURCE_",
                    "    (literal (param \"SOURCE_",
                    "    (literal (param \"ANCESTOR_",
                ]
                .iter()
                .any(|prefix| {
                    line.strip_prefix(prefix)
                        .and_then(|rest| {
                            rest.strip_suffix("\"))")
                                .or_else(|| rest.strip_suffix("\")))"))
                        })
                        .is_some_and(|index| {
                            !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
                        })
                }),
            "unexpected generated rule: {line}"
        );
    }
}

#[test]
fn profile_template_and_generated_rules() {
    let folders = Folders::new();
    let empty = folders.reader("/bin/cat", &[]).profile();
    assert!(!empty.contains(";; sources"));
    assert!(!empty.contains("SOURCE_"));
    assert!(empty.contains("; The folders above each allowed path, metadata only, so the reader can resolve paths.\n(allow file-read-metadata\n    (literal (param \"ANCESTOR_0\"))"));
    let source = Scratch::new("reader-source");
    let file = canary(&source.0, "f");
    let profile = folders
        .reader(
            "/bin/cat",
            &[Source::Folder(&source.0), Source::File(&file)],
        )
        .profile();
    assert!(profile.contains("; Sources the person agreed to, read-only.\n(allow file-read*\n    (subpath (param \"SOURCE_0\"))\n    (literal (param \"SOURCE_1\")))"));
    let template = include_str!("../sandbox/reader.sb");
    let claude = include_str!("../sandbox/claude-code.sb");
    let start = "; These rules follow the allows";
    let block = |text: &str| {
        let rest = &text[text.find(start).unwrap()..];
        rest[..rest.find("))\n").unwrap() + 2].to_owned()
    };
    assert_eq!(block(template), block(claude));
    assert_eq!(
        template
            .lines()
            .filter(|line| *line == ";; sources")
            .count(),
        1
    );
    let rules = template
        .lines()
        .filter(|line| !line.trim_start().starts_with(';'))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in ["network", "process-fork", ".credentials.json"] {
        assert!(!rules.contains(forbidden));
    }
}

fn invalid(result: voice_core::error::Result<Reader>, expected: &str) {
    match result {
        Err(Error::Invalid(message)) => assert_eq!(message, expected),
        Err(error) => panic!("unexpected error: {error}"),
        Ok(_) => panic!("reader accepted invalid input"),
    }
}

#[test]
fn refuses_invalid_sources_folders_and_programs() {
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let file = canary(&source.0, "file");
    let missing = source.0.join("missing");
    let ssh = source.0.join(".ssh");
    fs::create_dir(&ssh).unwrap();
    let pem = canary(&source.0, "real.pem");
    let key = canary(&source.0, "real.key");
    let link = source.0.join("link");
    symlink(&key, &link).unwrap();
    let new = |program: &Path, member: &Path, sources: &[Source<'_>]| {
        Reader::new(
            program,
            &folders.home.0,
            member,
            &folders.state.0,
            &folders.runs.0,
            sources,
            &DenyList::builtin(),
        )
    };
    for bad in [
        Source::File(Path::new("relative")),
        Source::File(&missing),
        Source::Folder(&file),
        Source::File(&source.0),
        Source::Folder(&ssh),
        Source::File(&pem),
        Source::File(&link),
    ] {
        invalid(
            new(Path::new("/bin/cat"), &folders.member.0, &[bad]),
            text::READER_SOURCE,
        );
    }
    let diary = source.0.join("diary");
    fs::create_dir(&diary).unwrap();
    let personal_deny = DenyList::new(&folders.home.0, &["diary".into()]).unwrap();
    invalid(
        Reader::new(
            Path::new("/bin/cat"),
            &folders.home.0,
            &folders.member.0,
            &folders.state.0,
            &folders.runs.0,
            &[Source::Folder(&diary)],
            &personal_deny,
        ),
        text::READER_SOURCE,
    );
    let _reader = new(
        Path::new("/bin/cat"),
        &folders.member.0,
        &[Source::Folder(&diary)],
    )
    .unwrap();
    let in_runs = folders.runs.0.join("source");
    fs::create_dir(&in_runs).unwrap();
    for path in [
        Path::new("/"),
        &folders.home.0,
        folders.state.0.parent().unwrap(),
        &folders.state.0,
        &in_runs,
    ] {
        invalid(
            new(
                Path::new("/bin/cat"),
                &folders.member.0,
                &[Source::Folder(path)],
            ),
            text::READER_SOURCE,
        );
    }
    let member_in_state = folders.state.0.join("member");
    let state_in_member = folders.member.0.join("state");
    let runs_in_state = folders.state.0.join("runs");
    fs::create_dir(&member_in_state).unwrap();
    fs::create_dir(&state_in_member).unwrap();
    fs::set_permissions(&state_in_member, fs::Permissions::from_mode(0o700)).unwrap();
    for (member, state, runs) in [
        (&member_in_state, &folders.state.0, &folders.runs.0),
        (&folders.member.0, &state_in_member, &folders.runs.0),
        (&folders.member.0, &folders.state.0, &runs_in_state),
    ] {
        invalid(
            Reader::new(
                Path::new("/bin/cat"),
                &folders.home.0,
                member,
                state,
                runs,
                &[],
                &DenyList::builtin(),
            ),
            text::READER_FOLDER,
        );
    }
    let too_many = (0..=MAX_SOURCES)
        .map(|_| Source::Folder(&source.0))
        .collect::<Vec<_>>();
    invalid(
        new(Path::new("/bin/cat"), &folders.member.0, &too_many),
        text::READER_SOURCE,
    );
    invalid(new(Path::new("/bin/cat"), &file, &[]), text::READER_FOLDER);
    let script = canary(&source.0, "script");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    invalid(new(&script, &folders.member.0, &[]), text::NOT_NATIVE);
    fs::set_permissions(&folders.state.0, fs::Permissions::from_mode(0o755)).unwrap();
    invalid(
        new(Path::new("/bin/cat"), &folders.member.0, &[]),
        text::READER_FOLDER,
    );
}

#[test]
fn run_folder_is_private_fresh_and_removed_and_sources_deduplicated() {
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let reader = folders.reader(
        "/bin/cat",
        &[Source::Folder(&source.0), Source::Folder(&source.0)],
    );
    let path = reader.run_folder().to_owned();
    assert_eq!(path.parent(), Some(folders.runs.0.as_path()));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    assert!(reader.profile().contains("SOURCE_0"));
    assert!(!reader.profile().contains("SOURCE_1"));
    let second = folders.reader("/bin/cat", &[]);
    assert_ne!(path, second.run_folder());
    drop(reader);
    assert!(!path.exists());
}

fn environment_names(reader: &Reader) -> Vec<String> {
    let command = reader.command(&[]);
    let mut names = command
        .get_envs()
        .map(|(name, value)| {
            assert!(value.is_some());
            name.to_str().unwrap().to_owned()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn environment_and_command() {
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let file = canary(&source.0, "f");
    let reader = folders.reader("/bin/cat", &[Source::File(&file)]);
    let names = environment_names(&reader);
    let mut expected = vec!["HOME", "PATH", "LANG", "TMPDIR"];
    // The same system account lookup supplies both optional names.
    if names.iter().any(|name| name == "USER") {
        expected.extend(["USER", "LOGNAME"]);
    }
    expected.sort();
    assert_eq!(names, expected);
    let command = reader.command(&["argument"]);
    assert_eq!(command.get_program(), "/usr/bin/sandbox-exec");
    let args = command.get_args().collect::<Vec<_>>();
    assert_eq!(args[0], "-p");
    assert_eq!(args[1], reader.profile().as_str());
    assert_eq!(command.get_current_dir(), Some(reader.run_folder()));
    assert_eq!(args[args.len() - 2], fs::canonicalize("/bin/cat").unwrap());
    assert_eq!(args[args.len() - 1], "argument");
    let (pairs, remainder) = args[2..args.len() - 2].as_chunks::<2>();
    assert!(remainder.is_empty());
    let parameters = pairs
        .iter()
        .map(|pair| {
            assert_eq!(pair[0], "-D");
            let (name, value) = pair[1].to_str().unwrap().split_once('=').unwrap();
            (name.to_owned(), PathBuf::from(value))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(parameters["SOURCE_0"], file);
    let mut ancestors = BTreeSet::new();
    for path in [
        fs::canonicalize("/bin/cat").unwrap(),
        folders.member.0.clone(),
        folders.state.0.clone(),
        reader.run_folder().to_owned(),
        file,
    ] {
        ancestors.extend(path.ancestors().skip(1).map(Path::to_owned));
    }
    let actual = (0..ancestors.len())
        .map(|index| parameters[&format!("ANCESTOR_{index}")].clone())
        .collect::<Vec<_>>();
    assert_eq!(actual, ancestors.into_iter().collect::<Vec<_>>());
    assert_eq!(parameters.len(), 5 + actual.len());
}

#[test]
fn environment_is_cleared() {
    if !available() {
        return;
    }
    let folders = Folders::new();
    let reader = folders.reader("/usr/bin/env", &[]);
    let output = run(&reader, &[]);
    assert!(output.status.success(), "environment command failed");
    // Compare names only: never print an environment value.
    let mut names = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| line.split_once('=').unwrap().0.to_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, environment_names(&reader));
}

#[test]
fn usable_keeps_only_accepted_sources_in_order() {
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let file = canary(&source.0, "file");
    let missing = source.0.join("missing");
    let ssh = source.0.join(".ssh");
    let diary = source.0.join("diary");
    let in_state = folders.state.0.join("source");
    let in_runs = folders.runs.0.join("source");
    let other_state = folders.home.0.join(".claude/state/other/source");
    let other_crew = folders.home.0.join(".claude/crew/other/source");
    for path in [&other_state, &other_crew] {
        fs::create_dir_all(path).unwrap();
    }
    for path in [&ssh, &diary, &in_state, &in_runs] {
        fs::create_dir(path).unwrap();
    }
    let deny = DenyList::new(&folders.home.0, &["diary".into()]).unwrap();
    let state_root = folders.home.0.join(".claude/state");
    let crew_root = folders.home.0.join(".claude/crew");
    let sources = [
        Source::Folder(&source.0),
        Source::File(&missing),
        Source::File(&source.0),
        Source::Folder(&file),
        Source::Folder(&folders.home.0),
        Source::Folder(folders.state.0.parent().unwrap()),
        Source::Folder(&in_state),
        Source::Folder(&in_runs),
        Source::Folder(&other_state),
        Source::Folder(&other_crew),
        Source::Folder(&state_root),
        Source::Folder(&crew_root),
        Source::Folder(&ssh),
        Source::Folder(&diary),
        Source::Folder(&source.0),
        Source::File(&file),
        Source::File(&file),
    ];
    let kept = usable(
        &folders.home.0,
        &folders.member.0,
        &folders.state.0,
        &folders.runs.0,
        &sources,
        &deny,
    );
    let paths = kept
        .iter()
        .map(|source| match source {
            Source::Folder(path) => (true, *path),
            Source::File(path) => (false, *path),
        })
        .collect::<Vec<_>>();
    assert_eq!(paths, [(true, source.0.as_path()), (false, file.as_path())]);
    Reader::new(
        Path::new("/bin/cat"),
        &folders.home.0,
        &folders.member.0,
        &folders.state.0,
        &folders.runs.0,
        &kept,
        &deny,
    )
    .unwrap();
    assert!(
        usable(
            &missing,
            &folders.member.0,
            &folders.state.0,
            &folders.runs.0,
            &sources,
            &deny,
        )
        .is_empty()
    );
}

#[test]
fn usable_refuses_linked_crew_folders_and_their_contents() {
    for linked_root in [false, true] {
        let folders = Folders::new();
        let private_copy = Scratch::new("reader-private-copy");
        let target = private_copy.0.join("assistant");
        let inside = target.join("source");
        fs::create_dir_all(&inside).unwrap();
        let file = canary(&inside, "file");
        let neighbour = private_copy.0.join("neighbour");
        fs::create_dir(&neighbour).unwrap();
        fs::create_dir_all(folders.home.0.join(".claude")).unwrap();
        let crew = folders.home.0.join(".claude/crew");
        if linked_root {
            symlink(&target, &crew).unwrap();
        } else {
            fs::create_dir(&crew).unwrap();
            symlink(&target, crew.join("other")).unwrap();
        }
        let sources = [
            Source::Folder(&target),
            Source::Folder(&inside),
            Source::File(&file),
            Source::Folder(&neighbour),
        ];
        let kept = usable(
            &folders.home.0,
            &folders.member.0,
            &folders.state.0,
            &folders.runs.0,
            &sources,
            &DenyList::builtin(),
        );
        assert_eq!(kept.len(), 1);
        assert!(matches!(kept[0], Source::Folder(path) if path == neighbour));
    }
}

#[test]
fn usable_limits_sources_to_256() {
    let folders = Folders::new();
    let source = Scratch::new("reader-source");
    let paths = (0..257)
        .map(|index| {
            let path = source.0.join(index.to_string());
            fs::create_dir(&path).unwrap();
            path
        })
        .collect::<Vec<_>>();
    let sources = paths
        .iter()
        .map(|path| Source::Folder(path))
        .collect::<Vec<_>>();
    let kept = usable(
        &folders.home.0,
        &folders.member.0,
        &folders.state.0,
        &folders.runs.0,
        &sources,
        &DenyList::builtin(),
    );
    assert_eq!(MAX_SOURCES, 256);
    assert_eq!(kept.len(), 256);
    for (source, expected) in kept.iter().zip(&paths[..256]) {
        assert!(matches!(source, Source::Folder(path) if *path == expected));
    }
    folders.reader("/bin/cat", &kept);
}
