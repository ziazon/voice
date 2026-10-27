//! App boundary: only the fixed, public sentences here can reach the webview.
use crate::dto;
use persona_builder::consent::Record;
use persona_builder::{
    assistants,
    cli_command::{CLI_FOLDER, cli_word, shipped_cli},
    consent, crew,
    error::Error,
};
use persona_builder_agent::{claude, ledger::local_day, text};
use persona_builder_ingest::private::{read_private_optional, remove_private, write_private};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

type Result<T> = std::result::Result<T, dto::Failure>;
const LOOK: &str = "Persona Builder couldn't look for your assistants. Close it and open it again.";
pub(crate) const FIND: &str =
    "Persona Builder couldn't find that assistant. Close it and open it again.";
const MAKE: &str = "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again.";
const KEEP: &str = "Persona Builder couldn't keep your answer. Check there's space on this computer, then try again.";
pub(crate) fn failure(message: &str) -> dto::Failure {
    dto::Failure {
        message: message.into(),
    }
}
pub(crate) fn home(message: &str) -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| failure(message))
}

pub(crate) fn listed(home: &Path, slug: &str) -> Result<assistants::Listed> {
    assistants::assistant_dir(home, slug).map_err(|_| failure(FIND))?;
    let listed = assistants::list(home)
        .map_err(|_| failure(FIND))?
        .into_iter()
        .find(|listed| listed.member.slug == slug)
        .ok_or_else(|| failure(FIND))?;
    crew::check_name("name", &listed.member.name).map_err(|_| failure(FIND))?;
    Ok(listed)
}

fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let year = date[..4].parse::<u32>().unwrap_or(0);
    let month = date[5..7].parse::<u32>().unwrap_or(0);
    let day = date[8..].parse::<u32>().unwrap_or(0);
    let days = persona_builder_ingest::dates::days_in_month(i64::from(year), i64::from(month));
    year > 0 && (1..=12).contains(&month) && day > 0 && i64::from(day) <= days
}
/// Invalid, unavailable or superseded permission records never count as agreement.
pub fn read_consent(home: &Path, slug: &str) -> dto::Consent {
    let record = read_private_optional(home, slug, "consent.json")
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
        .filter(|record| record.version == consent::VERSION && valid_date(&record.agreed_on));
    dto::Consent {
        agreed: record.is_some(),
        agreed_on: record.map(|record| record.agreed_on),
    }
}
/// The shared private record writer; the caller validates the member first.
pub fn write_consent(home: &Path, slug: &str) -> persona_builder::error::Result<dto::Consent> {
    let record = Record {
        version: consent::VERSION,
        agreed_on: local_day(SystemTime::now()),
    };
    let bytes = serde_json::to_vec(&record)
        .map_err(|_| Error::Invalid("couldn't encode permission".into()))?;
    write_private(home, slug, "consent.json", &bytes)?;
    Ok(read_consent(home, slug))
}
fn assistant(home: &Path, listed: assistants::Listed) -> dto::Assistant {
    dto::Assistant {
        consent: read_consent(home, &listed.member.slug),
        slug: listed.member.slug,
        name: listed.member.name,
        principal: listed.member.principal,
    }
}
pub fn list_assistants_in(home: &Path) -> Result<Vec<dto::Assistant>> {
    assistants::list(home)
        .map(|items| {
            items
                .into_iter()
                .map(|item| assistant(home, item))
                .collect()
        })
        .map_err(|_| failure(LOOK))
}
pub fn list_assistants() -> Result<Vec<dto::Assistant>> {
    list_assistants_in(&home(LOOK)?)
}
pub fn new_assistant_in(home: &Path, request: dto::NewAssistantRequest) -> Result<dto::Assistant> {
    for (what, value, field) in [
        ("name", &request.name, "Your assistant's name"),
        ("pronouns", &request.pronouns, "Its pronouns"),
        ("your name", &request.principal, "Your name"),
        (
            "your pronouns",
            &request.principal_pronouns,
            "Your pronouns",
        ),
    ] {
        crew::check_name(what, value).map_err(|_| {
            failure(&format!(
                "{field} can be up to 64 characters on one line, without @ < > {{ }} [ ] ( ) # \\ or `."
            ))
        })?;
    }
    if crew::slugify(&request.name).is_none() {
        return Err(failure(
            "Your assistant's name needs at least one letter or number.",
        ));
    }
    let new = assistants::New {
        name: &request.name,
        pronouns: &request.pronouns,
        principal: &request.principal,
        principal_pronouns: &request.principal_pronouns,
    };
    let cli = shipped_cli()
        .and_then(|path| cli_word(&path, home))
        .map_err(|error| match error {
            Error::Invalid(message) if message == CLI_FOLDER => failure(CLI_FOLDER),
            _ => failure(MAKE),
        })?;
    let listed =
        assistants::make(home, &new, &local_day(SystemTime::now()), &cli).map_err(|error| {
            match error {
                Error::Exists(_) => failure(&format!(
                    "You already have an assistant called {}. Pick another name, or open that one.",
                    request.name
                )),
                _ => failure(MAKE),
            }
        })?;
    if clear_consent(home, &listed.member.slug).is_err() {
        let _ = std::fs::remove_dir_all(&listed.dir);
        return Err(failure(MAKE));
    }
    Ok(assistant(home, listed))
}
pub fn new_assistant(request: dto::NewAssistantRequest) -> Result<dto::Assistant> {
    new_assistant_in(&home(MAKE)?, request)
}
pub fn consent_text() -> Result<dto::ConsentText> {
    Ok(dto::ConsentText {
        version: consent::VERSION,
        title: consent::TITLE.into(),
        sections: consent::SECTIONS
            .iter()
            .map(|section| dto::ConsentSection {
                heading: section.heading.into(),
                lines: section.lines.iter().map(|line| (*line).into()).collect(),
            })
            .collect(),
    })
}
pub fn agree_consent_in(home: &Path, request: dto::SlugRequest) -> Result<dto::Consent> {
    listed(home, &request.slug)?;
    write_consent(home, &request.slug).map_err(|_| failure(KEEP))
}
pub fn agree_consent(request: dto::SlugRequest) -> Result<dto::Consent> {
    agree_consent_in(&home(FIND)?, request)
}
pub fn show_assistant_folder_in(
    home: &Path,
    request: dto::SlugRequest,
    open: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<()> {
    let dir = listed(home, &request.slug)?.dir;
    let dir = dir.canonicalize().map_err(|_| failure(FIND))?;
    open(&dir).map_err(|_| failure("Persona Builder couldn't open the folder."))
}
pub fn show_assistant_folder(request: dto::SlugRequest) -> Result<()> {
    show_assistant_folder_in(&home(FIND)?, request, |dir| {
        let (program, args) = opener_args(dir);
        // Wait to reap the child; ignore its exit code (explorer can open the folder and exit 1).
        Command::new(program).args(args).status().map(|_| ())
    })
}
fn ready_status(status: claude::Status) -> dto::ClaudeStatus {
    dto::ClaudeStatus {
        ready: true,
        message: text::ready(&status.version),
        note: status.api_key_set.then(|| text::API_KEY_SET.into()),
        can_sign_in: false,
    }
}
fn unavailable_status(error: Error) -> dto::ClaudeStatus {
    dto::ClaudeStatus {
        ready: false,
        message: claude_message(error),
        note: None,
        can_sign_in: false,
    }
}
pub fn claude_check_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
) -> Result<dto::ClaudeStatus> {
    let claude = match claude::find_in(home, path, fallbacks) {
        Ok(claude) => claude,
        Err(error) => return Ok(unavailable_status(error)),
    };
    Ok(match claude.status(home) {
        Ok(status) if status.signed_in => ready_status(status),
        Ok(status) => dto::ClaudeStatus {
            ready: false,
            message: text::signed_out_at(
                &status.version,
                &text::shown_program(claude.program(), home),
            ),
            note: None,
            can_sign_in: true,
        },
        Err(error) => unavailable_status(error),
    })
}
pub fn claude_check() -> Result<dto::ClaudeStatus> {
    let Ok(home) = home(text::FAILED) else {
        return Ok(unavailable_status(Error::Invalid(text::FAILED.into())));
    };
    claude_check_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
    )
}

/// Clear permission before returning a newly made assistant.
pub fn clear_consent(home: &Path, slug: &str) -> persona_builder::error::Result<bool> {
    remove_private(home, slug, "consent.json")
}

pub(crate) fn opener_args(dir: &Path) -> (&'static str, Vec<OsString>) {
    #[cfg(target_os = "macos")]
    return ("/usr/bin/open", vec![OsString::from("-R"), dir.into()]);
    #[cfg(target_os = "linux")]
    return ("xdg-open", vec![dir.into()]);
    #[cfg(target_os = "windows")]
    return ("explorer", vec![dir.into()]);
}

