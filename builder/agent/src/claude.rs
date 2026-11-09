//! The person's own Claude Code (PB-D9): finding it, checking it is signed in, and one tool-less
//! call. Voice starts Claude Code's own `auth login --console` off macOS; `--claudeai` is only in
//! testers' builds (VO-D10). Voice never reads, sets or passes a token.
//! Claude Code keeps its own login file in Voice's private profile, excluded from Time Machine.
//! Source, read 2026-11-08: Claude Code 2.1.292's own bundle and `claude auth login --help`
//! (Console uses API billing; subscription authMethod is `claude.ai` or `oauth_token`).
//!
//! Every `claude` runs inside the sandbox (`sandbox::Launch`) with a proxy of its own
//! (`egress::Egress`) and an environment built from nothing, so no credential, provider switch
//! or endpoint of the person's reaches it; see `docs/spikes/VO11-os-sandbox.md`.
//!
//! Sources, read 2026-10-24: `https://code.claude.com/docs/en/headless` (the JSON reply, its
//! `structured_output`, the 10 MB stdin cap, and that without `--bare` a `-p` run loads the working
//! folder's `.claude/settings.json` and `.mcp.json`; `--bare` is not used because it never reads the
//! person's own login), `claude --help` 2.1.292 (`--safe-mode`, `--tools ""`, `--max-budget-usd`,
//! `auth status --json`), and `https://code.claude.com/docs/en/setup` (where the installers put it).
//! Sources, read 2026-10-26: `https://code.claude.com/docs/en/agent-sdk/python` (`RateLimitEvent`),
//! and `https://code.claude.com/docs/en/headless` (stream-json).
use crate::{
    child::{self, Ran},
    egress::Egress,
    guard::Sealed,
    ledger::{Line, Session},
    provider::{Provider, Reach, Reply, Usage, check_brief},
    sandbox::{Launch, Link, RunFolder},
    text,
};
use serde_json::Value;
use std::{
    ffi::OsStr,
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Once,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use voice_core::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    Console,
    Subscription,
}

pub const SUBSCRIPTION_OFFERED: bool = cfg!(feature = "claude-subscription");

/// Whether the platform allows the requested sign-in to start.
pub fn starts(sign_in: SignIn) -> bool {
    !cfg!(target_os = "macos") || sign_in != SignIn::Console
}

fn login_args(sign_in: SignIn) -> Option<[&'static str; 3]> {
    match sign_in {
        SignIn::Console => Some(["auth", "login", "--console"]),
        #[cfg(feature = "claude-subscription")]
        SignIn::Subscription => Some(["auth", "login", "--claudeai"]),
        #[cfg(not(feature = "claude-subscription"))]
        SignIn::Subscription => None,
    }
}

pub const AGENT_ID: &str = "claude-code";
pub const AGENT_NAME: &str = "Claude, through your own Claude Code";
pub const REACH: Reach = Reach::Anthropic;
pub const DEFAULT_MODEL: &str = "sonnet";
/// How long one call may take before it is killed: a 300-record labelling step can take minutes.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
pub use crate::provider::{MAX_PROMPT, MAX_SYSTEM};
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const CHECK_CAP: u64 = 64 * 1024;
const STDOUT_CAP: u64 = 16 * 1024 * 1024;
#[derive(Debug, PartialEq, Eq)]
pub struct Ready {
    pub version: String,
}

#[derive(Debug)]
pub struct Claude {
    program: PathBuf,
    profile: PathBuf,
    timeout: Duration,
    stop: Option<Arc<AtomicBool>>,
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
    find_in(home, path, &fallbacks(home))
}

/// The first `claude` in the absolute folders of `path`, then in `fallbacks`. Relative and empty
/// `PATH` entries are skipped, so a `.` in PATH never runs a `claude` from the current folder.
pub fn find_in(home: &Path, path: Option<&OsStr>, fallbacks: &[PathBuf]) -> Result<Claude> {
    find_in_impl(
        home,
        path,
        fallbacks,
        #[cfg(target_os = "macos")]
        Path::new("/usr/bin/tmutil"),
    )
}

