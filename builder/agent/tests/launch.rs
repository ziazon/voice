use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use voice_agent::{claude::SignIn, sandbox, text};
use voice_core::error::Error;

const SIGN_IN_LINK: &str = "https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A59244%2Fcallback&scope=user%3Aprofile&code_challenge=x&code_challenge_method=S256&state=y";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "voice-launch-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }

    fn program(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn invalid<T>(result: voice_core::error::Result<T>, message: &str) {
    assert!(matches!(result, Err(Error::Invalid(ref found)) if found == message));
}

#[test]
fn valid_links_have_only_the_two_sign_in_pages_and_safe_ascii() {
    for prefix in [
        "https://claude.com/cai/oauth/authorize?",
        "https://claude.ai/oauth/authorize?",
    ] {
        let good = SIGN_IN_LINK.replace("https://claude.com/cai/oauth/authorize?", prefix);
        assert!(sandbox::valid_link(&good, SignIn::Subscription));
        assert!(!sandbox::valid_link(prefix, SignIn::Subscription));
        let exact = format!("{good}{}", "a".repeat(sandbox::MAX_LINK - good.len()));
        assert!(sandbox::valid_link(&exact, SignIn::Subscription));
        assert!(!sandbox::valid_link(
            &format!("{exact}a"),
            SignIn::Subscription
        ));
        for byte in 0..=255_u8 {
            if !(0x21..=0x7e).contains(&byte) || b"\"<>\\^`{|}".contains(&byte) {
                let link = format!("{good}{}", char::from(byte));
                assert!(
                    !sandbox::valid_link(&link, SignIn::Subscription),
                    "accepted byte {byte}"
                );
            }
        }
    }
    for link in [
        "",
        " ",
        "\t",
        "http://claude.com/cai/oauth/authorize?a",
        "https://claude.com.example/cai/oauth/authorize?a",
        "https://claude.com/cai/oauth/authorize@x?a",
        "https://claude.com/cai/oauth/authorizex?a",
        "https://claude.com/cai/oauth/authorize",
        "https://claude.ai.example/oauth/authorize?a",
        "https://claude.ai/oauth/authorize@x?a",
        "https://claude.ai/oauth/authorizex?a",
    ] {
        let link = link.replace(
            "?a",
            &format!("?{}", SIGN_IN_LINK.split_once('?').unwrap().1),
        );
        assert!(
            !sandbox::valid_link(&link, SignIn::Subscription),
            "accepted {link:?}"
        );
    }
}

#[test]
fn sign_in_query_keys_are_plain_and_nonempty() {
    assert!(sandbox::valid_link(SIGN_IN_LINK, SignIn::Subscription));
    let accepted: Vec<_> = [
        "&client%5Fid=other",
        "&redirect%5Furi=https%3A%2F%2Fexample.com%2Fcallback",
        "&=x",
        "&client_id+=other",
    ]
    .into_iter()
    .filter(|suffix| sandbox::valid_link(&format!("{SIGN_IN_LINK}{suffix}"), SignIn::Subscription))
    .collect();
    assert!(accepted.is_empty(), "accepted query keys: {accepted:?}");
}

#[test]
fn sign_in_requires_the_claude_code_client_and_local_callback() {
    for port in ["1", "65535"] {
        assert!(sandbox::valid_link(
            &SIGN_IN_LINK.replace("59244", port),
            SignIn::Subscription
        ));
    }
    assert!(sandbox::valid_link(
        &SIGN_IN_LINK.replace("code=true", "code=a+b=c"),
        SignIn::Subscription
    ));
    assert!(sandbox::valid_link(
        &SIGN_IN_LINK.replace("response_type=code", "response_type=%63ode"),
        SignIn::Subscription
    ));
    let mut invalid = vec![
        SIGN_IN_LINK.replace("9d1c250a-e61b-44d9-88ed-5944d1962f5e", "other"),
        format!("{SIGN_IN_LINK}&client_id=other"),
        format!("{SIGN_IN_LINK}&response_type=code"),
        format!("{SIGN_IN_LINK}&redirect_uri=http%3A%2F%2Flocalhost%3A1%2Fcallback"),
        format!("{SIGN_IN_LINK}&state=z"),
        SIGN_IN_LINK.replace(
            "&redirect_uri=http%3A%2F%2Flocalhost%3A59244%2Fcallback",
            "",
        ),
        SIGN_IN_LINK.replace("&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e", ""),
        SIGN_IN_LINK.replace("&response_type=code", ""),
        SIGN_IN_LINK.replace("response_type=code", "response_type=token"),
        format!("{SIGN_IN_LINK}&broken"),
        format!("{SIGN_IN_LINK}#fragment"),
    ];
    for callback in [
        "https://example.com/callback",
        "http://localhost.example.com:1/callback",
        "http://localhost:0/callback",
        "http://localhost:65536/callback",
        "http://localhost:1/callback/x",
        "http://127.0.0.1:1/callback",
        "http://localhost:01/callback",
        "http://localhost:000001/callback",
        "http://localhost:+1/callback",
        "http://localhost:/callback",
    ] {
        invalid.push(SIGN_IN_LINK.replace("http%3A%2F%2Flocalhost%3A59244%2Fcallback", callback));
    }
    for escape in ["%", "%2", "%GG"] {
        invalid.push(format!("{SIGN_IN_LINK}&extra={escape}"));
    }
    for link in invalid {
        assert!(
            !sandbox::valid_link(&link, SignIn::Subscription),
            "accepted {link}"
        );
    }
}

#[test]
fn resolves_only_native_regular_programs() {
    let scratch = Scratch::new();
    for (name, bytes) in [
        ("script", b"#!/bin/sh\nexit 0\n".as_slice()),
        ("short", b"xx".as_slice()),
        ("empty", b"".as_slice()),
        ("elf", b"\x7fELF".as_slice()),
        ("other", b"abcd".as_slice()),
    ] {
        invalid(
            sandbox::resolve_program(&scratch.program(name, bytes)),
            text::NOT_NATIVE,
        );
    }
    for (name, bytes) in [
        ("macho", b"\xcf\xfa\xed\xfe"),
        ("universal", b"\xca\xfe\xba\xbe"),
        ("universal64", b"\xca\xfe\xba\xbf"),
    ] {
        let program = scratch.program(name, bytes);
        assert_eq!(sandbox::resolve_program(&program).unwrap(), program);
    }
    invalid(
        sandbox::resolve_program(&scratch.0.join("missing")),
        text::DOES_NOT_RUN,
    );
    invalid(sandbox::resolve_program(&scratch.0), text::DOES_NOT_RUN);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let program = scratch.program("not-executable", b"\xcf\xfa\xed\xfe");
        fs::set_permissions(&program, fs::Permissions::from_mode(0o600)).unwrap();
        invalid(sandbox::resolve_program(&program), text::DOES_NOT_RUN);
    }
}

