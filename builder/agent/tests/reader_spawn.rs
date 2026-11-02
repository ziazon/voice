#![cfg(target_os = "macos")]

mod common;

use common::Scratch;
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        io::{FromRawFd, OwnedFd},
    },
    path::Path,
    process::{Command, Stdio},
};
use voice_agent::{
    reader::{Reader, Source},
    spawn,
};
use voice_core::deny::DenyList;
const SKIP: &str =
    "sandbox_profile: skipped, already inside a sandbox (macOS refuses a sandbox inside one)";

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

#[test]
fn the_reader_gets_only_stdin_stdout_and_stderr() {
    if !available() {
        return;
    }
    let mut sibling = Command::new("/bin/cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = [0; 2];
    // SAFETY: pipe points to two writable descriptors.
    assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
    // SAFETY: pipe succeeded and both descriptors are newly owned.
    let (_read, _write) = unsafe { (OwnedFd::from_raw_fd(pipe[0]), OwnedFd::from_raw_fd(pipe[1])) };
    // SAFETY: the write end is live and fd 200 is reserved by this test.
    assert_eq!(unsafe { libc::dup2(pipe[1], 200) }, 200);
    // SAFETY: dup2 returned a newly owned descriptor.
    let inherited = unsafe { OwnedFd::from_raw_fd(200) };
    let mut app = Vec::new();
    let mut inherited_identity = None;
    for fd in 3..=255 {
        let stat = match fs::metadata(format!("/dev/fd/{fd}")) {
            Ok(stat) => stat,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(libc::EBADF) =>
            {
                continue;
            }
            Err(error) => panic!("metadata for fd {fd}: {error}"),
        };
        if stat.file_type().is_fifo() || stat.file_type().is_socket() {
            let identity = (stat.dev(), stat.ino());
            app.push(identity);
            if fd == 200 {
                inherited_identity = Some(identity);
            }
        }
    }
    let inherited_identity = inherited_identity.expect("fd 200 is a pipe");
    assert!(app.contains(&inherited_identity));
    let parse = |output: &str| {
        output
            .lines()
            .map(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                assert_eq!(fields.len(), 3, "{line}");
                (
                    fields[0].parse::<i32>().unwrap(),
                    (
                        fields[1].parse::<u64>().unwrap(),
                        fields[2].parse::<u64>().unwrap(),
                    ),
                )
            })
            .collect::<Vec<_>>()
    };
    let folders = Folders::new();
    let reader = folders.reader("/bin/zsh", &[]);
    let args = [
        "-fc",
        "zmodload zsh/stat; for n in {3..255}; do [[ -e /dev/fd/$n ]] || continue; zstat -A d +device /dev/fd/$n; zstat -A i +inode /dev/fd/$n; print $n $d $i; done",
    ];
    let control = reader.command(&args).output().unwrap();
    assert!(control.status.success(), "zsh measurement failed");
    let control = parse(&String::from_utf8(control.stdout).unwrap());
    assert!(!control.is_empty(), "zsh printed no descriptor identities");
    assert!(
        control.contains(&(200, inherited_identity)) && app.contains(&inherited_identity),
        "{control:?}"
    );
    let mut child = reader.spawn(&args).unwrap();
    drop(child.stdin.take());
    let mut output = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    assert!(child.wait().unwrap().success(), "zsh measurement failed");
    drop(inherited);
    sibling.kill().unwrap();
    sibling.wait().unwrap();
    let measured = parse(&output);
    assert!(!measured.is_empty(), "zsh printed no descriptor identities");
    assert!(
        measured
            .iter()
            .all(|(fd, identity)| *fd != 200 && !app.contains(identity)),
        "{measured:?}"
    );
}

#[test]
fn a_spawned_child_reads_stdin_and_writes_stdout() {
    let dir = Scratch::new("spawn-cat");
    let mut child = spawn::spawn(Path::new("/bin/cat"), &[], &[], &dir.0).unwrap();
    child.stdin.take().unwrap().write_all(b"a line\n").unwrap();
    let mut output = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    assert_eq!(output, "a line\n");
    assert_eq!(child.wait().unwrap().code(), Some(0));
}