/// Find with a backup-exclusion program supplied by a test; production always uses tmutil.
#[cfg(all(target_os = "macos", feature = "test-stubs"))]
pub fn find_in_with_tmutil(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    tmutil: &Path,
) -> Result<Claude> {
    find_in_impl(home, path, fallbacks, tmutil)
}

fn find_in_impl(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    #[cfg(target_os = "macos")] tmutil: &Path,
) -> Result<Claude> {
    let on_path = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|folder| folder.is_absolute())
        .map(|folder| folder.join("claude"));
    let program = on_path
        .chain(fallbacks.iter().cloned())
        .find(|candidate| runnable(candidate))
        .ok_or_else(|| Error::Invalid(text::NOT_INSTALLED.into()))?;
    let profile = ensure_profile(
        home,
        #[cfg(target_os = "macos")]
        tmutil,
    )?;
    Ok(Claude::at(program, profile))
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
    plan: Option<&'static str>,
    usage: Usage,
    cost: Option<f64>,
    result: Result<Reply>,
}

fn judge(ran: Ran, timeout: Duration) -> Option<Judged> {
    let (status, stdout) = match ran {
        Ran::NotStarted => return None,
        Ran::Stopped => {
            return Some(Judged {
                outcome: "stopped",
                plan: None,
                usage: Usage::default(),
                cost: None,
                result: Err(invalid(text::STOPPED)),
            });
        }
        Ran::CutOff => {
            return Some(Judged {
                outcome: "cut-off",
                plan: None,
                usage: Usage::default(),
                cost: None,
                result: Err(Error::Invalid(text::cut_off(
                    timeout.as_secs().div_ceil(60).max(1),
                ))),
            });
        }
        Ran::Finished { status, stdout } => (status, stdout),
    };
    let mut plan = None;
    let mut reply = None;
    let mut readable = true;
    for raw in stdout.split(|&byte| byte == b'\n') {
        if raw.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let line = match serde_json::from_slice::<Value>(raw) {
            Ok(line) if line.is_object() => line,
            _ => {
                readable = false;
                continue;
            }
        };
        match line.get("type").and_then(Value::as_str) {
            Some("rate_limit_event") => {
                match line
                    .get("rate_limit_info")
                    .and_then(|info| info.get("status"))
                    .and_then(Value::as_str)
                {
                    Some("rejected") => plan = Some("rejected"),
                    Some("allowed_warning") if plan.is_none() => plan = Some("allowed_warning"),
                    _ => {}
                }
            }
            Some("result") => reply = Some(line),
            _ => {}
        }
    }
    let reply = reply.filter(|_| readable);
    let usage = reply.as_ref().map(usage_of).unwrap_or_default();
    let cost = reply
        .as_ref()
        .and_then(|reply| reply.get("total_cost_usd"))
        .and_then(Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0);
    let failed = |outcome, message: &str| {
        let (outcome, message) = if plan == Some("rejected")
            && matches!(outcome, "error" | "no-output" | "unreadable")
        {
            ("plan-limit", text::PLAN_STOPPED)
        } else {
            (outcome, message)
        };
        Judged {
            outcome,
            plan,
            usage,
            cost,
            result: Err(invalid(message)),
        }
    };
    let Some(reply) = reply else {
        return Some(failed("unreadable", text::FAILED));
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
        plan,
        usage,
        cost,
        result: Ok(Reply {
            output: output.clone(),
            usage,
            cost_usd: cost.unwrap_or_default(),
        }),
    })
}

/// Voice's own Claude Code profile (`CLAUDE_CONFIG_DIR`): its sign-in and settings,
/// apart from the person's own `~/.claude`.
pub fn profile_folder(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    let folder = "Library/Application Support/Voice/claude-code";
    #[cfg(not(target_os = "macos"))]
    let folder = ".local/share/voice/claude-code";
    home.join(folder)
}