#[test]
fn runs_live_beside_the_profile() {
    assert_eq!(
        sandbox::runs_folder(&PathBuf::from("app/profile")),
        PathBuf::from("app/runs")
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn unsupported_platform_does_nothing_even_for_invalid_inputs() {
    let scratch = Scratch::new();
    invalid(
        sandbox::Launch::new(
            &scratch.0.join("missing"),
            &scratch.0.join("profile"),
            &scratch.0,
            sandbox::RunFolder::Fresh,
            0,
            true,
        ),
        text::MAC_ONLY,
    );
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use sandbox::{Launch, Link, RunFolder};
    use std::{
        collections::BTreeMap,
        ffi::OsString,
        io::Write,
        os::unix::fs::{MetadataExt, PermissionsExt, symlink},
        path::Path,
        process::Command,
    };

    const HELPER: &str =
        "#!/bin/bash\numask 077\nprintf '%s\\n' \"$1\" > \"${0%/*}/sign-in-link\"\n";

    fn private(path: &Path) {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn profile(scratch: &Scratch) -> PathBuf {
        let profile = scratch.0.join("profile");
        private(&profile);
        profile
    }

    fn launch(profile: &Path, home: &Path, run: RunFolder<'_>, sign_in: bool) -> Launch {
        Launch::new(Path::new("/bin/ls"), profile, home, run, 4321, sign_in).unwrap()
    }

    #[test]
    fn concurrent_launches_create_distinct_fresh_run_folders() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        assert!(!sandbox::runs_folder(&profile).exists());
        let barrier = std::sync::Barrier::new(16);
        let launches = std::thread::scope(|scope| {
            let threads: Vec<_> = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        Launch::new(
                            Path::new("/bin/ls"),
                            &profile,
                            &scratch.0,
                            RunFolder::Fresh,
                            4321,
                            false,
                        )
                    })
                })
                .collect();
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap().unwrap())
                .collect::<Vec<_>>()
        });
        let folders: std::collections::BTreeSet<_> =
            launches.iter().map(|launch| launch.run_folder()).collect();
        assert_eq!(folders.len(), 16);
    }

    #[cfg(feature = "test-stubs")]
    #[test]
    fn scripts_run_directly_with_the_native_launch_environment() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let script = scratch.program("script", b"#!/bin/sh\nexit 0\n");
        let direct =
            Launch::new(&script, &profile, &scratch.0, RunFolder::Fresh, 4322, false).unwrap();
        let native = launch(&profile, &scratch.0, RunFolder::Fresh, false);
        let command = direct.command(&["x"]);
        assert_eq!(command.get_program(), script.as_os_str());
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--setting-sources", "project,local", "x"]
        );
        assert_eq!(command.get_current_dir(), Some(direct.run_folder()));
        let native_command = native.command(&["x"]);
        assert_eq!(native_command.get_program(), sandbox::SANDBOX_EXEC);
        let environment = |command: &Command| {
            command
                .get_envs()
                .filter(|(name, _)| {
                    !["CLAUDE_CODE_TMPDIR", "TMPDIR", "HTTPS_PROXY", "https_proxy"]
                        .iter()
                        .any(|skip| name == skip)
                })
                .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
                .collect::<Vec<_>>()
        };
        assert!(environment(&command) == environment(&native_command));
        invalid(sandbox::resolve_program(&script), text::NOT_NATIVE);
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn refuses_a_folder_another_account_owns() {
        let folder = Path::new("/private/var/audit");
        let metadata = fs::metadata(folder).unwrap();
        if metadata.uid() != 0
            || metadata.permissions().mode() & 0o777 != 0o700
            || sandbox::has_acl(folder).unwrap()
        {
            eprintln!("launch: skipped, /private/var/audit is not a root-owned private folder");
            return;
        }
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Given(folder),
                4321,
                false,
            ),
            text::RUN_FOLDER_FAILED,
        );
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                folder,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                false,
            ),
            text::PROFILE_FAILED,
        );
    }

    #[test]
    fn refuses_a_fresh_run_parent_with_a_non_inheritable_acl() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let runs = sandbox::runs_folder(&profile);
        private(&runs);
        let output = Command::new("/bin/chmod")
            .args(["+a", "everyone allow list"])
            .arg(&runs)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                false,
            ),
            text::RUN_FOLDER_FAILED,
        );
        assert_eq!(fs::read_dir(&runs).unwrap().count(), 0);
        let output = Command::new("/bin/chmod")
            .arg("-N")
            .arg(&runs)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        drop(launch(&profile, &scratch.0, RunFolder::Fresh, false));
    }

    #[test]
    fn refuses_a_fresh_run_that_inherits_an_acl() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let runs = sandbox::runs_folder(&profile);
        private(&runs);
        let output = Command::new("/bin/chmod")
            .args([
                "+a",
                "everyone allow list,search,add_file,file_inherit,directory_inherit",
            ])
            .arg(&runs)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                false,
            ),
            text::RUN_FOLDER_FAILED,
        );
        assert_eq!(fs::read_dir(&runs).unwrap().count(), 0);
        let output = Command::new("/bin/chmod")
            .arg("-N")
            .arg(&runs)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        drop(launch(&profile, &scratch.0, RunFolder::Fresh, false));
    }

    #[test]
    fn refuses_a_profile_with_an_acl_until_it_is_removed() {
        refuses_a_folder_with_an_acl(true);
    }

    #[test]
    fn refuses_a_given_run_with_an_acl_until_it_is_removed() {
        refuses_a_folder_with_an_acl(false);
    }

    fn refuses_a_folder_with_an_acl(on_profile: bool) {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let given = scratch.0.join("given");
        private(&given);
        let folder = if on_profile { &profile } else { &given };
        let output = Command::new("/bin/chmod")
            .args(["+a", "everyone allow list"])
            .arg(folder)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(mode(folder), 0o700);
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Given(&given),
                4321,
                false,
            ),
            if on_profile {
                text::PROFILE_FAILED
            } else {
                text::RUN_FOLDER_FAILED
            },
        );
        let output = Command::new("/bin/chmod")
            .arg("-N")
            .arg(folder)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        drop(launch(
            &profile,
            &scratch.0,
            RunFolder::Given(&given),
            false,
        ));
    }

    #[test]
    fn sign_in_removes_an_old_link_file() {
        sign_in_removes_an_old_link(false);
    }

    #[test]
    fn sign_in_removes_an_old_link_symlink_without_touching_its_target() {
        sign_in_removes_an_old_link(true);
    }

    fn sign_in_removes_an_old_link(linked: bool) {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let given = scratch.0.join("given");
        private(&given);
        let target = scratch.0.join("target");
        let bytes = format!("{SIGN_IN_LINK}\n");
        fs::write(&target, &bytes).unwrap();
        let link = given.join(sandbox::LINK);
        if linked {
            symlink(&target, &link).unwrap();
        } else {
            fs::write(&link, &bytes).unwrap();
        }
        let launch = launch(&profile, &scratch.0, RunFolder::Given(&given), true);
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::NotYet);
        assert!(fs::symlink_metadata(link).is_err());
        assert_eq!(fs::read(target).unwrap(), bytes.as_bytes());
    }

    #[test]
    fn real_native_program_and_its_symlink_resolve() {
        let scratch = Scratch::new();
        let native = fs::canonicalize("/bin/ls").unwrap();
        assert_eq!(
            sandbox::resolve_program(Path::new("/bin/ls")).unwrap(),
            native
        );
        let link = scratch.0.join("linked-program");
        symlink("/bin/ls", &link).unwrap();
        assert_eq!(sandbox::resolve_program(&link).unwrap(), native);
    }

    #[test]
    fn fresh_runs_are_private_unique_and_removed_while_given_runs_stay() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let first = launch(&profile, &scratch.0, RunFolder::Fresh, false);
        let run = first.run_folder().to_owned();
        assert_eq!(run.parent().unwrap(), sandbox::runs_folder(&profile));
        assert_eq!(mode(run.parent().unwrap()), 0o700);
        assert_eq!(mode(&run), 0o700);
        let second = launch(&profile, &scratch.0, RunFolder::Fresh, false);
        assert_ne!(run, second.run_folder());
        let parts = |path: &Path| {
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .split('-')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let first_name = parts(&run);
        let second_name = parts(second.run_folder());
        assert_eq!(first_name.len(), 3);
        assert_eq!(first_name[0], std::process::id().to_string());
        assert_ne!(first_name[1], second_name[1]);
        assert!(first_name[1].parse::<u64>().is_ok());
        assert!(first_name[2].parse::<u128>().unwrap() > 0);
        assert!(!run.join(sandbox::HELPER).exists());
        drop(first);
        assert!(!run.exists());
        assert!(second.run_folder().is_dir());
        let given = scratch.0.join("given");
        private(&given);
        drop(launch(
            &profile,
            &scratch.0,
            RunFolder::Given(&given),
            false,
        ));
        assert!(given.is_dir());
    }

    #[test]
    fn given_runs_remove_sign_in_files_and_can_sign_in_again() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let given = scratch.0.join("given");
        private(&given);
        let kept = given.join("kept");
        fs::write(&kept, "canary").unwrap();
        let target = scratch.0.join("target");
        fs::write(&target, "canary").unwrap();
        for linked in [false, true] {
            let launch = launch(&profile, &scratch.0, RunFolder::Given(&given), true);
            let helper = given.join(sandbox::HELPER);
            let link = given.join(sandbox::LINK);
            assert_eq!(fs::read(&helper).unwrap(), HELPER.as_bytes());
            if linked {
                fs::remove_file(&helper).unwrap();
                symlink(&target, &helper).unwrap();
                symlink(&target, &link).unwrap();
            } else {
                fs::write(&link, format!("{SIGN_IN_LINK}\n")).unwrap();
            }
            drop(launch);
            assert!(given.is_dir());
            assert!(fs::symlink_metadata(helper).is_err());
            assert!(fs::symlink_metadata(link).is_err());
            assert_eq!(fs::read(&kept).unwrap(), b"canary");
            assert_eq!(fs::read(&target).unwrap(), b"canary");
        }
    }

    #[test]
    fn plain_given_runs_leave_another_launchs_sign_in_files_alone() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let given = scratch.0.join("given");
        private(&given);
        let sign_in = launch(&profile, &scratch.0, RunFolder::Given(&given), true);
        let plain = launch(&profile, &scratch.0, RunFolder::Given(&given), false);
        let helper = given.join(sandbox::HELPER);
        let link = given.join(sandbox::LINK);
        let link_bytes = format!("{SIGN_IN_LINK}\n");
        fs::write(&link, &link_bytes).unwrap();

        drop(plain);
        assert!(helper.is_file());
        assert!(link.is_file());
        assert_eq!(fs::read(&helper).unwrap(), HELPER.as_bytes());
        assert_eq!(fs::read(&link).unwrap(), link_bytes.as_bytes());

        drop(sign_in);
        assert!(!helper.exists());
        assert!(!link.exists());
        assert!(given.is_dir());
    }

    #[test]
    fn refuses_a_symlink_in_place_of_the_profile() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let link = scratch.0.join("profile-link");
        symlink(&profile, &link).unwrap();
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &link,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                false,
            ),
            text::PROFILE_FAILED,
        );
        assert!(!sandbox::runs_folder(&profile).exists());
    }

    #[test]
    fn refuses_a_symlink_in_place_of_a_given_run() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let given = scratch.0.join("given");
        private(&given);
        let link = scratch.0.join("run-link");
        symlink(&given, &link).unwrap();
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Given(&link),
                4321,
                false,
            ),
            text::RUN_FOLDER_FAILED,
        );
        assert_eq!(fs::read_dir(given).unwrap().count(), 0);
    }

    #[test]
    fn refuses_bad_inputs_in_order_before_creating_a_run() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        invalid(
            Launch::new(
                Path::new("missing"),
                &profile,
                &scratch.0,
                RunFolder::Fresh,
                0,
                false,
            ),
            "the proxy port must be set",
        );
        let script = scratch.program("not-native", b"plain text\n");
        invalid(
            Launch::new(&script, &profile, &scratch.0, RunFolder::Fresh, 4321, false),
            text::NOT_NATIVE,
        );
        assert!(!sandbox::runs_folder(&profile).exists());
        fs::set_permissions(&profile, fs::Permissions::from_mode(0o755)).unwrap();
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                false,
            ),
            text::PROFILE_FAILED,
        );
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        for path in [&scratch.0.join("missing"), &script] {
            invalid(
                Launch::new(
                    Path::new("/bin/ls"),
                    path,
                    &scratch.0,
                    RunFolder::Fresh,
                    4321,
                    false,
                ),
                text::PROFILE_FAILED,
            );
        }
    }

    #[test]
    fn tightens_the_runs_parent_and_refuses_a_link_or_file_in_its_place() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let runs = sandbox::runs_folder(&profile);
        fs::create_dir(&runs).unwrap();
        fs::set_permissions(&runs, fs::Permissions::from_mode(0o755)).unwrap();
        drop(launch(&profile, &scratch.0, RunFolder::Fresh, false));
        assert_eq!(mode(&runs), 0o700);
        fs::remove_dir(&runs).unwrap();
        symlink(&scratch.0, &runs).unwrap();
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                false,
            ),
            text::RUN_FOLDER_FAILED,
        );
        fs::remove_file(&runs).unwrap();
        fs::write(&runs, "file").unwrap();
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                false,
            ),
            text::RUN_FOLDER_FAILED,
        );
    }

    #[test]
    fn given_runs_must_be_private_directories_and_helpers_never_replace_files() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let given = scratch.0.join("given");
        private(&given);
        fs::set_permissions(&given, fs::Permissions::from_mode(0o755)).unwrap();
        let file = scratch.program("not-a-folder", b"file");
        for path in [&given, &scratch.0.join("missing"), &file] {
            invalid(
                Launch::new(
                    Path::new("/bin/ls"),
                    &profile,
                    &scratch.0,
                    RunFolder::Given(path),
                    4321,
                    false,
                ),
                text::RUN_FOLDER_FAILED,
            );
        }
        fs::set_permissions(&given, fs::Permissions::from_mode(0o700)).unwrap();
        let helper = given.join(sandbox::HELPER);
        fs::write(&helper, "keep").unwrap();
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Given(&given),
                4321,
                true,
            ),
            text::RUN_FOLDER_FAILED,
        );
        assert_eq!(fs::read(&helper).unwrap(), b"keep");
        fs::remove_file(&helper).unwrap();
        let outside = scratch.0.join("outside");
        fs::write(&outside, "keep").unwrap();
        symlink(&outside, &helper).unwrap();
        invalid(
            Launch::new(
                Path::new("/bin/ls"),
                &profile,
                &scratch.0,
                RunFolder::Given(&given),
                4321,
                true,
            ),
            text::RUN_FOLDER_FAILED,
        );
        assert_eq!(fs::read(outside).unwrap(), b"keep");
    }

    fn expected_environment(
        launch: &Launch,
        profile: &Path,
        home: &Path,
        sign_in: bool,
    ) -> BTreeMap<OsString, OsString> {
        let run = launch.run_folder();
        let lang = std::env::var("LANG")
            .ok()
            .filter(|lang| {
                !lang.is_empty()
                    && lang.len() <= 64
                    && lang
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.@-".contains(&b))
            })
            .unwrap_or_else(|| "en_US.UTF-8".into());
        let mut env: BTreeMap<OsString, OsString> = [
            ("HOME", home.as_os_str().to_owned()),
            ("PATH", "/usr/bin:/bin:/usr/sbin:/sbin".into()),
            ("LANG", lang.into()),
            ("CLAUDE_CONFIG_DIR", profile.as_os_str().to_owned()),
            ("CLAUDE_CODE_TMPDIR", run.as_os_str().to_owned()),
            ("TMPDIR", format!("{}/", run.display()).into()),
            ("HTTPS_PROXY", "http://127.0.0.1:4321".into()),
            ("https_proxy", "http://127.0.0.1:4321".into()),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".into()),
            ("DISABLE_AUTOUPDATER", "1".into()),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        let output = Command::new("/usr/bin/id").arg("-un").output().unwrap();
        assert!(output.status.success());
        let login = String::from_utf8(output.stdout).unwrap();
        let login = login.trim_end_matches('\n');
        env.insert("USER".into(), login.into());
        env.insert("LOGNAME".into(), login.into());
        if sign_in {
            env.insert("BROWSER".into(), run.join(sandbox::HELPER).into_os_string());
        }
        env
    }

    // get_envs() cannot show inherited variables; the live C-program test proves env_clear().
    #[test]
    fn command_has_exact_arguments_directory_and_environment() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let home = scratch.0.join("home as given");
        for sign_in in [false, true] {
            let launch = launch(&profile, &home, RunFolder::Fresh, sign_in);
            let command = launch.command(&["auth", "status", "--json"]);
            assert_eq!(command.get_program(), sandbox::SANDBOX_EXEC);
            assert_eq!(command.get_current_dir(), Some(launch.run_folder()));
            let expected: Vec<OsString> = vec![
                "-p".into(),
                include_str!("../sandbox/claude-code.sb").into(),
                "-D".into(),
                format!("PROGRAM={}", fs::canonicalize("/bin/ls").unwrap().display()).into(),
                "-D".into(),
                format!("PROFILE_DIR={}", profile.display()).into(),
                "-D".into(),
                format!("RUN_DIR={}", launch.run_folder().display()).into(),
                "-D".into(),
                "PROXY_PORT=4321".into(),
                "-D".into(),
                format!("SIGN_IN={}", if sign_in { "yes" } else { "no" }).into(),
                fs::canonicalize("/bin/ls").unwrap().into_os_string(),
                "--setting-sources".into(),
                "project,local".into(),
                "auth".into(),
                "status".into(),
                "--json".into(),
            ];
            assert_eq!(
                command.get_args().map(OsString::from).collect::<Vec<_>>(),
                expected
            );
            let env: BTreeMap<_, _> = command
                .get_envs()
                .map(|(k, v)| (k.to_owned(), v.expect("only explicit values").to_owned()))
                .collect();
            assert!(
                env == expected_environment(&launch, &profile, &home, sign_in),
                "child environment differs from the launch allowlist"
            );
            for key in [
                "ANTHROPIC_API_KEY",
                "CLAUDECODE",
                "SSH_AUTH_SOCK",
                "NODE_OPTIONS",
            ] {
                assert!(!env.contains_key(std::ffi::OsStr::new(key)));
            }
            assert_eq!(
                env[std::ffi::OsStr::new("HTTPS_PROXY")],
                "http://127.0.0.1:4321"
            );
            if let Some(parent_proxy) = std::env::var_os("HTTPS_PROXY")
                && parent_proxy != "http://127.0.0.1:4321"
            {
                assert!(env.get(std::ffi::OsStr::new("HTTPS_PROXY")) != Some(&parent_proxy));
            }
            let helper = launch.run_folder().join(sandbox::HELPER);
            if sign_in {
                assert_eq!(mode(&helper), 0o700);
                assert_eq!(fs::read(helper).unwrap(), HELPER.as_bytes());
            } else {
                assert!(!helper.exists());
            }
        }
    }

    #[test]
    fn language_is_validated_in_a_separate_process() {
        for (lang, expected) in [
            (Some("fr_CA.UTF-8@latin"), "fr_CA.UTF-8@latin"),
            (Some(""), "en_US.UTF-8"),
            (Some("en US"), "en_US.UTF-8"),
            (Some("en_US\n"), "en_US.UTF-8"),
            (Some("é"), "en_US.UTF-8"),
            (None, "en_US.UTF-8"),
        ] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "mac::language_child", "--nocapture"]);
            command.env("VOICE_TEST_EXPECT_LANG", expected);
            if let Some(lang) = lang {
                command.env("LANG", lang);
            } else {
                command.env_remove("LANG");
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        for (len, expected) in [(64, "a".repeat(64)), (65, "en_US.UTF-8".into())] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "mac::language_child", "--nocapture"])
                .env("LANG", "a".repeat(len))
                .env("VOICE_TEST_EXPECT_LANG", expected)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        }
    }

    #[test]
    fn language_child() {
        let Some(expected) = std::env::var_os("VOICE_TEST_EXPECT_LANG") else {
            return;
        };
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let launch = launch(&profile, &scratch.0, RunFolder::Fresh, false);
        let command = launch.command(&[]);
        let lang = command
            .get_envs()
            .find(|(name, _)| *name == "LANG")
            .unwrap()
            .1
            .unwrap();
        assert_eq!(lang, expected);
        assert!(
            !command
                .get_envs()
                .any(|(name, _)| name == "VOICE_TEST_EXPECT_LANG")
        );
    }

    #[test]
    fn hands_out_a_valid_link_once_and_refuses_unsafe_files_or_contents() {
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let launch = launch(&profile, &scratch.0, RunFolder::Fresh, true);
        let path = launch.run_folder().join(sandbox::LINK);
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::NotYet);
        let good = SIGN_IN_LINK;
        fs::write(&path, format!("{good}\n")).unwrap();
        assert_eq!(
            launch.sign_in_link(SignIn::Subscription),
            Link::Ready(good.into())
        );
        assert!(!path.exists());
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::NotYet);
        for bytes in [b"".as_slice(), good.as_bytes()] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::NotYet);
            assert_eq!(fs::read(&path).unwrap(), bytes);
            let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
            if bytes.is_empty() {
                file.write_all(good.as_bytes()).unwrap();
            }
            file.write_all(b"\n").unwrap();
            assert_eq!(
                launch.sign_in_link(SignIn::Subscription),
                Link::Ready(good.into())
            );
            assert!(!path.exists());
        }
        for bytes in [
            b"https://example.invalid/\n".to_vec(),
            vec![b'a'; sandbox::MAX_LINK + 1],
            vec![b'a'; sandbox::MAX_LINK + 10],
            vec![0xff, b'\n'],
            format!("{good}\n\n").into_bytes(),
            vec![b'\n'],
        ] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::Refused);
            assert!(!path.exists());
        }
        let mut invalid_utf8 = good.as_bytes().to_vec();
        invalid_utf8.extend_from_slice(&[0xff, b'\n']);
        fs::write(&path, invalid_utf8).unwrap();
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::Refused);
        assert!(!path.exists());
        let exact = format!("{good}{}", "x".repeat(sandbox::MAX_LINK - good.len()));
        fs::write(&path, &exact).unwrap();
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::NotYet);
        assert_eq!(fs::read(&path).unwrap(), exact.as_bytes());
        fs::write(&path, format!("{exact}\n")).unwrap();
        assert_eq!(
            launch.sign_in_link(SignIn::Subscription),
            Link::Ready(exact.clone())
        );
        fs::write(&path, format!("{exact}\nx")).unwrap();
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::Refused);
        assert!(!path.exists());
        fs::write(&path, format!("{exact}x\n")).unwrap();
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::Refused);
        assert!(!path.exists());
        let outside = scratch.0.join("outside");
        fs::write(&outside, format!("{good}\n")).unwrap();
        symlink(&outside, &path).unwrap();
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::Refused);
        assert_eq!(fs::read_to_string(&outside).unwrap(), format!("{good}\n"));
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert_eq!(launch.sign_in_link(SignIn::Subscription), Link::Refused);
    }

    #[test]
    fn a_sign_in_fifo_is_refused_without_waiting_for_a_writer() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt, sync::mpsc, thread, time::Duration};

        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let launch = launch(&profile, &scratch.0, RunFolder::Fresh, true);
        let path = launch.run_folder().join(sandbox::LINK);
        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a live, null-terminated path and the mode is valid.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let (send, receive) = mpsc::channel();
        let reader = thread::spawn(move || {
            send.send(launch.sign_in_link(SignIn::Subscription))
                .unwrap()
        });
        assert_eq!(
            receive
                .recv_timeout(Duration::from_secs(2))
                .expect("FIFO reader blocked"),
            Link::Refused,
        );
        reader.join().unwrap();
    }

    fn available() -> bool {
        let output = Command::new(sandbox::SANDBOX_EXEC)
            .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
            .output()
            .expect("run sandbox-exec availability check");
        if output.status.success() {
            return true;
        }
        let skip = "launch: skipped, already inside a sandbox (macOS refuses a sandbox inside one)";
        assert!(
            std::env::var("VOICE_REQUIRE_SANDBOX").as_deref() != Ok("1"),
            "{skip}"
        );
        eprintln!("{skip}");
        false
    }

    #[test]
    fn real_sandbox_keeps_the_environment_and_hands_over_only_the_sign_in_link() {
        if !available() {
            return;
        }
        match Command::new("cc").arg("--version").output() {
            Ok(output) if output.status.success() => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("launch: skipped, cc is not installed");
                return;
            }
            other => panic!("cc availability check failed: {other:?}"),
        }
        let scratch = Scratch::new();
        let profile = profile(&scratch);
        let source = scratch.0.join("child.c");
        let program = scratch.0.join("child");
        fs::write(&source, r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
extern char **environ;
int main(int argc, char **argv) {
    char path[8192];
    snprintf(path, sizeof(path), "%s/seen", getenv("CLAUDE_CODE_TMPDIR"));
    FILE *file = fopen(path, "w");
    if (!file) return 2;
    for (int i = 0; i < argc; i++) fprintf(file, "%s\n", argv[i]);
    fputs("--environment--\n", file);
    for (char **entry = environ; *entry; entry++) fprintf(file, "%s\n", *entry);
    if (fclose(file)) return 3;
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "login")) {
            execl("/bin/bash", "bash", "-c", "\"$BROWSER\" 'https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A59244%2Fcallback&scope=user%3Aprofile&code_challenge=x&code_challenge_method=S256&state=y'", NULL);
            return 4;
        }
    }
    return 0;
}
"#).unwrap();
        let output = Command::new("cc")
            .arg(&source)
            .arg("-o")
            .arg(&program)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        for sign_in in [true, false] {
            let launch = Launch::new(
                &program,
                &profile,
                &scratch.0,
                RunFolder::Fresh,
                4321,
                sign_in,
            )
            .unwrap();
            let output = launch.command(&["auth", "login"]).output().unwrap();
            let seen = fs::read_to_string(launch.run_folder().join("seen")).unwrap();
            let (args, environment) = seen.split_once("--environment--\n").unwrap();
            assert_eq!(
                args.lines().collect::<Vec<_>>(),
                [
                    program.to_str().unwrap(),
                    "--setting-sources",
                    "project,local",
                    "auth",
                    "login"
                ]
            );
            let env: BTreeMap<OsString, OsString> = environment
                .lines()
                .map(|line| {
                    let (k, v) = line.split_once('=').unwrap();
                    (k.into(), v.into())
                })
                .collect();
            assert_eq!(environment.lines().count(), env.len());
            assert!(
                env == expected_environment(&launch, &profile, &scratch.0, sign_in),
                "child environment differs from the launch allowlist"
            );
            if sign_in {
                assert!(output.status.success(), "{output:?}");
                assert_eq!(mode(&launch.run_folder().join(sandbox::LINK)), 0o600);
                assert_eq!(
                    launch.sign_in_link(SignIn::Subscription),
                    Link::Ready(SIGN_IN_LINK.into())
                );
            } else {
                assert_eq!(output.status.code(), Some(4), "{output:?}");
                assert!(!launch.run_folder().join(sandbox::LINK).exists());
            }
        }
    }
}

#[test]
fn sign_in_links_are_specific_to_the_chosen_account_kind() {
    let console = SIGN_IN_LINK.replace(
        "https://claude.com/cai/oauth/authorize?",
        "https://platform.claude.com/oauth/authorize?",
    );
    assert!(sandbox::valid_link(&console, SignIn::Console));
    assert!(!sandbox::valid_link(&console, SignIn::Subscription));
    for prefix in [
        "https://claude.com/cai/oauth/authorize?",
        "https://claude.ai/oauth/authorize?",
    ] {
        let subscription = SIGN_IN_LINK.replace("https://claude.com/cai/oauth/authorize?", prefix);
        assert!(sandbox::valid_link(&subscription, SignIn::Subscription));
        assert!(!sandbox::valid_link(&subscription, SignIn::Console));
    }
    for bad in [
        console.replace(
            "client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e",
            "client_id=other",
        ),
        console.replace("localhost", "example.com"),
        format!("{console}#fragment"),
        format!("{console}&state=duplicate"),
    ] {
        assert!(!sandbox::valid_link(&bad, SignIn::Console));
    }
}
