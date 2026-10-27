mod common;
use common::*;
use persona_builder_agent::{
    claude::{self, Claude},
    text,
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
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
        home.0
            .join("Library/Application Support/Persona Builder/claude-code")
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        claude::profile_folder(&home.0),
        home.0.join(".local/share/persona-builder/claude-code")
    );
}

#[test]
fn find_creates_private_app_and_profile_folders() {
    let home = Scratch::new("profile-create");
    let stub = Stub::new("exit 0");
    let claude = found(&stub, &home.0);
    let folder = claude::profile_folder(&home.0);
    assert_eq!(claude.profile(), Some(folder.as_path()));
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
            "case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) {status};; esac"
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
fn sign_in_has_exact_argv_empty_stdin_and_maps_exit_status() {
    let home = Scratch::new("sign-in");
    for (body, expected) in [("exit 0", None), ("exit 1", Some(text::SIGN_IN_FAILED))] {
        let stub = Stub::new(body);
        let claude = found(&stub, &home.0);
        let result = claude.sign_in(&home.0, &AtomicBool::new(false));
        assert_eq!(
            result.err().map(|error| error.to_string()),
            expected.map(str::to_owned)
        );
        assert_eq!(stub.argv(0).unwrap(), ["auth", "login", "--claudeai"]);
        assert_eq!(stub.file("stdin"), "");
    }
}

#[test]
fn sign_in_timeout_and_failed_spawn_are_fixed_messages() {
    let home = Scratch::new("sign-in-timeout");
    let stub = Stub::new("sleep 30");
    assert_eq!(
        found(&stub, &home.0)
            .with_timeout(Duration::from_secs(1))
            .sign_in(&home.0, &AtomicBool::new(false))
            .unwrap_err()
            .to_string(),
        text::SIGN_IN_TIMED_OUT
    );
    assert_eq!(
        Claude::at(home.0.join("missing"))
            .sign_in(&home.0, &AtomicBool::new(false))
            .unwrap_err()
            .to_string(),
        text::DOES_NOT_RUN
    );
}

#[test]
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
        claude.sign_in(&home.0, &stop).unwrap_err()
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
        "Claude Code 2.1.292 at ~/.local/bin/claude isn't signed in for Persona Builder yet. Persona Builder keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with your Claude account."
    );
}