fn ensure_profile(home: &Path, #[cfg(target_os = "macos")] tmutil: &Path) -> Result<PathBuf> {
    let folder = profile_folder(home);
    let app = folder.parent().expect("profile has an app folder");
    let prepare = || -> std::io::Result<()> {
        fs::create_dir_all(app.parent().expect("app has a parent"))?;
        for path in [app, folder.as_path()] {
            let metadata = match fs::symlink_metadata(path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    match fs::DirBuilder::new().mode(0o700).create(path) {
                        Ok(()) => (),
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                        Err(error) => return Err(error),
                    }
                    fs::symlink_metadata(path)?
                }
                Err(error) => return Err(error),
            };
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(std::io::Error::other("not a directory"));
            }
            if metadata.permissions().mode() & 0o077 != 0 {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            }
        }
        Ok(())
    };
    prepare().map_err(|_| invalid(text::PROFILE_FAILED))?;
    #[cfg(target_os = "macos")]
    exclude_from_backup(&folder, tmutil);
    Ok(folder)
}

/// Best effort: a failed exclusion stays silent and is tried again at the next find.
#[cfg(target_os = "macos")]
fn exclude_from_backup(folder: &Path, tmutil: &Path) {
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt, process::CommandExt},
    };
    let Ok(path) = CString::new(folder.as_os_str().as_bytes()) else {
        return;
    };
    // SAFETY: both strings live throughout the call. Null with size zero asks for the
    // attribute's size only: Voice never reads any attribute contents.
    if unsafe {
        libc::getxattr(
            path.as_ptr(),
            c"com.apple.metadata:com_apple_backup_excludeItem".as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            0,
        )
    } >= 0
    {
        return;
    }
    // VF3 fact 5, measured 2026-11-09: addexclusion without -p sets this attribute
    // without admin rights. Exclude only Claude Code's profile, never the whole app folder.
    let Ok(mut child) = Command::new(tmutil)
        .arg("addexclusion")
        .arg(folder)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
    else {
        return;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if started.elapsed() < Duration::from_secs(2) => {
                thread::sleep(Duration::from_millis(20))
            }
            _ => {
                // SAFETY: the unreaped child leads its own process group.
                unsafe {
                    libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
                }
                let _ = child.wait();
                return;
            }
        }
    }
}

