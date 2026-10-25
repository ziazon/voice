//! The save step's other sections: Claude Code on and off, the claude.ai files, forget.
use crate::{
    assistants::{failure, home, listed, opener_args},
    dto,
};
use persona_builder::{
    assistants::Listed,
    crew,
    error::Error,
    forget as core_forget,
    install::{self, FileState, Places},
    member, pack,
};
use persona_builder_ingest::private::{self, lock_state, private_folder_exists};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, dto::Failure>;
const STATUS: &str = "Persona Builder couldn't check Claude Code. Close it and open it again.";
const NOT_SET_UP: &str =
    "Claude Code isn't set up on this computer yet. Install it and sign in, then come back.";
const NOTHING_ON: &str = "No assistant is on in Claude Code.";
const CHANGED_ON: &str = "What's on in Claude Code has changed. Look again, then choose.";
const OFF: &str =
    "Persona Builder couldn't turn the assistant off. Close it and open it again, then try again.";
const FILES: &str =
    "Persona Builder couldn't look for the claude.ai files. Close it and open it again.";
const PACK: &str = "Persona Builder couldn't make the claude.ai files. Check there's space on this computer, then try again.";
const MAKE_FIRST: &str = "Make the claude.ai files first.";
const SHOW: &str = "Persona Builder couldn't open the folder.";
const FORGET: &str = "Persona Builder couldn't forget anything, and nothing was deleted. Close it and open it again, then try again.";
const FORGET_CHANGED: &str =
    "There's more to forget than when you looked. Look again, then confirm.";

pub fn claude_home(home: &Path, config_dir: Option<&OsStr>) -> PathBuf {
    config_dir
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"))
}
fn shown_name(name: &str) -> Option<String> {
    crew::check_name("name", name)
        .ok()
        .map(|()| name.to_owned())
}
fn said(message: impl Into<String>) -> dto::Said {
    dto::Said {
        message: message.into(),
    }
}
fn on_failed(name: &str) -> String {
    format!(
        "Persona Builder couldn't turn {name} on, so nothing in your Claude folder was changed."
    )
}
fn other_on(shown: Option<&str>, name: &str) -> String {
    match shown {
        Some(shown) => {
            format!("{shown} is on in Claude Code. Turn {shown} off first, then turn {name} on.")
        }
        None => format!(
            "Another assistant is on in Claude Code. Turn it off first, then turn {name} on."
        ),
    }
}
fn busy(name: &str) -> String {
    format!("Persona Builder is already working on {name}. Wait for it to finish, then try again.")
}
fn no_kinds(name: &str) -> String {
    format!(
        "{name} doesn't know any kinds of question yet. Build {name} first, then make the claude.ai files."
    )
}
fn nothing_to_forget(name: &str) -> String {
    format!("There's nothing to forget: Persona Builder hasn't kept anything for {name} yet.")
}
fn count(files: usize) -> u32 {
    u32::try_from(files).unwrap_or(u32::MAX)
}
fn places(home: &Path, claude_home: &Path) -> Places {
    Places {
        home: home.into(),
        claude_home: claude_home.into(),
    }
}
fn is_this(status: &install::Status, listed: &Listed) -> bool {
    status.slug == listed.member.slug
        && matches!((fs::canonicalize(&status.member_dir), fs::canonicalize(&listed.dir)), (Ok(a), Ok(b)) if a == b)
}
fn lock_error(error: Error, name: &str, fallback: &str) -> dto::Failure {
    if matches!(error, Error::Invalid(ref message) if message == private::BUSY) {
        failure(&busy(name))
    } else {
        failure(fallback)
    }
}

