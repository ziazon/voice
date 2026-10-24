//! The person's own Claude Code (PB-D9): finding it, checking it is signed in, and one tool-less
//! call. Persona Builder never logs in for anyone and never reads, sets or passes a token.
//!
//! Sources, read 2026-10-24: `https://code.claude.com/docs/en/headless` (the JSON reply, its
//! `structured_output`, the 10 MB stdin cap, and that without `--bare` a `-p` run loads the working
//! folder's `.claude/settings.json` and `.mcp.json`; `--bare` is not used because it never reads the
//! person's own login), `claude --help` 2.1.292 (`--safe-mode`, `--tools ""`, `--max-budget-usd`,
//! `auth status --json`), and `https://code.claude.com/docs/en/setup` (where the installers put it).
use crate::{
    child::{self, Ran},
    ledger::{Line, Session},
    text,
};
use persona_builder::error::{Error, Result};
use serde_json::Value;
use std::{
    ffi::OsStr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

pub const AGENT_ID: &str = "claude-code";
pub const DEFAULT_MODEL: &str = "sonnet";
/// How long one call may take before it is killed: a 300-record labelling step can take minutes.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// The largest prompt, under the CLI's 10 MB stdin cap.
pub const MAX_PROMPT: usize = 8 * 1024 * 1024;
pub const MAX_SYSTEM: usize = 32 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const CHECK_CAP: u64 = 64 * 1024;
const STDOUT_CAP: u64 = 16 * 1024 * 1024;
/// Set by Claude Code in the sessions it runs; Persona Builder's calls run as a fresh, top-level CLI.
const SESSION_MARKERS: [&str; 2] = ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"];

/// One call: what it is for (`step`, kept in the ledger), the instructions, the person's records,
/// the model and the shape the answer must have.
pub struct Call<'a> {
    pub step: &'a str,
    pub system: &'a str,
    pub prompt: &'a str,
    pub model: &'a str,
    pub schema: &'a Value,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

/// A call's answer. `output` is untrusted: the caller checks it.
#[derive(Debug)]
pub struct Reply {
    pub output: Value,
    pub usage: Usage,
    pub cost_usd: f64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Ready {
    pub version: String,
    /// `ANTHROPIC_API_KEY` is set (its value is never shown or kept): Claude Code may bill that key.
    pub api_key_set: bool,
}

#[derive(Debug)]
pub struct Claude {
    program: PathBuf,
    timeout: Duration,
}

/// The places Claude Code is looked for after `PATH`: an app opened from the Finder gets a short
/// PATH. The native installer's launcher, then Homebrew's on Apple silicon and on Intel.
pub fn fallbacks(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".local/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ]
}

/// Claude Code on `PATH`, else in the usual places.
pub fn find(path: Option<&OsStr>, home: &Path) -> Result<Claude> {
    find_in(path, &fallbacks(home))
}

/// The first `claude` in the absolute folders of `path`, then in `fallbacks`. Relative and empty
/// `PATH` entries are skipped, so a `.` in PATH never runs a `claude` from the current folder.
pub fn find_in(path: Option<&OsStr>, fallbacks: &[PathBuf]) -> Result<Claude> {
    let on_path = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|folder| folder.is_absolute())
        .map(|folder| folder.join("claude"));
    on_path
        .chain(fallbacks.iter().cloned())
        .find(|candidate| runnable(candidate))
        .map(Claude::at)
        .ok_or_else(|| Error::Invalid(text::NOT_INSTALLED.into()))
}

/// A regular file with an execute bit, links followed (the installers' launchers are links).
fn runnable(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Lower-case letters, digits, dots and dashes, starting with a letter or digit, at most 64.
fn valid_model(model: &str) -> bool {
    let mut chars = model.chars();
    model.len() <= 64
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.'))
}

/// A step's name for the ledger: a lower-case letter, then up to 39 letters, digits or dashes.
fn valid_step(step: &str) -> bool {
    let mut chars = step.chars();
    step.len() <= 40
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `2.1.292`: two to four dot-separated runs of digits.
fn valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    (2..=4).contains(&parts.len())
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The prompt's first character that is neither white space nor an invisible format character
/// (a byte-order mark, a zero-width space or joiner, a word joiner, a soft hyphen) is `/`.
fn opens_with_slash(prompt: &str) -> bool {
    let invisible = |c: char| matches!(c, '\u{00ad}' | '\u{feff}' | '\u{200b}'..='\u{200f}' | '\u{2060}'..='\u{2064}');
    prompt
        .chars()
        .find(|&c| !c.is_whitespace() && !invisible(c))
        == Some('/')
}

/// The failures `claude` reports when its login is missing or expired: they name `/login` or
/// authenticating.
fn signed_out(result: &str) -> bool {
    result.contains("/login")
        || result.contains("authenticate")
        || result.starts_with("Failed to authenticate")
}

fn count(usage: Option<&Value>, key: &str) -> u64 {
    usage
        .and_then(|usage| usage.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn usage_of(reply: &Value) -> Usage {
    let usage = reply.get("usage");
    Usage {
        input_tokens: count(usage, "input_tokens"),
        output_tokens: count(usage, "output_tokens"),
        cache_creation_input_tokens: count(usage, "cache_creation_input_tokens"),
        cache_read_input_tokens: count(usage, "cache_read_input_tokens"),
    }
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.to_owned())
}

/// What a finished call came to: its ledger fields and what the caller gets.
struct Judged {
    outcome: &'static str,
    usage: Usage,
    cost: Option<f64>,
    result: Result<Reply>,
}

fn judge(ran: Ran, timeout: Duration) -> Option<Judged> {
    let (status, stdout) = match ran {
        Ran::NotStarted => return None,
        Ran::CutOff => {
            return Some(Judged {
                outcome: "cut-off",
                usage: Usage::default(),
                cost: None,
                result: Err(Error::Invalid(text::cut_off(
                    timeout.as_secs().div_ceil(60).max(1),
                ))),
            });
        }
        Ran::Finished { status, stdout } => (status, stdout),
    };
    let reply = match serde_json::from_slice::<Value>(&stdout) {
        Ok(reply) if reply.is_object() => reply,
        _ => {
            return Some(Judged {
                outcome: "unreadable",
                usage: Usage::default(),
                cost: None,
                result: Err(invalid(text::FAILED)),
            });
        }
    };
    let usage = usage_of(&reply);
    let cost = reply
        .get("total_cost_usd")
        .and_then(Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0);
    let failed = |outcome, message: &str| Judged {
        outcome,
        usage,
        cost,
        result: Err(invalid(message)),
    };
    let is_error = reply.get("is_error").and_then(Value::as_bool);
    if is_error == Some(true)
        && reply
            .get("result")
            .and_then(Value::as_str)
            .is_some_and(signed_out)
    {
        return Some(failed("signed-out", text::SIGNED_OUT));
    }
    if !status.success() || is_error != Some(false) {
        return Some(failed("error", text::FAILED));
    }
    // The headless docs: a run that ends without valid structured output has no such field.
    let Some(output) = reply
        .get("structured_output")
        .filter(|output| output.is_object() || output.is_array())
    else {
        return Some(failed("no-output", text::FAILED));
    };
    Some(Judged {
        outcome: "ok",
        usage,
        cost,
        result: Ok(Reply {
            output: output.clone(),
            usage,
            cost_usd: cost.unwrap_or_default(),
        }),
    })
}

impl Claude {
    pub fn at(program: PathBuf) -> Claude {
        Claude {
            program,
            timeout: DEFAULT_TIMEOUT,
        }
    }
    pub fn with_timeout(self, timeout: Duration) -> Claude {
        Claude { timeout, ..self }
    }
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// `claude` in `cwd`, with the person's own environment minus the session markers.
    fn command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(&self.program);
        command.current_dir(cwd);
        for marker in SESSION_MARKERS {
            command.env_remove(marker);
        }
        command
    }

    fn ask(&self, home: &Path, args: &[&str]) -> Option<Vec<u8>> {
        let mut command = self.command(home);
        command.args(args);
        match child::run(
            &mut command,
            None,
            CHECK_TIMEOUT.min(self.timeout),
            CHECK_CAP,
        ) {
            Ran::Finished { status, stdout } if status.success() => Some(stdout),
            _ => None,
        }
    }

    /// Claude Code runs and is signed in. Only `loggedIn` is read from the status: the object can
    /// carry an email address, which is never kept or shown.
    pub fn check(&self, home: &Path) -> Result<Ready> {
        let stdout = self
            .ask(home, &["--version"])
            .ok_or_else(|| invalid(text::DOES_NOT_RUN))?;
        let version = String::from_utf8_lossy(&stdout)
            .split_whitespace()
            .next()
            .filter(|version| valid_version(version))
            .map(str::to_owned)
            .ok_or_else(|| invalid(text::DOES_NOT_RUN))?;
        let logged_in = self
            .ask(home, &["auth", "status", "--json"])
            .and_then(|stdout| serde_json::from_slice::<Value>(&stdout).ok())
            .and_then(|status| status.get("loggedIn").and_then(Value::as_bool));
        if logged_in != Some(true) {
            return Err(invalid(text::SIGNED_OUT));
        }
        Ok(Ready {
            version,
            api_key_set: std::env::var_os("ANTHROPIC_API_KEY").is_some_and(|key| !key.is_empty()),
        })
    }

    /// One tool-less call, spent against the session's cap and recorded in its ledger.
    pub fn call(&self, session: &Session, call: &Call) -> Result<Reply> {
        if !valid_step(call.step) {
            return Err(invalid(
                "a step's name must be a lower-case letter, then up to 39 letters, digits or dashes",
            ));
        }
        if !valid_model(call.model) {
            return Err(invalid(
                "the model name takes lower-case letters, digits, dots and dashes",
            ));
        }
        if call.prompt.trim().is_empty() || call.prompt.len() > MAX_PROMPT {
            return Err(invalid("a step's records must be there and at most 8 MiB"));
        }
        // `claude -p` expands a prompt that opens with `/` as a command (`/model`, `/config`),
        // and the built-in ones work under `--safe-mode`: records never open a prompt that way.
        if opens_with_slash(call.prompt) {
            return Err(invalid("a step's records can't begin with /"));
        }
        if call.system.trim().is_empty() || call.system.len() > MAX_SYSTEM {
            return Err(invalid(
                "a step's instructions must be there and at most 32 KiB",
            ));
        }
        if !call.schema.is_object() {
            return Err(invalid("a step's answer shape must be a JSON object"));
        }
        let folder = session.work_folder()?;
        let started = session.start(AGENT_ID, call.step, call.model)?;
        let schema = call.schema.to_string();
        let budget = started.budget();
        let budget_arg = format!("{budget:.2}");
        let mut command = self.command(&folder);
        command.args([
            "-p",
            "--system-prompt",
            call.system,
            "--model",
            call.model,
            "--safe-mode",
            "--no-session-persistence",
            "--tools",
            "",
            "--output-format",
            "json",
            "--json-schema",
            &schema,
            "--setting-sources",
            "project,local",
            "--strict-mcp-config",
            "--mcp-config",
            r#"{"mcpServers":{}}"#,
            "--permission-mode",
            "dontAsk",
            "--max-budget-usd",
            &budget_arg,
        ]);
        let begun = Instant::now();
        let ran = child::run(
            &mut command,
            Some(call.prompt.as_bytes()),
            self.timeout,
            STDOUT_CAP,
        );
        let seconds = begun.elapsed().as_secs_f64();
        let Some(judged) = judge(ran, self.timeout) else {
            // A child that never started spent nothing.
            session.discard(started)?;
            return Err(invalid(text::DOES_NOT_RUN));
        };
        let (cost_usd, cost_from) = match judged.cost {
            Some(cost) => (cost, "claude"),
            // Money may have been spent: count the whole budget, so the cap never under-counts.
            None => (budget, "budget"),
        };
        let line = Line {
            agent: AGENT_ID,
            step: call.step,
            model: call.model,
            outcome: judged.outcome,
            seconds,
            usage: judged.usage,
            cost_usd,
            cost_from,
        };
        session.finish(started, &line)?;
        judged.result
    }
}