/// Only the agent's public fixed sentences cross the app boundary.
pub(crate) fn claude_message(error: Error) -> String {
    match error {
        Error::Invalid(message)
            if [
                text::NOT_INSTALLED,
                text::DOES_NOT_RUN,
                text::SIGNED_OUT,
                text::PLAN_STOPPED,
                text::SIGN_IN_FAILED,
                text::SIGN_IN_TIMED_OUT,
                text::SIGN_IN_STOPPED,
                text::PROFILE_FAILED,
            ]
            .contains(&message.as_str()) =>
        {
            message
        }
        _ => text::FAILED.into(),
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn macos_opener_reveals_instead_of_launching() {
        let dir = Path::new("/fictional/assistant.app");
        assert_eq!(
            opener_args(dir),
            ("/usr/bin/open", vec![OsString::from("-R"), dir.into()])
        );
    }
}

/// The single Claude sign-in currently waiting for the browser.
#[derive(Default)]
pub struct ClaudeSignIns {
    state: Mutex<Option<Arc<AtomicBool>>>,
}
impl ClaudeSignIns {
    pub fn new() -> Self {
        Self::default()
    }
}
pub const SIGN_IN_BUSY: &str =
    "Persona Builder is already waiting for you to sign in on Claude's page in your browser.";
pub fn claude_sign_ins() -> &'static ClaudeSignIns {
    static SIGN_INS: OnceLock<ClaudeSignIns> = OnceLock::new();
    SIGN_INS.get_or_init(ClaudeSignIns::new)
}
/// Releases the slot on every return, including an unwinding panic.
struct ClaudeSignInGuard<'a>(&'a ClaudeSignIns);
impl Drop for ClaudeSignInGuard<'_> {
    fn drop(&mut self) {
        *self.0.state.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
}
pub fn claude_sign_in_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    sign_ins: &ClaudeSignIns,
) -> Result<dto::ClaudeStatus> {
    let flag = {
        let mut state = sign_ins.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.is_some() {
            return Err(failure(SIGN_IN_BUSY));
        }
        let flag = Arc::new(AtomicBool::new(false));
        *state = Some(Arc::clone(&flag));
        flag
    };
    let _guard = ClaudeSignInGuard(sign_ins);
    let claude = match claude::find_in(home, path, fallbacks) {
        Ok(claude) => claude,
        Err(error) => return Ok(unavailable_status(error)),
    };
    match claude.status(home) {
        Ok(status) if status.signed_in => return Ok(ready_status(status)),
        Err(error) => return Ok(unavailable_status(error)),
        _ => {}
    }
    let result = claude.sign_in(home, &flag);
    if let Err(Error::Invalid(message)) = &result
        && message == text::SIGN_IN_STOPPED
    {
        return Ok(dto::ClaudeStatus {
            ready: false,
            message: message.clone(),
            note: None,
            can_sign_in: true,
        });
    }
    if let Ok(status) = claude.status(home)
        && status.signed_in
    {
        return Ok(ready_status(status));
    }
    Ok(dto::ClaudeStatus {
        ready: false,
        message: result
            .err()
            .map(claude_message)
            .unwrap_or_else(|| text::SIGN_IN_FAILED.into()),
        note: None,
        can_sign_in: true,
    })
}
pub fn claude_sign_in_cancel_in(sign_ins: &ClaudeSignIns) -> Result<()> {
    if let Some(flag) = sign_ins
        .state
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
    {
        flag.store(true, Ordering::SeqCst);
    }
    Ok(())
}
pub fn claude_sign_in() -> Result<dto::ClaudeStatus> {
    let Ok(home) = home(text::FAILED) else {
        return Ok(unavailable_status(Error::Invalid(text::FAILED.into())));
    };
    claude_sign_in_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        claude_sign_ins(),
    )
}
pub fn claude_sign_in_cancel() -> Result<()> {
    claude_sign_in_cancel_in(claude_sign_ins())
}
pub fn claude_sign_in_shutdown(wait: Duration) {
    let sign_ins = claude_sign_ins();
    let _ = claude_sign_in_cancel_in(sign_ins);
    let started = Instant::now();
    while sign_ins
        .state
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .is_some()
    {
        if started.elapsed() >= wait {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