pub fn claude_code_status_in(
    home: &Path,
    claude_home: &Path,
    request: dto::SlugRequest,
) -> Result<dto::ClaudeCodeStatus> {
    let listed = listed(home, &request.slug)?;
    let empty = |state| dto::ClaudeCodeStatus {
        state,
        on_name: None,
        on_slug: None,
        out_of_date: false,
        changed: false,
    };
    if !claude_home.is_dir() {
        return Ok(empty(dto::OnState::NotSetUp));
    }
    let Some(status) = install::status(&places(home, claude_home)).map_err(|_| failure(STATUS))?
    else {
        return Ok(empty(dto::OnState::Off));
    };
    let this = is_this(&status, &listed);
    Ok(dto::ClaudeCodeStatus {
        state: if this {
            dto::OnState::This
        } else {
            dto::OnState::Other
        },
        on_name: shown_name(&status.name),
        on_slug: Some(status.slug),
        out_of_date: this && status.out_of_date,
        changed: this
            && (!status.line
                || status
                    .files
                    .iter()
                    .any(|(_, state)| *state != FileState::Present)),
    })
}
pub fn turn_on_in(home: &Path, claude_home: &Path, request: dto::SlugRequest) -> Result<dto::Said> {
    let listed = listed(home, &request.slug)?;
    let name = &listed.member.name;
    if !claude_home.is_dir() {
        return Err(failure(NOT_SET_UP));
    }
    let places = places(home, claude_home);
    if let Some(status) = install::status(&places).map_err(|_| failure(&on_failed(name)))?
        && !is_this(&status, &listed)
    {
        return Err(failure(&other_on(
            shown_name(&status.name).as_deref(),
            name,
        )));
    }
    install::turn_on(&listed.dir, &places).map_err(|_| failure(&on_failed(name)))?;
    Ok(said(format!(
        "{name} is on. Start a new Claude Code session to use {name}."
    )))
}
pub fn turn_off_in(
    home: &Path,
    claude_home: &Path,
    request: dto::TurnOffRequest,
) -> Result<dto::Said> {
    if !claude_home.is_dir() {
        return Ok(said(NOTHING_ON));
    }
    let places = places(home, claude_home);
    let Some(status) = install::status(&places).map_err(|_| failure(OFF))? else {
        return Ok(said(NOTHING_ON));
    };
    if status.slug != request.slug {
        return Err(failure(CHANGED_ON));
    }
    let result = install::turn_off(&places).map_err(|_| failure(OFF))?;
    let shown = result.name.as_deref().and_then(shown_name);
    let subject = shown.as_deref().unwrap_or("The assistant");
    Ok(said(if result.left.is_empty() {
        format!("{subject} is off. Everything Persona Builder added is gone.")
    } else {
        let pronoun = shown.as_deref().unwrap_or("it");
        format!(
            "{subject} is off. A few things in your Claude folder were changed after {pronoun} was turned on, so Persona Builder left them where they are."
        )
    }))
}
fn pack_file(name: &str) -> bool {
    if name == "instructions.md" {
        return true;
    }
    name.strip_prefix("decisions-")
        .and_then(|name| name.strip_suffix(".md"))
        .is_some_and(|kind| {
            (1..=64).contains(&kind.len())
                && kind
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}
fn files(home: &Path, slug: &str, fallback: &str) -> Result<dto::ClaudeAiFiles> {
    let mut files = vec![];
    if private_folder_exists(home, slug, "claude-ai").map_err(|_| failure(fallback))? {
        let folder = home.join(".claude/state").join(slug).join("claude-ai");
        for entry in fs::read_dir(folder).map_err(|_| failure(fallback))? {
            let entry = entry.map_err(|_| failure(fallback))?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|_| failure(fallback))?;
            if metadata.is_file()
                && let Some(name) = entry.file_name().to_str()
                && pack_file(name)
            {
                files.push(name.to_owned());
            }
        }
    }
    files.sort_by(|a, b| (a != "instructions.md", a).cmp(&(b != "instructions.md", b)));
    Ok(dto::ClaudeAiFiles { files })
}
pub fn claude_ai_files_in(home: &Path, request: dto::SlugRequest) -> Result<dto::ClaudeAiFiles> {
    listed(home, &request.slug)?;
    files(home, &request.slug, FILES)
}
pub fn make_claude_ai_files_in(
    home: &Path,
    request: dto::SlugRequest,
) -> Result<dto::ClaudeAiFiles> {
    let listed = listed(home, &request.slug)?;
    let name = &listed.member.name;
    let state = member::private_state_dir(&listed.member, home).ok_or_else(|| failure(PACK))?;
    let _lock = lock_state(home, &request.slug).map_err(|error| lock_error(error, name, PACK))?;
    if private_folder_exists(home, &request.slug, "claude-ai-new").map_err(|_| failure(PACK))? {
        fs::remove_dir_all(state.join("claude-ai-new")).map_err(|_| failure(PACK))?;
    }
    pack::pack(&listed.dir, &state.join("claude-ai-new")).map_err(|error| {
        if matches!(error, Error::Invalid(ref message) if message == &pack::no_kinds_yet(name)) {
            failure(&no_kinds(name))
        } else {
            failure(PACK)
        }
    })?;
    if private_folder_exists(home, &request.slug, "claude-ai").map_err(|_| failure(PACK))? {
        fs::remove_dir_all(state.join("claude-ai")).map_err(|_| failure(PACK))?;
    }
    fs::rename(state.join("claude-ai-new"), state.join("claude-ai")).map_err(|_| failure(PACK))?;
    files(home, &request.slug, PACK)
}
pub fn show_claude_ai_files_in(
    home: &Path,
    request: dto::SlugRequest,
    open: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<()> {
    listed(home, &request.slug)?;
    if !private_folder_exists(home, &request.slug, "claude-ai").map_err(|_| failure(SHOW))? {
        return Err(failure(MAKE_FIRST));
    }
    let folder = home
        .join(".claude/state")
        .join(&request.slug)
        .join("claude-ai")
        .canonicalize()
        .map_err(|_| failure(SHOW))?;
    open(&folder).map_err(|_| failure(SHOW))
}
pub fn forget_preview_in(home: &Path, request: dto::SlugRequest) -> Result<dto::ForgetPreview> {
    let listed = listed(home, &request.slug)?;
    let preview = core_forget::forget(&listed.dir, home, false).map_err(|_| failure(FORGET))?;
    Ok(dto::ForgetPreview {
        files: count(preview.files),
    })
}
pub fn forget_in(home: &Path, request: dto::ForgetRequest) -> Result<dto::Said> {
    let listed = listed(home, &request.slug)?;
    let preview = core_forget::forget(&listed.dir, home, false).map_err(|_| failure(FORGET))?;
    let name = &listed.member.name;
    if preview.files == 0 {
        return Ok(said(nothing_to_forget(name)));
    }
    let _lock = lock_state(home, &request.slug).map_err(|error| lock_error(error, name, FORGET))?;
    let preview = core_forget::forget(&listed.dir, home, false).map_err(|_| failure(FORGET))?;
    if count(preview.files) != request.files {
        return Err(failure(FORGET_CHANGED));
    }
    let forgotten = core_forget::forget(&listed.dir, home, true).map_err(|_| failure(FORGET))?;
    let count = if forgotten.files == 1 {
        "1 file".into()
    } else {
        format!("{} files", forgotten.files)
    };
    Ok(said(format!(
        "Deleted {count}. {name}'s own folder is untouched."
    )))
}
pub fn claude_code_status(request: dto::SlugRequest) -> Result<dto::ClaudeCodeStatus> {
    let home = home(STATUS)?;
    claude_code_status_in(
        &home,
        &claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
        request,
    )
}
pub fn turn_on(request: dto::SlugRequest) -> Result<dto::Said> {
    let home = home(STATUS)?;
    turn_on_in(
        &home,
        &claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
        request,
    )
}
pub fn turn_off(request: dto::TurnOffRequest) -> Result<dto::Said> {
    let home = home(OFF)?;
    turn_off_in(
        &home,
        &claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
        request,
    )
}
pub fn claude_ai_files(request: dto::SlugRequest) -> Result<dto::ClaudeAiFiles> {
    claude_ai_files_in(&home(FILES)?, request)
}
pub fn make_claude_ai_files(request: dto::SlugRequest) -> Result<dto::ClaudeAiFiles> {
    make_claude_ai_files_in(&home(PACK)?, request)
}
pub fn show_claude_ai_files(request: dto::SlugRequest) -> Result<()> {
    show_claude_ai_files_in(&home(SHOW)?, request, |dir| {
        let (program, args) = opener_args(dir);
        Command::new(program).args(args).status().map(|_| ())
    })
}
pub fn forget_preview(request: dto::SlugRequest) -> Result<dto::ForgetPreview> {
    forget_preview_in(&home(FORGET)?, request)
}
pub fn forget(request: dto::ForgetRequest) -> Result<dto::Said> {
    forget_in(&home(FORGET)?, request)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    #[test]
    fn shown_names_must_be_safe() {
        assert_eq!(shown_name("Ari"), Some("Ari".into()));
        for name in ["Ari\n@~/.ssh/x", "", &"a".repeat(65)] {
            assert_eq!(shown_name(name), None);
        }
    }
    #[test]
    fn claude_home_uses_nonempty_config_only() {
        let home = Path::new("/scratch/home");
        assert_eq!(claude_home(home, Some(OsStr::new("/x"))), Path::new("/x"));
        assert_eq!(
            claude_home(home, Some(OsStr::new(""))),
            home.join(".claude")
        );
        assert_eq!(claude_home(home, None), home.join(".claude"));
    }
}