// Claude Code 2.1.292's bundle, read 2026-11-08: Console is api_key; both
// claude.ai and setup-token's oauth_token are subscriptions (VO-D10).
fn sign_in_status(status: &Value) -> (bool, bool) {
    if status.get("loggedIn").and_then(Value::as_bool) != Some(true) {
        return (false, false);
    }
    match status.get("authMethod").and_then(Value::as_str) {
        Some("api_key") => (true, false),
        Some("claude.ai" | "oauth_token") => (SUBSCRIPTION_OFFERED, !SUBSCRIPTION_OFFERED),
        _ => (false, false),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Status {
    pub version: String,
    pub signed_in: bool,
    pub subscription_refused: bool,
}

/// How long Voice waits for the person to finish signing in.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

impl Claude {
    pub fn profile(&self) -> &Path {
        &self.profile
    }

    fn launch(&self, home: &Path, sign_in: bool) -> Result<(Egress, Launch)> {
        let egress = Egress::start().map_err(|_| invalid(text::PROXY_FAILED))?;
        let launch = Launch::new(
            &self.program,
            &self.profile,
            home,
            RunFolder::Fresh,
            egress.port(),
            sign_in,
        )?;
        Ok((egress, launch))
    }

    /// Wait for Claude Code's login inside the sandbox; the sign-in page opens in the browser.
    pub fn sign_in(&self, home: &Path, stop: &AtomicBool, sign_in: SignIn) -> Result<()> {
        self.sign_in_with(home, stop, sign_in, &open_in_browser)
    }

    /// `sign_in`, with the opener given (tests).
    pub fn sign_in_with(
        &self,
        home: &Path,
        stop: &AtomicBool,
        sign_in: SignIn,
        open: &(dyn Fn(&str) -> std::io::Result<()> + Sync),
    ) -> Result<()> {
        // VF3 fact 4, measured with Claude Code 2.1.292 on 2026-11-09: Console creates
        // an API key, then fails to save it while the Keychain is closed. Wait for VF5.
        if !starts(sign_in) {
            return Err(invalid(text::CONSOLE_NOT_AVAILABLE));
        }
        let args = login_args(sign_in).ok_or_else(|| invalid(text::SUBSCRIPTION_NOT_OFFERED))?;
        let (egress, launch) = self.launch(home, true)?;
        let mut command = launch.command(&args);
        let abort = AtomicBool::new(false);
        let done = AtomicBool::new(false);
        let refused = AtomicBool::new(false);
        let ran = thread::scope(|scope| {
            scope.spawn(|| {
                let mut opened = false;
                while !done.load(Ordering::SeqCst) {
                    if stop.load(Ordering::SeqCst) {
                        abort.store(true, Ordering::SeqCst);
                        break;
                    }
                    if !opened {
                        match launch.sign_in_link(sign_in) {
                            Link::NotYet => (),
                            Link::Ready(link) => {
                                opened = true;
                                if open(&link).is_err() {
                                    refused.store(true, Ordering::SeqCst);
                                    abort.store(true, Ordering::SeqCst);
                                    break;
                                }
                            }
                            Link::Refused => {
                                refused.store(true, Ordering::SeqCst);
                                abort.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
            let ran = child::run(
                &mut command,
                None,
                SIGN_IN_TIMEOUT.min(self.timeout),
                CHECK_CAP,
                Some(&abort),
            );
            done.store(true, Ordering::SeqCst);
            ran
        });
        drop(launch);
        egress.stop();
        match ran {
            Ran::Finished { status, .. } if status.success() => Ok(()),
            Ran::Finished { .. } => Err(invalid(text::SIGN_IN_FAILED)),
            Ran::CutOff => Err(invalid(text::SIGN_IN_TIMED_OUT)),
            Ran::Stopped if refused.load(Ordering::SeqCst) => Err(invalid(text::SIGN_IN_FAILED)),
            Ran::Stopped => Err(invalid(text::SIGN_IN_STOPPED)),
            Ran::NotStarted => Err(invalid(text::DOES_NOT_RUN)),
        }
    }

    /// Claude Code removes its own login; Voice never opens or deletes that file.
    pub fn sign_out(&self, home: &Path) -> Result<()> {
        self.ask(home, &["auth", "logout"])?
            .ok_or_else(|| invalid(text::SIGN_OUT_FAILED))?;
        match fs::symlink_metadata(self.profile.join(".credentials.json")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(invalid(text::SIGN_OUT_FILE_LEFT)),
        }
    }

    pub fn at(program: PathBuf, profile: PathBuf) -> Claude {
        Claude {
            program,
            profile,
            timeout: DEFAULT_TIMEOUT,
            stop: None,
        }
    }
    pub fn with_timeout(self, timeout: Duration) -> Claude {
        Claude { timeout, ..self }
    }
    pub fn with_stop(self, stop: Arc<AtomicBool>) -> Claude {
        Claude {
            stop: Some(stop),
            ..self
        }
    }
    pub fn program(&self) -> &Path {
        &self.program
    }

    fn ask(&self, home: &Path, args: &[&str]) -> Result<Option<Vec<u8>>> {
        let (egress, launch) = self.launch(home, false)?;
        let ran = child::run(
            &mut launch.command(args),
            None,
            CHECK_TIMEOUT.min(self.timeout),
            CHECK_CAP,
            None,
        );
        drop(launch);
        egress.stop();
        Ok(match ran {
            Ran::Finished { status, stdout } if status.success() => Some(stdout),
            _ => None,
        })
    }

    /// Check the version and sign-in. Only `loggedIn` and `authMethod` are read from the status:
    /// the object can carry an email address, which is never kept or shown.
    pub fn status(&self, home: &Path) -> Result<Status> {
        let stdout = self
            .ask(home, &["--version"])?
            .ok_or_else(|| invalid(text::DOES_NOT_RUN))?;
        let version = String::from_utf8_lossy(&stdout)
            .split_whitespace()
            .next()
            .filter(|version| valid_version(version))
            .map(str::to_owned)
            .ok_or_else(|| invalid(text::DOES_NOT_RUN))?;
        let (signed_in, subscription_refused) = self
            .ask(home, &["auth", "status", "--json"])?
            .and_then(|stdout| serde_json::from_slice::<Value>(&stdout).ok())
            .map(|status| sign_in_status(&status))
            .unwrap_or((false, false));
        Ok(Status {
            version,
            signed_in,
            subscription_refused,
        })
    }

    /// Claude Code runs and is signed in for this profile.
    pub fn check(&self, home: &Path) -> Result<Ready> {
        let status = self.status(home)?;
        if status.subscription_refused {
            return Err(invalid(text::SUBSCRIPTION_REFUSED));
        }
        if !status.signed_in {
            return Err(invalid(text::SIGNED_OUT));
        }
        Ok(Ready {
            version: status.version,
        })
    }
}

impl Provider for Claude {
    fn id(&self) -> &'static str {
        AGENT_ID
    }
    fn name(&self) -> &'static str {
        AGENT_NAME
    }
    fn reach(&self) -> Reach {
        REACH
    }
    /// One tool-less call, spent against the session's cap and recorded in its ledger.
    fn call(&self, session: &Session, sealed: &Sealed) -> Result<Reply> {
        sealed.check(AGENT_ID, REACH)?;
        let call = sealed.brief();
        check_brief(call)?;
        if !valid_model(call.model) {
            return Err(invalid(
                "the model name takes lower-case letters, digits, dots and dashes",
            ));
        }
        // `claude -p` expands a prompt that opens with `/` as a command (`/model`, `/config`),
        // and the built-in ones work under `--safe-mode`: records never open a prompt that way.
        if opens_with_slash(call.prompt) {
            return Err(invalid("a step's records can't begin with /"));
        }
        if self
            .stop
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            return Err(invalid(text::STOPPED));
        }
        let (egress, launch) = self.launch(session.home(), false)?;
        let started = session.start(AGENT_ID, call.step, call.model)?;
        let schema = call.schema.to_string();
        let budget = started.budget();
        let mut command = launch.command(&[
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
            "stream-json",
            "--verbose",
            "--json-schema",
            &schema,
            "--strict-mcp-config",
            "--mcp-config",
            r#"{"mcpServers":{}}"#,
            "--permission-mode",
            "dontAsk",
        ]);
        if let Some(budget) = budget {
            command.args(["--max-budget-usd", &format!("{budget:.2}")]);
        }
        let begun = Instant::now();
        let ran = child::run(
            &mut command,
            Some(call.prompt.as_bytes()),
            self.timeout,
            STDOUT_CAP,
            self.stop.as_deref(),
        );
        drop(launch);
        egress.stop();
        let seconds = begun.elapsed().as_secs_f64();
        let Some(judged) = judge(ran, self.timeout) else {
            // A child that never started spent nothing.
            session.discard(started)?;
            return Err(invalid(text::DOES_NOT_RUN));
        };
        let (cost_usd, cost_from) = match judged.cost {
            Some(cost) => (cost, "claude"),
            // Without a reported cost, count the budget when one was set.
            None => match budget {
                Some(budget) => (budget, "budget"),
                None => (0.0, "unknown"),
            },
        };
        let line = Line {
            agent: AGENT_ID,
            step: call.step,
            model: call.model,
            outcome: judged.outcome,
            plan: judged.plan,
            seconds,
            usage: judged.usage,
            cost_usd,
            cost_from,
        };
        session.finish(started, &line)?;
        judged.result
    }
}

fn open_command(link: &str) -> Command {
    let mut command = Command::new("/usr/bin/open");
    command
        .args(["-u", link])
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn open_in_browser(link: &str) -> std::io::Result<()> {
    let status = open_command(link).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("open failed"))
    }
}

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn interrupted(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// The stop flag set by Ctrl-C in the terminal.
pub fn stop_on_interrupt() -> &'static AtomicBool {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // SAFETY: sigaction and its empty mask receive valid pointers; the handler only stores
        // into a static atomic and remains valid for the lifetime of the process.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = interrupted as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            action.sa_flags = 0;
            libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
        }
    });
    &INTERRUPTED
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::sandbox;

    #[test]
    fn the_browser_opener_uses_a_url_from_the_root_folder() {
        let command = open_command("https://claude.com/x");
        assert_eq!(command.get_program(), "/usr/bin/open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-u", "https://claude.com/x"]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("/")));
    }

    #[test]
    fn every_native_launch_uses_its_own_proxy_and_sandbox() {
        let root = std::env::temp_dir().join(format!(
            "voice-wiring-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        struct Scratch(PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let scratch = Scratch(root);
        let profile = scratch.0.join("profile");
        fs::DirBuilder::new().mode(0o700).create(&profile).unwrap();
        let claude = Claude::at("/bin/ls".into(), profile.clone());
        for sign_in in [false, true] {
            let (egress, launch) = claude.launch(&scratch.0, sign_in).unwrap();
            let command = launch.command(&[]);
            assert_eq!(command.get_program(), sandbox::SANDBOX_EXEC);
            assert_ne!(egress.port(), 0);
            let args: Vec<_> = command.get_args().collect();
            let port = format!("PROXY_PORT={}", egress.port());
            assert!(args.windows(2).any(|pair| pair == ["-D", port.as_str()]));
            let mode = if sign_in { "SIGN_IN=yes" } else { "SIGN_IN=no" };
            assert!(args.windows(2).any(|pair| pair == ["-D", mode]));
            assert_eq!(
                command.get_current_dir().unwrap().parent().unwrap(),
                sandbox::runs_folder(&profile.canonicalize().unwrap())
            );
            assert_eq!(
                command
                    .get_envs()
                    .any(|(name, value)| name == "BROWSER" && value.is_some()),
                sign_in
            );
            drop(launch);
            egress.stop();
        }
    }
}

#[cfg(test)]
mod sign_in_status_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn login_arguments_offer_only_the_builds_allowed_accounts() {
        assert_eq!(
            login_args(SignIn::Console),
            Some(["auth", "login", "--console"])
        );
        #[cfg(feature = "claude-subscription")]
        assert_eq!(
            login_args(SignIn::Subscription),
            Some(["auth", "login", "--claudeai"])
        );
        #[cfg(not(feature = "claude-subscription"))]
        assert_eq!(login_args(SignIn::Subscription), None);
    }

    #[test]
    fn sign_in_status_accepts_only_console_or_an_offered_subscription() {
        for method in [
            "api_key",
            "claude.ai",
            "oauth_token",
            "none",
            "api_key_helper",
            "third_party",
            "unknown",
        ] {
            for logged_in in [true, false] {
                let subscription = matches!(method, "claude.ai" | "oauth_token");
                let expected = (
                    logged_in && (method == "api_key" || (subscription && SUBSCRIPTION_OFFERED)),
                    logged_in && subscription && !SUBSCRIPTION_OFFERED,
                );
                assert_eq!(
                    sign_in_status(&json!({"loggedIn": logged_in, "authMethod": method})),
                    expected,
                    "{method}, loggedIn={logged_in}"
                );
            }
        }
        assert_eq!(SUBSCRIPTION_OFFERED, cfg!(feature = "claude-subscription"));
        for value in [
            json!({}),
            json!({"loggedIn": true}),
            json!({"loggedIn": true, "authMethod": 7}),
            json!({"loggedIn": true, "authMethod": null}),
            json!({"loggedIn": false}),
            json!({"loggedIn": false, "authMethod": 7}),
            json!({"loggedIn": false, "authMethod": null}),
            json!({"authMethod": "api_key"}),
            json!({"loggedIn": "true", "authMethod": "api_key"}),
        ] {
            assert_eq!(sign_in_status(&value), (false, false), "{value}");
        }
    }
}
