//! The build's steps through the `persona-builder` binary against the real Python, on the invented
//! members and files `tests/crew_steps_fixtures.py` writes: the same printed lines, exit codes and
//! files, byte for byte.
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn python_available() -> bool {
    if Command::new("python3").arg("--version").output().is_ok() {
        return true;
    }
    if std::env::var("PERSONA_BUILDER_REQUIRE_PARITY").as_deref() == Ok("1") {
        panic!("parity tools missing: python3 not on PATH");
    }
    eprintln!("skipped: python3 not found");
    false
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

/// Every file under a folder, by its name, with its bytes.
fn files(folder: &Path) -> Vec<(String, Vec<u8>)> {
    let mut found: Vec<_> = fs::read_dir(folder)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().into_string().unwrap(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    found.sort();
    found
}

fn compare(case: &str, who: &str) {
    let home = std::env::temp_dir().join(format!(
        "pb-steps-parity-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&home).unwrap();
    let scratch = Scratch(home.canonicalize().unwrap());
    let home = &scratch.0;
    let state = home.join(".claude/state").join(who);
    let fixture = Command::new("python3")
        .arg("-I")
        .arg(repo().join("tests/crew_steps_fixtures.py"))
        .args([case, who])
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    let member_dir = home.join("crew").join(who);
    fs::create_dir_all(&member_dir).unwrap();
    fs::rename(state.join("member.json"), member_dir.join("member.json")).unwrap();
    let runs: Vec<Value> = serde_json::from_slice(&fixture.stdout).unwrap();
    assert!(!runs.is_empty());
    for run in runs {
        let cmd = run["cmd"].as_str().unwrap();
        let mut py_args = Vec::new();
        let mut rs_args = Vec::new();
        let mut outs = Vec::new();
        for arg in run["args"].as_array().unwrap() {
            let arg = arg.as_str().unwrap();
            if let Some(name) = arg.strip_prefix("@in:") {
                py_args.push(state.join(name).into_os_string());
                rs_args.push(name.into());
            } else if let Some(name) = arg.strip_prefix("@doc:") {
                py_args.push(state.join(name).into_os_string());
                rs_args.push(state.join(name).into_os_string());
            } else if let Some(name) = arg.strip_prefix("@out:") {
                py_args.push(state.join(format!("py-{name}")).into_os_string());
                rs_args.push(name.into());
                outs.push(name.to_owned());
            } else {
                py_args.push(arg.into());
                rs_args.push(arg.into());
            }
        }
        let python = Command::new("python3")
            .arg("-I")
            .arg(repo().join("scripts/crew_decisions.py"))
            .arg("--member")
            .arg(member_dir.join("member.json"))
            .arg(cmd)
            .args(&py_args)
            .env("HOME", home)
            .env("PYTHONHASHSEED", "0")
            .current_dir(home)
            .output()
            .unwrap();
        let rust: Output = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env_clear()
            .env("HOME", home)
            .env("PATH", std::env::var_os("PATH").unwrap())
            .current_dir(home)
            .arg(cmd)
            .arg(&member_dir)
            .args(&rs_args)
            .output()
            .unwrap();
        let what = format!("{case} {who} {cmd} {:?}", run["args"]);
        assert_eq!(
            rust.status.code(),
            python.status.code(),
            "{what}: exit codes; rust stderr: {}",
            String::from_utf8_lossy(&rust.stderr)
        );
        let stdout = String::from_utf8(rust.stdout).unwrap();
        let saved = format!("saved to ~/.claude/state/{who}/");
        let stdout = match stdout.trim_end_matches('\n').rsplit_once('\n') {
            Some((before, last)) if last.starts_with(&saved) => format!("{before}\n"),
            None if stdout.starts_with(&saved) => String::new(),
            _ => stdout,
        };
        assert_eq!(
            stdout,
            String::from_utf8(python.stdout).unwrap(),
            "{what}: printed lines"
        );
        for out in outs {
            let (ours, theirs) = (state.join(&out), state.join(format!("py-{out}")));
            if theirs.is_dir() {
                assert_eq!(mode(&ours), 0o700, "{what}: {out} folder mode");
                assert_eq!(files(&ours), files(&theirs), "{what}: {out} files");
                for (name, _) in files(&ours) {
                    assert_eq!(mode(&ours.join(name)), 0o600, "{what}: {out} file mode");
                }
            } else if theirs.exists() {
                assert_eq!(mode(&ours), 0o600, "{what}: {out} mode");
                assert_eq!(
                    fs::read(&ours).unwrap(),
                    fs::read(&theirs).unwrap(),
                    "{what}: {out} bytes"
                );
            } else {
                assert!(
                    !ours.exists(),
                    "{what}: {out} written where the Python wrote nothing"
                );
            }
        }
    }
}

#[test]
fn every_step_matches_the_python() {
    if !python_available() {
        return;
    }
    for case in ["base", "empty", "shares"] {
        for who in ["ada", "ari"] {
            compare(case, who);
        }
    }
}
