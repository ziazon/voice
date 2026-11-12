//! App boundary: only the fixed, public sentences here can reach the webview.
use crate::dto;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};
use voice_agent::{claude, ledger::local_day, text};
use voice_core::consent::Record;
use voice_core::{
    assistants,
    cli_command::{CLI_FOLDER, CLI_PLACE, shipped_cli_word},
    consent, crew,
    error::Error,
    existing, old_files,
};
use voice_ingest::private::{read_private_optional, remove_private, write_private};

type Result<T> = std::result::Result<T, dto::Failure>;
const LOOK: &str = "Voice couldn't look for your assistants. Close it and open it again.";
pub(crate) const FIND: &str = "Voice couldn't find that assistant. Close it and open it again.";
const LOOK_BEFORE: &str =
    "Voice couldn't look for an older assistant with that name. Close it and open it again.";
const MAKE: &str = "Voice couldn't make the assistant's folder. Check there's space on this computer, then try again.";
const KEEP: &str =
    "Voice couldn't keep your answer. Check there's space on this computer, then try again.";
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
    let listed = assistants::find(home, slug)
        .map_err(|_| failure(FIND))?
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
    let days = voice_ingest::dates::days_in_month(i64::from(year), i64::from(month));
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
pub fn write_consent(home: &Path, slug: &str) -> voice_core::error::Result<dto::Consent> {
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
fn check_field(what: &str, value: &str, field: &str) -> Result<()> {
    crew::check_name(what, value).map_err(|_| {
        failure(&format!(
            "{field} can be up to 64 characters on one line, without @ < > {{ }} [ ] ( ) # \\ or `."
        ))
    })
}
fn checked_request(request: &dto::NewAssistantRequest) -> Result<String> {
    check_field("name", &request.name, "Your assistant's name")?;
    for (what, value, field) in [
        ("pronouns", &request.pronouns, "Its pronouns"),
        ("your name", &request.principal, "Your name"),
        (
            "your pronouns",
            &request.principal_pronouns,
            "Your pronouns",
        ),
    ] {
        check_field(what, value, field)?;
    }
    checked_name(&request.name)
}
fn cli_word(home: &Path) -> Result<String> {
    shipped_cli_word(home).map_err(|error| match error {
        Error::Invalid(message) if message == CLI_FOLDER || message == CLI_PLACE => {
            failure(&message)
        }
        _ => failure(MAKE),
    })
}
fn already_have(name: &str) -> dto::Failure {
    failure(&format!(
        "You already have an assistant called {name}. Pick another name, or open that one."
    ))
}
fn checked_name(name: &str) -> Result<String> {
    check_field("name", name, "Your assistant's name")?;
    crew::slugify(name)
        .ok_or_else(|| failure("Your assistant's name needs at least one letter or number."))
}
/// Offer files using the same privacy rules as the assistant's readers.
pub fn offer_old_files(
    home: &Path,
    folder: &Path,
) -> voice_core::error::Result<Vec<old_files::Offered>> {
    old_files::offer(folder, &|value| private_ok(home, value))
}

fn private_ok(home: &Path, value: &serde_json::Value) -> bool {
    if let Some(filters) = value.get("filters") {
        let Some(filters) = filters.as_object() else {
            return false;
        };
        let Some(template) = voice_core::template::FILES
            .iter()
            .find(|(name, _)| *name == "member.json")
            .and_then(|(_, text)| serde_json::from_str::<serde_json::Value>(text).ok())
        else {
            return false;
        };
        let Some(base_filters) = template
            .get("filters")
            .and_then(serde_json::Value::as_object)
        else {
            return false;
        };
        for (key, value) in filters {
            let valid = if matches!(key.as_str(), "pasted_prefixes" | "denied_paths") {
                value.as_array().is_some_and(|items| {
                    items
                        .iter()
                        .all(|item| item.as_str().is_some_and(|text| !text.is_empty()))
                })
            } else if matches!(key.as_str(), "excluded_folders" | "personal" | "secret") {
                value.as_str().is_some_and(|pattern| {
                    if voice_ingest::filters::check_pattern(key, pattern).is_err() {
                        return false;
                    }
                    let Some(base) = base_filters
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .filter(|base| !base.is_empty())
                    else {
                        return true;
                    };
                    // read_member joins patterns inside groups. An extended-mode
                    // comment must not swallow the closing group in that form.
                    voice_ingest::filters::check_pattern(key, &format!("(?:{base})|(?:{pattern})"))
                        .is_ok()
                })
            } else {
                value.is_string()
            };
            if !valid {
                return false;
            }
        }
    }
    voice_ingest::anonymize::Rules::from_value(value.get("anonymizer"), home).is_ok()
}

fn check_brought_member(home: &Path, folder: &Path) -> voice_core::error::Result<()> {
    let member = voice_core::member::read_member(&folder.join("member.json"))?;
    voice_ingest::filters::Filters::compile(&member.filters)?;
    voice_ingest::anonymize::Rules::for_member(&member, home)?;
    Ok(())
}

pub fn find_existing_in(
    home: &Path,
    claude_home: &Path,
    request: dto::FindExistingRequest,
) -> Result<dto::ExistingAssistant> {
    let slug = checked_name(&request.name)?;
    let found = existing::find(home, claude_home, &slug).map_err(|_| failure(LOOK_BEFORE))?;
    let mut lines = Vec::new();
    if let Some(state) = &found.state {
        if !state.link && !state.not_folder {
            let noun = if state.files == 1 { "file" } else { "files" };
            lines.push(format!(
                "A private folder from before, ~/.claude/state/{slug}/, holds {} {noun}.",
                state.files
            ));
        }
        if state.corpus {
            lines.push("One of them is a put-together history that Voice would replace.".into());
        }
        if state.live_log {
            lines.push("One of them is a log of answers you recorded.".into());
        }
        if state.link {
            lines.push(format!(
                "~/.claude/state/{slug} is a link to somewhere else."
            ));
        }
        if state.not_folder {
            lines.push(format!("~/.claude/state/{slug} is a file, not a folder."));
        }
    }
    if found.skill.is_some() {
        lines.push(format!(
            "Claude Code has a skill called {slug} that Voice didn't add."
        ));
    }
    if !found.lines.is_empty() {
        let noun = if found.lines.len() == 1 {
            "line"
        } else {
            "lines"
        };
        lines.push(format!(
            "Your CLAUDE.md mentions /{slug} on {} {noun} that Voice didn't add.",
            found.lines.len()
        ));
    }
    if found.old_folder.is_some() {
        lines.push(
            "That skill points to the folder the assistant was made in, with its own files.".into(),
        );
    }
    Ok(dto::ExistingAssistant {
        files: found
            .old_folder
            .as_deref()
            .map(|folder| offer_old_files(home, folder))
            .transpose()
            .map_err(|_| failure(LOOK_BEFORE))?
            .unwrap_or_default()
            .into_iter()
            .map(|file| dto::OldFile {
                name: file.name.into(),
                problem: file.problem.map(str::to_owned),
            })
            .collect(),
        found: found.any(),
        can_bring_in: found.any()
            && bring_in_refusal(home, &found)
                .map_err(|_| failure(LOOK_BEFORE))?
                .is_none(),
        lines,
    })
}
pub fn find_existing(request: dto::FindExistingRequest) -> Result<dto::ExistingAssistant> {
    let home = home(LOOK_BEFORE)?;
    find_existing_in(
        &home,
        &crate::use_it::claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
        request,
    )
}
pub fn new_assistant_in(
    home: &Path,
    claude_home: &Path,
    request: dto::NewAssistantRequest,
) -> Result<dto::Assistant> {
    let slug = checked_request(&request)?;
    let found = existing::find(home, claude_home, &slug).map_err(|_| failure(LOOK_BEFORE))?;
    if found.any() {
        return Err(failure(&format!(
            "Voice found an older {} on this computer, so it made nothing and changed nothing. Choose Back it up and bring it in, or pick another name.",
            request.name
        )));
    }
    let new = assistants::New {
        name: &request.name,
        pronouns: &request.pronouns,
        principal: &request.principal,
        principal_pronouns: &request.principal_pronouns,
    };
    let cli = cli_word(home)?;
    let listed =
        assistants::make(home, &new, &local_day(SystemTime::now()), &cli).map_err(|error| {
            match error {
                Error::Exists(_) => already_have(&request.name),
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
    let home = home(MAKE)?;
    new_assistant_in(
        &home,
        &crate::use_it::claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
        request,
    )
}
const CORPUS_NOT_PLAIN: &str = "Voice can't bring this one in: the decisions.jsonl in its private folder is a link or a folder, not a file. Pick another name for now.";
const HISTORY_BEFORE: &str = "Voice can't bring this one in: its private folder already holds a history from before Voice. Pick another name for now.";

fn state_refusal(slug: &str) -> String {
    format!(
        "Voice can't bring in ~/.claude/state/{slug} because it's a link or a file, not a folder. Move it somewhere else, or pick another name."
    )
}

/// The same preflight decides whether the screen and CLI offer to bring an older assistant in.
pub fn bring_in_refusal(
    home: &Path,
    found: &existing::Found,
) -> voice_core::error::Result<Option<String>> {
    let state = home.join(".claude/state").join(&found.slug);
    match fs::symlink_metadata(&state) {
        Ok(metadata) if !metadata.is_dir() => {
            return Ok(Some(state_refusal(&found.slug)));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Io {
                path: state,
                source,
            });
        }
    }
    for (name, must_be_regular) in [
        ("decisions-before-voice.jsonl", false),
        ("decisions.jsonl", true),
    ] {
        let path = state.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !must_be_regular || !metadata.is_file() => {
                return Ok(Some(
                    if must_be_regular {
                        CORPUS_NOT_PLAIN
                    } else {
                        HISTORY_BEFORE
                    }
                    .into(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(Error::Io { path, source }),
        }
    }
    Ok(None)
}

fn make_and_clear(
    home: &Path,
    new: &assistants::New<'_>,
    date: &str,
    cli: &str,
) -> voice_core::error::Result<assistants::Listed> {
    let made = assistants::make(home, new, date, cli)?;
    if let Err(error) = clear_consent(home, &made.member.slug) {
        let _ = fs::remove_dir_all(&made.dir);
        return Err(error);
    }
    Ok(made)
}

/// Back up an older assistant before making a Voice assistant with its history as an input.
pub fn bring_in(
    home: &Path,
    claude_home: &Path,
    new: &assistants::New<'_>,
    date: &str,
    stamp: &str,
    cli: &str,
    files: &[String],
) -> voice_core::error::Result<(assistants::Listed, Option<PathBuf>, Vec<String>)> {
    bring_in_checked(
        home,
        claude_home,
        new,
        date,
        stamp,
        cli,
        files,
        &check_brought_member,
    )
}

#[allow(clippy::too_many_arguments)] // The public bring-in arguments plus its late-check seam.
fn bring_in_checked(
    home: &Path,
    claude_home: &Path,
    new: &assistants::New<'_>,
    date: &str,
    stamp: &str,
    cli: &str,
    files: &[String],
    after_copy: &dyn Fn(&Path, &Path) -> voice_core::error::Result<()>,
) -> voice_core::error::Result<(assistants::Listed, Option<PathBuf>, Vec<String>)> {
    let slug = crew::slugify(new.name)
        .ok_or_else(|| Error::Invalid("name must contain a letter or digit".into()))?;
    let found = existing::find(home, claude_home, &slug)?;
    if !files.is_empty() {
        let offered = found
            .old_folder
            .as_deref()
            .map(|folder| offer_old_files(home, folder))
            .transpose()?;
        let mut seen = std::collections::HashSet::new();
        for name in files {
            if !seen.insert(name)
                || !offered.as_ref().is_some_and(|offered| {
                    offered
                        .iter()
                        .any(|file| file.name == name && file.problem.is_none())
                })
            {
                return Err(old_files::refusal(name));
            }
        }
    }
    if !found.any() {
        return make_and_clear(home, new, date, cli).map(|made| (made, None, Vec::new()));
    }
    if let Some(sentence) = bring_in_refusal(home, &found)? {
        return Err(Error::Invalid(sentence));
    }
    let state = home.join(".claude/state").join(&slug);
    let _lock = match fs::symlink_metadata(&state) {
        Ok(_) => Some(voice_ingest::private::lock_existing_state(home, &slug)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(Error::Io {
                path: state,
                source,
            });
        }
    };
    let mut items = Vec::new();
    for (name, source, follow) in [
        (
            "state",
            found.state.as_ref().map(|state| state.path.as_path()),
            false,
        ),
        ("skill", found.skill.as_deref(), false),
        ("CLAUDE.md", found.claude_md.as_deref(), true),
        ("member", found.old_folder.as_deref(), false),
    ] {
        if let Some(source) = source {
            items.push(voice_core::backup::Item {
                name,
                source,
                follow,
            });
        }
    }
    let backup = voice_core::backup::back_up(home, &slug, stamp, &items)?;
    let corpus = state.join("decisions.jsonl");
    let before = state.join("decisions-before-voice.jsonl");
    let mut moved = false;
    let mut cleared = false;
    let crew = assistants::crew_dir(home);
    let crew_existed = fs::symlink_metadata(&crew).is_ok();
    let result = (|| {
        if fs::symlink_metadata(&corpus).is_ok() {
            fs::hard_link(&corpus, &before).map_err(|source| Error::Io {
                path: before.clone(),
                source,
            })?;
            if let Err(source) = fs::remove_file(&corpus) {
                let _ = fs::remove_file(&before);
                return Err(Error::Io {
                    path: corpus.clone(),
                    source,
                });
            }
            moved = true;
        }
        let made = make_and_clear(home, new, date, cli)?;
        cleared = true;
        if !files.is_empty() {
            let copied = old_files::bring(&backup.join("member"), &made.dir, files, &|value| {
                private_ok(home, value)
            });
            let checked = copied.and_then(|()| {
                after_copy(home, &made.dir)
                    .map_err(|_| Error::Invalid(old_files::CANT_BRING.into()))
            });
            if let Err(error) = checked {
                let _ = fs::remove_dir_all(&made.dir);
                return Err(error);
            }
        }
        Ok(made)
    })();
    match result {
        Ok(made) => Ok((
            made,
            Some(backup),
            old_files::NAMES
                .into_iter()
                .filter(|name| files.iter().any(|file| file == name))
                .map(str::to_owned)
                .collect(),
        )),
        Err(error) => {
            if !crew_existed {
                // Only remove a parent this call created, and only while it is empty.
                let _ = fs::remove_dir(&crew);
            }
            let consent = backup.join("state/consent.json");
            let consent_restored = !cleared
                || fs::symlink_metadata(&consent).is_err()
                || fs::rename(&consent, state.join("consent.json")).is_ok();
            let restored = !moved || fs::hard_link(&before, &corpus).is_ok();
            if restored && consent_restored {
                if moved {
                    let _ = fs::remove_file(&before);
                }
                let _ = fs::remove_dir_all(&backup);
            }
            Err(error)
        }
    }
}

pub fn bring_in_assistant_in(
    home: &Path,
    claude_home: &Path,
    request: dto::BringInRequest,
) -> Result<dto::BroughtIn> {
    let files = request.files;
    let request = request.assistant;
    let slug = checked_request(&request)?;
    let cli = cli_word(home)?;
    let new = assistants::New {
        name: &request.name,
        pronouns: &request.pronouns,
        principal: &request.principal,
        principal_pronouns: &request.principal_pronouns,
    };
    let now = SystemTime::now();
    let (made, backup, files) = bring_in(
        home,
        claude_home,
        &new,
        &local_day(now),
        &voice_agent::ledger::local_stamp(now),
        &cli,
        &files,
    )
    .map_err(|error| match error {
        Error::Invalid(sentence)
            if sentence == HISTORY_BEFORE
                || sentence == CORPUS_NOT_PLAIN
                || sentence == state_refusal(&slug)
                || voice_core::backup::says(&sentence)
                || old_files::says(&sentence) =>
        {
            failure(&sentence)
        }
        Error::Exists(_) => already_have(&request.name),
        _ => failure(MAKE),
    })?;
    Ok(dto::BroughtIn {
        files,
        assistant: assistant(home, made),
        backup: backup.map(|path| {
            // back_up always makes this folder beneath the supplied home.
            format!(
                "~/{}",
                path.strip_prefix(home)
                    .expect("backup beneath home")
                    .display()
            )
        }),
    })
}

pub fn bring_in_assistant(request: dto::BringInRequest) -> Result<dto::BroughtIn> {
    let home = home(MAKE)?;
    bring_in_assistant_in(
        &home,
        &crate::use_it::claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
        request,
    )
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
    open(&dir).map_err(|_| failure("Voice couldn't open the folder."))
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
        note: None,
        can_sign_in: false,
        can_sign_in_with_subscription: false,
    }
}
fn subscription_refused_status() -> dto::ClaudeStatus {
    dto::ClaudeStatus {
        ready: false,
        message: text::SUBSCRIPTION_REFUSED.into(),
        note: None,
        can_sign_in: true,
        can_sign_in_with_subscription: claude::SUBSCRIPTION_OFFERED,
    }
}
fn unavailable_status(error: Error) -> dto::ClaudeStatus {
    dto::ClaudeStatus {
        ready: false,
        message: claude_message(error),
        note: None,
        can_sign_in: false,
        can_sign_in_with_subscription: false,
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
        Ok(status) if status.subscription_refused => subscription_refused_status(),
        Ok(status) => dto::ClaudeStatus {
            ready: false,
            message: text::signed_out_at(
                &status.version,
                &text::shown_program(claude.program(), home),
            ),
            note: None,
            can_sign_in: true,
            can_sign_in_with_subscription: claude::SUBSCRIPTION_OFFERED,
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
pub fn clear_consent(home: &Path, slug: &str) -> voice_core::error::Result<bool> {
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
                text::SUBSCRIPTION_REFUSED,
                text::SUBSCRIPTION_NOT_OFFERED,
                text::PLAN_STOPPED,
                text::CONSOLE_NOT_AVAILABLE,
                text::SIGN_OUT_FAILED,
                text::SIGN_OUT_FILE_LEFT,
                text::SIGN_IN_FAILED,
                text::SIGN_IN_TIMED_OUT,
                text::SIGN_IN_STOPPED,
                text::PROFILE_FAILED,
                text::NOT_NATIVE,
                text::MAC_ONLY,
                text::RUN_FOLDER_FAILED,
                text::PROXY_FAILED,
            ]
            .contains(&message.as_str())
                || text::SERVER_MESSAGES.contains(&message.as_str())
                || message.starts_with(text::SERVER_CUT_OFF_START)
                || message.starts_with("Nothing was sent.") =>
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

/// The single Claude sign-in or sign-out currently running.
#[derive(Default)]
pub struct ClaudeSignIns {
    state: Mutex<Option<Arc<AtomicBool>>>,
    signing_out: AtomicBool,
}
impl ClaudeSignIns {
    pub fn new() -> Self {
        Self::default()
    }
}
pub const SIGN_IN_BUSY: &str =
    "Voice is already waiting for you to sign in on Claude's page in your browser.";
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
/// Clears the marker even when a child fails or a call unwinds.
struct ClaudeSignOutGuard<'a>(&'a ClaudeSignIns);
impl Drop for ClaudeSignOutGuard<'_> {
    fn drop(&mut self) {
        self.0.signing_out.store(false, Ordering::SeqCst);
    }
}
fn stopped_sign_in_status() -> dto::ClaudeStatus {
    dto::ClaudeStatus {
        ready: false,
        message: text::SIGN_IN_STOPPED.into(),
        note: None,
        can_sign_in: true,
        can_sign_in_with_subscription: claude::SUBSCRIPTION_OFFERED,
    }
}
pub fn claude_sign_in_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    sign_ins: &ClaudeSignIns,
    request: &dto::ClaudeSignInRequest,
) -> Result<dto::ClaudeStatus> {
    let sign_in = if request.subscription {
        claude::SignIn::Subscription
    } else {
        claude::SignIn::Console
    };
    let flag = {
        let mut state = sign_ins.state.lock().unwrap_or_else(|p| p.into_inner());
        if sign_ins.signing_out.load(Ordering::SeqCst) {
            return Err(failure(text::SIGN_IN_DURING_SIGN_OUT));
        }
        if request.subscription && !claude::SUBSCRIPTION_OFFERED {
            return Ok(dto::ClaudeStatus {
                ready: false,
                message: text::SUBSCRIPTION_NOT_OFFERED.into(),
                note: None,
                can_sign_in: true,
                can_sign_in_with_subscription: false,
            });
        }
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
    if flag.load(Ordering::SeqCst) {
        return Ok(stopped_sign_in_status());
    }
    let result = claude.sign_in(home, &flag, sign_in);
    if let Err(Error::Invalid(message)) = &result
        && message == text::SIGN_IN_STOPPED
    {
        return Ok(stopped_sign_in_status());
    }
    if let Ok(status) = claude.status(home) {
        if status.signed_in {
            return Ok(ready_status(status));
        }
        if status.subscription_refused {
            return Ok(subscription_refused_status());
        }
    }
    Ok(dto::ClaudeStatus {
        ready: false,
        message: result
            .err()
            .map(claude_message)
            .unwrap_or_else(|| text::SIGN_IN_FAILED.into()),
        note: None,
        can_sign_in: true,
        can_sign_in_with_subscription: claude::SUBSCRIPTION_OFFERED,
    })
}
pub fn claude_sign_out_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    sign_ins: &ClaudeSignIns,
) -> Result<dto::ClaudeStatus> {
    {
        // Check and reserve under the same lock as sign-in, without holding it over child runs.
        let state = sign_ins.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.is_some() {
            return Err(failure(text::SIGN_OUT_DURING_SIGN_IN));
        }
        if sign_ins.signing_out.swap(true, Ordering::SeqCst) {
            return Err(failure(text::SIGN_OUT_BUSY));
        }
    }
    let _guard = ClaudeSignOutGuard(sign_ins);
    let claude =
        claude::find_in(home, path, fallbacks).map_err(|error| failure(&claude_message(error)))?;
    claude
        .sign_out(home)
        .map_err(|error| failure(&claude_message(error)))?;
    let mut status = claude_check_in(home, path, fallbacks)?;
    status.note = Some(text::SIGN_OUT_SUCCESS.into());
    Ok(status)
}
pub fn claude_sign_out() -> Result<dto::ClaudeStatus> {
    let home = home(text::SIGN_OUT_FAILED)?;
    claude_sign_out_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        claude_sign_ins(),
    )
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
pub fn claude_sign_in(request: dto::ClaudeSignInRequest) -> Result<dto::ClaudeStatus> {
    let Ok(home) = home(text::FAILED) else {
        return Ok(unavailable_status(Error::Invalid(text::FAILED.into())));
    };
    claude_sign_in_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        claude_sign_ins(),
        &request,
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

#[cfg(test)]
mod server_message_tests {
    use super::*;

    #[test]
    fn model_server_failures_cross_the_app_boundary() {
        for message in [text::SERVER_UNREACHABLE.to_owned(), text::server_cut_off(2)] {
            assert_eq!(claude_message(Error::Invalid(message.clone())), message);
        }
    }
}

#[cfg(test)]
mod bring_in_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Scratch(pub PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "voice-bring-in-api-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, (String, Vec<u8>)> {
        fn walk(
            root: &Path,
            at: &Path,
            out: &mut std::collections::BTreeMap<PathBuf, (String, Vec<u8>)>,
        ) {
            let meta = fs::symlink_metadata(at).unwrap();
            let (kind, bytes) = if meta.is_symlink() {
                (
                    "link",
                    fs::read_link(at)
                        .unwrap()
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                )
            } else if meta.is_dir() {
                ("folder", vec![])
            } else if meta.is_file() {
                ("file", fs::read(at).unwrap())
            } else {
                ("special", vec![])
            };
            out.insert(
                at.strip_prefix(root).unwrap().to_owned(),
                (kind.into(), bytes),
            );
            if meta.is_dir() {
                for entry in fs::read_dir(at).unwrap() {
                    walk(root, &entry.unwrap().path(), out);
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn late_check_failure_restores_history_consent_and_removes_new_crew_and_backup() {
        let scratch = Scratch::new();
        let home = scratch.path();
        let new = assistants::New {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
        };
        let old = assistants::make(&home.join("old"), &new, "2026-11-12", "voice").unwrap();
        fs::write(old.dir.join("IDENTITY.md"), "older identity\n").unwrap();
        let claude = home.join(".claude");
        let skill = claude.join("skills/ari");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            format!("--member {}", old.dir.join("member.json").display()),
        )
        .unwrap();
        let state = claude.join("state/ari");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("decisions.jsonl"), "older history\n").unwrap();
        fs::write(state.join("consent.json"), "older permission bytes").unwrap();
        let before = snapshot(home);
        let error = bring_in_checked(
            home,
            &claude,
            &new,
            "2026-11-12",
            "2026-11-12-100000",
            "voice",
            &["IDENTITY.md".into()],
            &|actual_home, dir| {
                assert_eq!(actual_home, home);
                assert_eq!(
                    fs::read(dir.join("IDENTITY.md")).unwrap(),
                    b"older identity\n"
                );
                assert!(!state.join("consent.json").exists());
                assert!(!state.join("decisions.jsonl").exists());
                Err(Error::Invalid("invented late failure".into()))
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), old_files::CANT_BRING);
        let mut after = snapshot(home);
        assert_eq!(
            after.remove(Path::new(".claude/voice-backups")),
            Some(("folder".into(), vec![]))
        );
        assert_eq!(
            after.remove(Path::new(".claude/voice-backups/ari")),
            Some(("folder".into(), vec![]))
        );
        assert_eq!(after, before);
        assert!(!claude.join("crew/ari").exists());
        assert_eq!(
            fs::read_dir(claude.join("voice-backups/ari"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn authoritative_check_rejects_broken_runtime_privacy_settings() {
        let scratch = Scratch::new();
        let home = scratch.path();
        let made = assistants::make(
            home,
            &assistants::New {
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
            },
            "2026-11-12",
            "voice",
        )
        .unwrap();
        for private in [r#"{"filters":{"secret":"("}}"#, r#"{"anonymizer":7}"#] {
            fs::write(made.dir.join("private.json"), private).unwrap();
            assert!(check_brought_member(home, &made.dir).is_err());
        }
    }
}
