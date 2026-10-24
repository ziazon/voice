#![allow(dead_code)]

use persona_builder_agent::{
    claude::{Call, Claude},
    ledger::{Cap, Clock, Session},
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "persona-builder-agent-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path.canonicalize().unwrap()),
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

/// The part of every stub that records how it was run, beside itself.
const RECORD: &str = r#"#!/bin/sh
here="$(cd "$(dirname "$0")" && pwd -P)"
n=$(ls "$here" | grep -c '^argv')
for a in "$@"; do printf '%s\n' "$a"; done > "$here/argv$n"
pwd -P > "$here/cwd"
cat > "$here/stdin"
if [ -n "${CLAUDECODE+x}" ]; then echo set; else echo unset; fi > "$here/claudecode"
if [ -n "${CLAUDE_CODE_ENTRYPOINT+x}" ]; then echo set; else echo unset; fi > "$here/entrypoint"
printf '%s' "${PB_AGENT_TEST_KEEP:-}" > "$here/keep"
"#;

/// A stub `claude` in a folder of its own: `body` runs after the recording part.
pub struct Stub {
    pub folder: Scratch,
}

impl Stub {
    pub fn new(body: &str) -> Self {
        let folder = Scratch::new("stub");
        let program = folder.0.join("claude");
        fs::write(&program, format!("{RECORD}{body}\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        Stub { folder }
    }
    /// A stub that prints `reply` and exits with `code`.
    pub fn replying(reply: &str, code: i32) -> Self {
        let stub = Self::new("cat \"$here/reply\"\nexit \"$(cat \"$here/code\")\"");
        fs::write(stub.folder.0.join("reply"), reply).unwrap();
        fs::write(stub.folder.0.join("code"), code.to_string()).unwrap();
        stub
    }
    pub fn program(&self) -> PathBuf {
        self.folder.0.join("claude")
    }
    pub fn claude(&self) -> Claude {
        Claude::at(self.program())
    }
    /// The argv of the stub's `n`th run, or `None` when it never ran that often.
    pub fn argv(&self, n: usize) -> Option<Vec<String>> {
        let text = fs::read_to_string(self.folder.0.join(format!("argv{n}"))).ok()?;
        Some(text.lines().map(str::to_owned).collect())
    }
    pub fn ran(&self) -> bool {
        self.argv(0).is_some()
    }
    pub fn file(&self, name: &str) -> String {
        fs::read_to_string(self.folder.0.join(name)).unwrap()
    }
}

/// An `ok` reply carrying `output`, the usage and the cost.
pub fn ok_reply(output: &Value) -> String {
    json!({
        "type": "result", "subtype": "success", "is_error": false, "result": "",
        "session_id": "s-1", "total_cost_usd": 0.0123,
        "usage": {"input_tokens": 1200, "output_tokens": 300, "cache_creation_input_tokens": 7, "cache_read_input_tokens": 9},
        "structured_output": output,
    })
    .to_string()
}

pub fn fixed_time() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_000_000)
}

pub fn fixed_clock() -> Clock {
    Box::new(fixed_time)
}

pub fn session(home: &Path) -> Session {
    Session::open_with_clock(home, "wren", Cap::default(), fixed_clock()).unwrap()
}

pub fn state(home: &Path) -> PathBuf {
    home.join(".claude/state/wren")
}

pub fn ledger(home: &Path) -> Option<String> {
    fs::read_to_string(state(home).join("runs.jsonl")).ok()
}

pub fn ledger_lines(home: &Path) -> Vec<Value> {
    ledger(home)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

pub fn schema() -> Value {
    json!({"type": "object", "properties": {"labels": {"type": "array"}}, "required": ["labels"]})
}

pub fn call<'a>(prompt: &'a str, schema: &'a Value) -> Call<'a> {
    Call {
        step: "label",
        system: "Label each record.",
        prompt,
        model: "sonnet",
        schema,
    }
}

/// A ledger line for `day` costing `cost`.
pub fn spent_line(day: &str, cost: f64) -> String {
    json!({"at": format!("{day}T12:00:00Z"), "day": day, "agent": "claude-code", "step": "label",
           "model": "sonnet", "outcome": "ok", "seconds": 1.0, "input_tokens": 1, "output_tokens": 1,
           "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0, "cost_usd": cost,
           "cost_from": "claude"})
    .to_string()
}

pub fn seed_ledger(home: &Path, lines: &[String]) {
    let state = state(home);
    fs::create_dir_all(&state).unwrap();
    fs::set_permissions(home.join(".claude"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(
        home.join(".claude/state"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    let mut text = lines.join("\n");
    text.push('\n');
    fs::write(state.join("runs.jsonl"), text).unwrap();
    fs::set_permissions(state.join("runs.jsonl"), fs::Permissions::from_mode(0o600)).unwrap();
}
