mod common;
use common::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
    sync::atomic::AtomicBool,
    thread,
};
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
use std::{
    sync::{Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use voice_agent::{
    claude::{self, Claude},
    text,
};

fn found(stub: &Stub, home: &Path) -> Claude {
    claude::find_in(home, None, &[stub.program()]).unwrap()
}

#[test]
fn profile_folder_is_under_the_given_home() {
    let home = Scratch::new("profile-path");
    #[cfg(target_os = "macos")]
    assert_eq!(
        claude::profile_folder(&home.0),
        home.0.join("Library/Application Support/Voice/claude-code")
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        claude::profile_folder(&home.0),
        home.0.join(".local/share/voice/claude-code")
    );
}

#[test]
fn find_creates_private_app_and_profile_folders() {
    let home = Scratch::new("profile-create");
    let stub = Stub::new("exit 0");
    let claude = found(&stub, &home.0);
    let folder = claude::profile_folder(&home.0);
    assert_eq!(claude.profile(), folder.as_path());
    for path in [folder.as_path(), folder.parent().unwrap()] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[test]
fn concurrent_finds_create_private_app_and_profile_folders() {
    let home = Scratch::new("profile-concurrent");
    let stub = Stub::new("exit 0");
    let barrier = std::sync::Barrier::new(16);
    let found = thread::scope(|scope| {
        let threads: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    claude::find_in(&home.0, None, &[stub.program()])
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(found.len(), 16);
    let folder = claude::profile_folder(&home.0);
    for claude in found {
        assert_eq!(claude.profile(), folder.as_path());
    }
    for path in [folder.as_path(), folder.parent().unwrap()] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[test]
fn find_tightens_existing_app_and_profile_folders() {
    let home = Scratch::new("profile-tighten");
    let stub = Stub::new("exit 0");
    let folder = claude::profile_folder(&home.0);
    fs::create_dir_all(&folder).unwrap();
    for path in [folder.as_path(), folder.parent().unwrap()] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    found(&stub, &home.0);
    for path in [folder.as_path(), folder.parent().unwrap()] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[test]
fn find_refuses_files_and_links_at_both_private_folders() {
    for at_app in [false, true] {
        for link in [false, true] {
            let home = Scratch::new("profile-refuse");
            let stub = Stub::new("exit 0");
            let folder = claude::profile_folder(&home.0);
            let path = if at_app {
                folder.parent().unwrap()
            } else {
                &folder
            };
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            if link {
                symlink(&home.0, path).unwrap();
            } else {
                fs::write(path, "occupied").unwrap();
            }
            let error = claude::find_in(&home.0, None, &[stub.program()]).unwrap_err();
            assert_eq!(error.to_string(), text::PROFILE_FAILED);
            assert!(!error.to_string().contains(home.0.to_str().unwrap()));
        }
    }
}

#[test]
fn missing_program_makes_no_profile_folder() {
    let home = Scratch::new("profile-missing");
    let error = claude::find_in(&home.0, None, &[]).unwrap_err();
    assert_eq!(error.to_string(), text::NOT_INSTALLED);
    assert!(!claude::profile_folder(&home.0).parent().unwrap().exists());
}

#[test]
fn status_keeps_the_version_when_signed_out() {
    let home = Scratch::new("status");
    for status in [
        "echo '{\"loggedIn\":false}'",
        "echo '{}'",
        "echo bad",
        "echo '{\"loggedIn\":\"true\"}'",
        "exit 1",
    ] {
        let stub = Stub::new(&format!(
            "case \"$3\" in --version) echo '2.1.292 (Claude Code)';; auth) {status};; esac"
        ));
        let claude = found(&stub, &home.0);
        let status = claude.status(&home.0).unwrap();
        assert_eq!(status.version, "2.1.292");
        assert!(!status.signed_in);
        assert_eq!(
            claude.check(&home.0).unwrap_err().to_string(),
            text::SIGNED_OUT
        );
    }
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn sign_in_has_exact_argv_empty_stdin_and_maps_exit_status() {
    let home = Scratch::new("sign-in");
    for (body, expected) in [("exit 0", None), ("exit 1", Some(text::SIGN_IN_FAILED))] {
        let stub = Stub::new(body);
        let claude = found(&stub, &home.0);
        let opened = Mutex::new(Vec::<String>::new());
        let result =
            claude.sign_in_with(&home.0, &AtomicBool::new(false), test_sign_in(), &|link| {
                opened.lock().unwrap().push(link.to_owned());
                Ok(())
            });
        assert!(opened.lock().unwrap().is_empty());
        assert_eq!(
            result.err().map(|error| error.to_string()),
            expected.map(str::to_owned)
        );
        assert_eq!(
            stub.argv(0).unwrap(),
            [
                "--setting-sources",
                "project,local",
                "auth",
                "login",
                if cfg!(target_os = "macos") {
                    "--claudeai"
                } else {
                    "--console"
                }
            ]
        );
        assert_eq!(stub.file("stdin"), "");
    }
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn sign_in_timeout_and_failed_spawn_are_fixed_messages() {
    let home = Scratch::new("sign-in-timeout");
    let stub = Stub::new("sleep 30");
    assert_eq!(
        found(&stub, &home.0)
            .with_timeout(Duration::from_secs(1))
            .sign_in_with(
                &home.0,
                &AtomicBool::new(false),
                test_sign_in(),
                &|_| panic!("no browser requested")
            )
            .unwrap_err()
            .to_string(),
        text::SIGN_IN_TIMED_OUT
    );
    assert_eq!(
        Claude::at(home.0.join("missing"), private_profile(&home.0))
            .sign_in_with(
                &home.0,
                &AtomicBool::new(false),
                test_sign_in(),
                &|_| panic!("no browser requested")
            )
            .unwrap_err()
            .to_string(),
        text::DOES_NOT_RUN
    );
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn stopped_sign_in_kills_the_process_group_within_a_second() {
    let home = Scratch::new("sign-in-stop");
    let stub = Stub::new("echo $$ > \"$here/parent\"\nsleep 30 &\necho $! > \"$here/pid\"\nwait");
    let claude = found(&stub, &home.0);
    let stop = AtomicBool::new(false);
    let begun = Instant::now();
    let error = thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(200));
            stop.store(true, Ordering::SeqCst);
        });
        claude
            .sign_in_with(&home.0, &stop, test_sign_in(), &|_| {
                panic!("no browser requested")
            })
            .unwrap_err()
    });
    assert_eq!(error.to_string(), text::SIGN_IN_STOPPED);
    assert!(begun.elapsed() < Duration::from_secs(1));
    for name in ["parent", "pid"] {
        let pid: i32 = stub.file(name).trim().parse().unwrap();
        let started = Instant::now();
        while unsafe { libc::kill(pid, 0) } == 0 && started.elapsed() < Duration::from_secs(2) {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "{name} {pid} survived");
    }
}

#[test]
fn displayed_paths_hide_the_home_and_signed_out_text_is_exact() {
    let home = Path::new("/invented/home");
    assert_eq!(
        text::shown_program(&home.join(".local/bin/claude"), home),
        "~/.local/bin/claude"
    );
    assert_eq!(
        text::shown_program(Path::new("/opt/homebrew/bin/claude"), home),
        "/opt/homebrew/bin/claude"
    );
    assert_eq!(
        text::signed_out_at("2.1.292", "~/.local/bin/claude"),
        "Claude Code 2.1.292 at ~/.local/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with an Anthropic Console account."
    );
}

#[cfg(not(target_os = "macos"))]
const SIGN_IN_LINK: &str = "https://platform.claude.com/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A59244%2Fcallback&scope=user%3Aprofile&code_challenge=x&code_challenge_method=S256&state=y";
#[cfg(all(target_os = "macos", feature = "claude-subscription"))]
const SIGN_IN_LINK: &str = "https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A59244%2Fcallback&scope=user%3Aprofile&code_challenge=x&code_challenge_method=S256&state=y";

#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn waiting_sign_in(link: &str) -> Stub {
    Stub::new(&format!(
        r#""$BROWSER" '{link}'
for n in $(seq 1 200); do
    [ -f "$here/go" ] && exit 0
    sleep 0.05
done
exit 1"#
    ))
}

#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn two_link_sign_in(first: &str, second: &str) -> Stub {
    Stub::new(&format!(
        r#""$BROWSER" '{first}'
for n in $(seq 1 100); do
    [ -f "$here/opened" ] && break
    sleep 0.05
done
[ -f "$here/opened" ] || exit 1
"$BROWSER" '{second}'
sleep 1
exit 0"#
    ))
}

#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn assert_no_link_in_files(folder: &Path, link: &str) {
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            assert_no_link_in_files(&path, link);
        } else if path.file_name().unwrap() != "claude" {
            assert!(
                !fs::read(&path)
                    .unwrap()
                    .windows(link.len())
                    .any(|bytes| bytes == link.as_bytes()),
                "sign-in link retained"
            );
        }
    }
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn sign_in_opens_the_valid_link_exactly_once_and_retains_no_link() {
    let home = Scratch::new("sign-in-link");
    let stub = waiting_sign_in(SIGN_IN_LINK);
    let claude = found(&stub, &home.0);
    let opened = Mutex::new(Vec::<String>::new());
    thread::scope(|scope| {
        scope.spawn(|| {
            let started = Instant::now();
            while opened.lock().unwrap().is_empty() && started.elapsed() < Duration::from_secs(5) {
                thread::sleep(Duration::from_millis(10));
            }
            // Leave several watcher turns before allowing the child to finish.
            thread::sleep(Duration::from_millis(250));
            fs::write(stub.folder.0.join("go"), "").unwrap();
        });
        claude
            .sign_in_with(&home.0, &AtomicBool::new(false), test_sign_in(), &|link| {
                opened.lock().unwrap().push(link.to_owned());
                Ok(())
            })
            .unwrap();
    });
    assert!(opened.lock().unwrap().as_slice() == [SIGN_IN_LINK]);
    assert_no_link_in_files(&stub.folder.0, SIGN_IN_LINK);
    assert_no_link_in_files(claude.profile().parent().unwrap(), SIGN_IN_LINK);
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn sign_in_never_opens_a_second_valid_link() {
    let home = Scratch::new("sign-in-second-link");
    let second = SIGN_IN_LINK.replace("state=y", "state=z");
    let stub = two_link_sign_in(SIGN_IN_LINK, &second);
    let claude = found(&stub, &home.0);
    let opened = Mutex::new(Vec::<String>::new());
    claude
        .sign_in_with(&home.0, &AtomicBool::new(false), test_sign_in(), &|link| {
            opened.lock().unwrap().push(link.to_owned());
            fs::write(stub.folder.0.join("opened"), "").unwrap();
            Ok(())
        })
        .unwrap();
    let opened = opened.lock().unwrap();
    assert_eq!(opened.len(), 1);
    assert!(opened.as_slice() == [SIGN_IN_LINK]);
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn sign_in_refuses_an_invalid_link_without_opening_it_and_kills_the_child() {
    let home = Scratch::new("sign-in-invalid-link");
    let stub = waiting_sign_in("https://example.com/x");
    let opened = Mutex::new(Vec::<String>::new());
    let started = Instant::now();
    let error = found(&stub, &home.0)
        .sign_in_with(&home.0, &AtomicBool::new(false), test_sign_in(), &|link| {
            opened.lock().unwrap().push(link.to_owned());
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.to_string(), text::SIGN_IN_FAILED);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(opened.lock().unwrap().is_empty());
    assert_no_link_in_files(&stub.folder.0, "https://example.com/x");
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn sign_in_opener_failure_kills_the_child() {
    let home = Scratch::new("sign-in-opener-failed");
    let stub = waiting_sign_in(SIGN_IN_LINK);
    let opened = Mutex::new(Vec::<String>::new());
    let started = Instant::now();
    let error = found(&stub, &home.0)
        .sign_in_with(&home.0, &AtomicBool::new(false), test_sign_in(), &|link| {
            opened.lock().unwrap().push(link.to_owned());
            Err(std::io::Error::other("invented opener failure"))
        })
        .unwrap_err();
    assert_eq!(error.to_string(), text::SIGN_IN_FAILED);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(opened.lock().unwrap().as_slice() == [SIGN_IN_LINK]);
    assert_no_link_in_files(&stub.folder.0, SIGN_IN_LINK);
}

#[test]
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn sign_in_stops_while_waiting_after_the_browser_opens() {
    let home = Scratch::new("sign-in-wait-stop");
    let stub = waiting_sign_in(SIGN_IN_LINK);
    let stop = AtomicBool::new(false);
    let opened = Mutex::new(Vec::<String>::new());
    let started = Instant::now();
    let error = found(&stub, &home.0)
        .sign_in_with(&home.0, &stop, test_sign_in(), &|link| {
            opened.lock().unwrap().push(link.to_owned());
            stop.store(true, Ordering::SeqCst);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.to_string(), text::SIGN_IN_STOPPED);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(opened.lock().unwrap().as_slice() == [SIGN_IN_LINK]);
    assert_no_link_in_files(&stub.folder.0, SIGN_IN_LINK);
}

#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
fn test_sign_in() -> claude::SignIn {
    #[cfg(target_os = "macos")]
    {
        claude::SignIn::Subscription
    }
    #[cfg(not(target_os = "macos"))]
    {
        claude::SignIn::Console
    }
}

#[cfg(target_os = "macos")]
#[test]
fn console_sign_in_is_refused_before_any_child_starts() {
    let home = Scratch::new("console-unavailable");
    let stub = Stub::new("exit 0");
    let error = found(&stub, &home.0)
        .sign_in_with(
            &home.0,
            &AtomicBool::new(false),
            claude::SignIn::Console,
            &|_| panic!("no browser requested"),
        )
        .unwrap_err();
    let expected =
        "Signing in with a Console account doesn't work on a Mac yet. A later update adds it.";
    #[cfg(feature = "claude-subscription")]
    let expected = format!("{expected} Use Sign in with a Claude subscription instead.");
    assert_eq!(error.to_string(), expected);
    assert!(!stub.ran());
}

#[cfg(target_os = "macos")]
#[test]
fn sign_out_uses_claude_code_to_remove_its_login_file() {
    let home = Scratch::new("sign-out-remove");
    let stub = Stub::new(
        r#"[ "$3" = auth ] && [ "$4" = logout ] || exit 1
rm "$CLAUDE_CONFIG_DIR/.credentials.json"
exit 0"#,
    );
    let claude = found(&stub, &home.0);
    let file = claude.profile().join(".credentials.json");
    fs::write(&file, "").unwrap();
    claude.sign_out(&home.0).unwrap();
    assert!(!file.exists());
    assert_eq!(
        stub.argv(0).unwrap(),
        ["--setting-sources", "project,local", "auth", "logout"]
    );
    assert_eq!(stub.file("stdin"), "");
    assert_launch_env(&stub, 0, claude.profile());
}

#[cfg(target_os = "macos")]
#[test]
fn sign_out_nonzero_exit_is_a_fixed_error_and_keeps_the_file() {
    let home = Scratch::new("sign-out-failed");
    let stub = Stub::new("exit 1");
    let claude = found(&stub, &home.0);
    let file = claude.profile().join(".credentials.json");
    fs::write(&file, "").unwrap();
    assert_eq!(
        claude.sign_out(&home.0).unwrap_err().to_string(),
        text::SIGN_OUT_FAILED
    );
    assert!(file.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn sign_out_reports_a_leftover_login_file_without_touching_it() {
    let home = Scratch::new("sign-out-leftover");
    let stub = Stub::new("exit 0");
    let claude = found(&stub, &home.0);
    let file = claude.profile().join(".credentials.json");
    fs::write(&file, "").unwrap();
    assert_eq!(
        claude.sign_out(&home.0).unwrap_err().to_string(),
        text::SIGN_OUT_FILE_LEFT
    );
    assert!(file.exists());
}

#[cfg(target_os = "macos")]
fn backup_exclusion_present(path: &Path) -> bool {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: both C strings live for the call; null with size zero asks only for the size.
    unsafe {
        libc::getxattr(
            path.as_ptr(),
            c"com.apple.metadata:com_apple_backup_excludeItem".as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            0,
        ) >= 0
    }
}

#[cfg(target_os = "macos")]
#[test]
fn find_excludes_only_the_profile_from_time_machine() {
    let home = Scratch::new("profile-backup");
    let stub = Stub::new("exit 0");
    let claude = found(&stub, &home.0);
    assert!(backup_exclusion_present(claude.profile()));
    assert!(!backup_exclusion_present(
        claude.profile().parent().unwrap()
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn an_existing_backup_exclusion_does_not_run_tmutil() {
    let home = Scratch::new("profile-backup-existing");
    let stub = Stub::new("exit 0");
    let claude = found(&stub, &home.0);
    // Set a fixture attribute directly; the test must not rely on the missing production fix.
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(claude.profile().as_os_str().as_bytes()).unwrap();
    let value = b"fixture";
    // SAFETY: the strings and value buffer are live and their lengths are correct.
    assert_eq!(
        unsafe {
            libc::setxattr(
                path.as_ptr(),
                c"com.apple.metadata:com_apple_backup_excludeItem".as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
                0,
            )
        },
        0
    );
    let tmutil = Stub::new("touch \"$here/ran\"; exit 1");
    claude::find_in_with_tmutil(&home.0, None, &[stub.program()], &tmutil.program()).unwrap();
    assert!(!tmutil.ran());
    assert!(!tmutil.folder.0.join("ran").exists());
    assert!(backup_exclusion_present(claude.profile()));
}

#[cfg(target_os = "macos")]
#[test]
fn a_failed_backup_exclusion_is_silent_and_retried_by_find() {
    let home = Scratch::new("profile-backup-failed");
    let stub = Stub::new("exit 0");
    // RECORD consumes stdin, so reaching this stub also proves tmutil's stdin is closed.
    let tmutil = Stub::new("exit 1");
    for _ in 0..2 {
        let claude =
            claude::find_in_with_tmutil(&home.0, None, &[stub.program()], &tmutil.program())
                .unwrap();
        assert!(!backup_exclusion_present(claude.profile()));
    }
    assert_eq!(
        tmutil.argv(0).unwrap(),
        [
            "addexclusion",
            claude::profile_folder(&home.0).to_str().unwrap()
        ]
    );
    assert!(tmutil.argv(1).is_some());
}

#[cfg(target_os = "macos")]
#[test]
fn missing_and_timed_out_tmutil_do_not_fail_find() {
    let home = Scratch::new("profile-backup-missing");
    let stub = Stub::new("exit 0");
    let missing = home.0.join("missing-tmutil");
    let claude = claude::find_in_with_tmutil(&home.0, None, &[stub.program()], &missing).unwrap();
    assert!(!backup_exclusion_present(claude.profile()));
    let tmutil = Stub::new("sleep 30");
    let started = std::time::Instant::now();
    let claude =
        claude::find_in_with_tmutil(&home.0, None, &[stub.program()], &tmutil.program()).unwrap();
    assert!(tmutil.ran());
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    assert!(!backup_exclusion_present(claude.profile()));
}

#[cfg(target_os = "macos")]
#[test]
fn sign_out_reports_a_dangling_login_link_without_removing_it() {
    let home = Scratch::new("sign-out-link");
    let stub = Stub::new("exit 0");
    let claude = found(&stub, &home.0);
    let file = claude.profile().join(".credentials.json");
    symlink(home.0.join("missing-login"), &file).unwrap();
    assert_eq!(
        claude.sign_out(&home.0).unwrap_err().to_string(),
        text::SIGN_OUT_FILE_LEFT
    );
    assert!(fs::symlink_metadata(file).unwrap().file_type().is_symlink());
}
