//! Saving boundary: private state and credentials stay outside the webview.
use crate::{
    assistants::{failure, home, listed},
    dto,
};
use std::{
    ffi::OsString,
    path::Path,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};
use voice_core::error::{Error, Result as CoreResult};
use voice_save::{
    flow::{self, GithubRemotes, MemberPaths, RemoteCheck, Saved, Saver},
    git::Git,
    github::{self, DeviceCode, Endpoints, Github, UreqHttp},
    keychain::{Keychain, TokenStore},
    record::{self, Role, SaveRecord},
    secret::Secret,
    text,
};

type Result<T> = std::result::Result<T, dto::Failure>;
const OPTIONS: &str =
    "Voice couldn't check where your assistant can be kept. Close it and open it again.";
const CHANGED: &str =
    "The places to keep your assistant have changed. Look at them again, then choose.";
const SIGN_IN: &str =
    "GitHub sign-in didn't work. Try again later, or keep your assistant on this computer only.";
const START: &str = "Start signing in to GitHub again.";
const SIGN_OUT: &str = "Voice couldn't sign you out of GitHub. Try again.";
const CHOOSE: &str = "Voice couldn't keep your choice. Close it and open it again, then try again.";
const SAVE: &str =
    "Voice couldn't save your assistant. Try again, or keep it on this computer only.";
pub const OPEN_SAVED_FAILED: &str = "Voice couldn't open that saved assistant. Try again.";
const SIGNUP_PAGE: &str = "https://github.com/signup";
const DEVICE_PAGE: &str = "https://github.com/login/device";
fn said(message: impl Into<String>) -> dto::Said {
    dto::Said {
        message: message.into(),
    }
}

pub struct FolderPrivacy;
impl flow::PrivacyCheck for FolderPrivacy {
    fn hits(&self, folder: &Path) -> CoreResult<Vec<String>> {
        let files = voice_save::git::folder_files(folder)?;
        let member = voice_core::member::read_member(&folder.join("member.json"))?;
        let filters = voice_ingest::filters::Filters::compile(&member.filters)?;
        Ok(voice_ingest::check::scan(folder, &files, &filters)?.hits)
    }
}
pub struct UnavailableRemotes;
impl RemoteCheck for UnavailableRemotes {
    fn confirm(&self, _token: &Secret, _urls: &[String]) -> CoreResult<()> {
        Err(Error::Invalid(text::NOT_SET_UP.into()))
    }
}

pub struct SignIns {
    state: Mutex<SignInState>,
}
struct SignInState {
    code: Option<DeviceCode>,
    cancel: Arc<AtomicBool>,
}
impl SignIns {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SignInState {
                code: None,
                cancel: Arc::new(AtomicBool::new(false)),
            }),
        }
    }
}
impl Default for SignIns {
    fn default() -> Self {
        Self::new()
    }
}
pub fn sign_ins() -> &'static SignIns {
    static SIGN_INS: std::sync::OnceLock<SignIns> = std::sync::OnceLock::new();
    SIGN_INS.get_or_init(SignIns::new)
}
fn map_error(error: Error, fallback: &str) -> dto::Failure {
    match error {
        Error::Invalid(message) if text::is_said(&message) => failure(&message),
        _ => failure(fallback),
    }
}
fn member(home: &Path, slug: &str, fallback: &str) -> Result<MemberPaths> {
    let listed = listed(home, slug)?;
    MemberPaths::load(&listed.dir.join("member.json"), home)
        .map_err(|error| map_error(error, fallback))
}
pub fn save_options_in(
    home: &Path,
    request: dto::SlugRequest,
    git: &Git,
    tokens: &dyn TokenStore,
) -> Result<dto::SaveOptions> {
    let member = member(home, &request.slug, OPTIONS)?;
    let found = flow::detect(git, home, &member.dir).map_err(|error| map_error(error, OPTIONS))?;
    let options = flow::options(&found, home);
    let record = record::read_record(&member.state_dir).ok().flatten();
    let member_computer = match &record {
        Some(SaveRecord::Github {
            role: Role::Member,
            builder,
            ..
        }) => Some(text::member_computer(
            &member.member.name,
            builder.as_deref(),
        )),
        _ => None,
    };
    let chosen = match record {
        Some(SaveRecord::Local) => Some(text::CHOSE_LOCAL.into()),
        Some(SaveRecord::Existing { repo, .. }) => {
            Some(text::chose_existing(&flow::shown(&repo, home)))
        }
        Some(SaveRecord::Github { owner, name, .. }) => {
            Some(text::chose_github(&format!("{owner}/{name}")))
        }
        None => None,
    };
    Ok(dto::SaveOptions {
        member_computer,
        choices: options
            .choices
            .into_iter()
            .map(|choice| match choice {
                flow::Choice::Existing { label, .. } => dto::SaveOption {
                    kind: dto::SaveKind::Existing,
                    label,
                },
                flow::Choice::Github { label } => dto::SaveOption {
                    kind: dto::SaveKind::Github,
                    label,
                },
                flow::Choice::Local { label } => dto::SaveOption {
                    kind: dto::SaveKind::Local,
                    label,
                },
            })
            .collect(),
        selected: options.selected as u32,
        note: options.note.map(str::to_owned),
        chosen,
        signed_in: matches!(tokens.load(), Ok(Some(_))),
        suggested_name: github::suggested_repo_name(&request.slug),
    })
}
pub fn open_link_in(
    request: dto::LinkRequest,
    open: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<()> {
    let (url, fallback) = match request.link {
        dto::Link::GithubSignup => (
            SIGNUP_PAGE,
            "Voice couldn't open your web browser. Go to https://github.com/signup yourself.",
        ),
        dto::Link::GithubDevice => (
            DEVICE_PAGE,
            "Voice couldn't open your web browser. Go to https://github.com/login/device yourself.",
        ),
    };
    open(url).map_err(|_| failure(fallback))
}
pub fn github_sign_in_start_in(
    sign_ins: &SignIns,
    github: Option<&Github<'_>>,
) -> Result<dto::SignInCode> {
    let mut state = sign_ins
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.cancel.store(true, Ordering::SeqCst);
    state.code = None;
    let github = github.ok_or_else(|| failure(text::NOT_SET_UP))?;
    let code = github
        .start_sign_in()
        .map_err(|error| map_error(error, SIGN_IN))?;
    let answer = dto::SignInCode {
        user_code: code.user_code.clone(),
    };
    state.code = Some(code);
    state.cancel = Arc::new(AtomicBool::new(false));
    Ok(answer)
}
pub fn github_sign_in_finish_in(
    sign_ins: &SignIns,
    github: Option<&Github<'_>>,
    tokens: &dyn TokenStore,
    sleep: &mut dyn FnMut(Duration),
) -> Result<dto::Said> {
    let (code, cancel) = {
        let mut state = sign_ins
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            state.code.take().ok_or_else(|| failure(START))?,
            Arc::clone(&state.cancel),
        )
    };
    let github = github.ok_or_else(|| failure(text::NOT_SET_UP))?;
    let mut wait = |duration: Duration| -> CoreResult<()> {
        let check = || {
            if cancel.load(Ordering::SeqCst) {
                Err(Error::Invalid(text::SIGN_IN_CANCELLED.into()))
            } else {
                Ok(())
            }
        };
        let mut remaining = duration;
        while !remaining.is_zero() {
            check()?;
            let step = remaining.min(Duration::from_millis(250));
            sleep(step);
            remaining -= step;
        }
        check()
    };
    let token = github
        .finish_sign_in_until(&code, &mut wait)
        .map_err(|error| map_error(error, SIGN_IN))?;
    let account = github
        .account(&token)
        .map_err(|error| map_error(error, SIGN_IN))?;
    let _state = sign_ins
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cancel.load(Ordering::SeqCst) {
        return Err(failure(text::SIGN_IN_CANCELLED));
    }
    tokens
        .save(&token)
        .map_err(|error| map_error(error, SIGN_IN))?;
    Ok(said(text::signed_in(&account.login)))
}
pub fn github_sign_in_cancel_in(sign_ins: &SignIns) -> Result<()> {
    let mut state = sign_ins
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.cancel.store(true, Ordering::SeqCst);
    state.code = None;
    Ok(())
}
pub fn github_sign_out_in(sign_ins: &SignIns, tokens: &dyn TokenStore) -> Result<dto::Said> {
    let mut state = sign_ins
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.cancel.store(true, Ordering::SeqCst);
    state.code = None;
    tokens
        .forget()
        .map_err(|error| map_error(error, SIGN_OUT))?;
    Ok(said(text::SIGNED_OUT))
}
pub fn save_choose_in(
    home: &Path,
    request: dto::SaveChoiceRequest,
    git: &Git,
    github: Option<&Github<'_>>,
    tokens: &dyn TokenStore,
) -> Result<dto::Said> {
    let member = member(home, &request.slug, CHOOSE)?;
    let found = flow::detect(git, home, &member.dir).map_err(|error| map_error(error, CHOOSE))?;
    let options = flow::options(&found, home);
    let choice = options
        .choices
        .get(request.choice as usize)
        .filter(|choice| {
            matches!(
                (choice, &request.kind),
                (flow::Choice::Existing { .. }, dto::SaveKind::Existing)
                    | (flow::Choice::Github { .. }, dto::SaveKind::Github)
                    | (flow::Choice::Local { .. }, dto::SaveKind::Local)
            )
        })
        .ok_or_else(|| failure(CHANGED))?;
    let saver = Saver {
        git,
        github,
        tokens,
    };
    let message = match choice {
        flow::Choice::Local { .. } => {
            saver
                .choose_local(&member)
                .map_err(|error| map_error(error, CHOOSE))?;
            text::CHOSE_LOCAL.into()
        }
        flow::Choice::Existing { root, .. } => {
            let github = github.ok_or_else(|| failure(text::NOT_SET_UP))?;
            saver
                .choose_existing(&member, home, root, &GithubRemotes { github })
                .map_err(|error| map_error(error, CHOOSE))?;
            text::chose_existing(&flow::shown(root, home))
        }
        flow::Choice::Github { .. } => {
            let name = request
                .name
                .as_deref()
                .ok_or_else(|| failure(text::BAD_REPO_NAME))?;
            let repo = saver
                .choose_github(&member, name)
                .map_err(|error| map_error(error, CHOOSE))?;
            text::chose_github(&repo.full())
        }
    };
    Ok(said(message))
}
pub fn save_push_in(
    home: &Path,
    request: dto::SlugRequest,
    git: &Git,
    github: Option<&Github<'_>>,
    tokens: &dyn TokenStore,
) -> Result<dto::Said> {
    let member = member(home, &request.slug, SAVE)?;
    let remotes = github.map(|github| GithubRemotes { github });
    let check: &dyn RemoteCheck = match &remotes {
        Some(remotes) => remotes,
        None => &UnavailableRemotes,
    };
    let saved = Saver {
        git,
        github,
        tokens,
    }
    .save_now_from(
        &member,
        home,
        &FolderPrivacy,
        check,
        voice_save::computer::name().as_deref(),
    )
    .map_err(|error| map_error(error, SAVE))?;
    Ok(said(match saved {
        Saved::KeptLocal => text::KEPT_LOCAL.into(),
        Saved::Pushed { message } => message,
    }))
}

/// List saved assistants without exposing the sign-in token to the app.
pub fn saved_on_github_in(
    home: &Path,
    github: Option<&Github<'_>>,
    tokens: &dyn TokenStore,
) -> Result<dto::SavedOnGithub> {
    let Some(token) = tokens
        .load()
        .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?
    else {
        return Ok(dto::SavedOnGithub {
            signed_in: false,
            items: Vec::new(),
        });
    };
    let github = github.ok_or_else(|| failure(text::NOT_SET_UP))?;
    let items = github
        .saved_assistants(&token)
        .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?
        .into_iter()
        .map(|item| {
            let here = voice_core::assistants::find(home, &item.slug)
                .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?
                .is_some();
            Ok(dto::SavedAssistant {
                owner: item.repo.owner,
                repo: item.repo.name,
                slug: item.slug,
                name: item.name,
                here,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(dto::SavedOnGithub {
        signed_in: true,
        items,
    })
}

pub fn open_saved_in(
    home: &Path,
    claude_home: &Path,
    request: dto::OpenSavedRequest,
    git: &Git,
    github: Option<&Github<'_>>,
    tokens: &dyn TokenStore,
) -> Result<dto::OpenedSaved> {
    let repo = github::RepoRef {
        owner: request.owner,
        name: request.repo,
    };
    let stamp = voice_agent::ledger::local_stamp(SystemTime::now());
    let staged = Saver {
        git,
        github,
        tokens,
    }
    .stage_open(home, &repo, &request.slug, &stamp, &FolderPrivacy)
    .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?;
    let result = (|| {
        crate::assistants::check_brought_member(home, &staged.member_dir)
            .map_err(|_| failure(&text::saved_copy_unusable(&repo.full())))?;
        let name = &staged.member.name;
        let already = || {
            failure(&format!(
                "You already have an assistant called {name} on this computer, so Voice opened nothing. Forget that one first, then open the saved one again."
            ))
        };
        let dir = voice_core::assistants::assistant_dir(home, &request.slug)
            .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?;
        if voice_core::assistants::find(home, &request.slug)
            .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?
            .is_some()
            || std::fs::symlink_metadata(&dir).is_ok()
        {
            return Err(already());
        }
        let found = voice_core::existing::find(home, claude_home, &request.slug)
            .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?;
        let refusal = crate::assistants::bring_in_refusal(home, &found)
            .map_err(|error| map_error(error, OPEN_SAVED_FAILED))?;
        let (made, backup) = crate::assistants::back_up_older_then(home, claude_home, &request.slug, &stamp, &mut || {
            let (placed, made_state, made_state_parent) = flow::place_opened(&staged, home, &repo)?;
            if let Err(error) = crate::assistants::clear_consent(home, &request.slug) {
                // Remove only what place_opened made; leave the older state intact.
                let _ = std::fs::remove_dir_all(&placed.dir);
                let _ = std::fs::remove_dir_all(placed.state_dir.join("backup.git"));
                let _ = std::fs::remove_file(placed.state_dir.join("save.json"));
                if made_state {
                    let _ = std::fs::remove_dir(&placed.state_dir);
                }
                if made_state_parent {
                    let _ = std::fs::remove_dir(home.join(".claude/state"));
                }
                return Err(error);
            }
            Ok(voice_core::assistants::Listed { dir: placed.dir, member: placed.member })
        }).map_err(|error| match error {
            Error::Invalid(sentence) if refusal.as_deref() == Some(&sentence) => {
                if found.state.as_ref().is_some_and(|state| state.link || state.not_folder) {
                    failure(&format!("~/.claude/state/{} is a file or a link, not a folder, so Voice opened nothing. Move it somewhere else, then try again.", request.slug))
                } else {
                    failure(&format!("Voice can't open {name} here yet: ~/.claude/state/{}/ holds files it can't back up. Move that folder somewhere else, then try again.", request.slug))
                }
            },
            Error::Exists(_) => already(),
            other => map_error(other, OPEN_SAVED_FAILED),
        })?;
        let backup = backup.map(|path| {
            format!(
                "~/{}",
                path.strip_prefix(home)
                    .expect("backup beneath home")
                    .display()
            )
        });
        let mut message = format!(
            "Opened {name} from {}. Voice asks for your permission again on this computer before {name} reads anything.",
            repo.full()
        );
        if let Some(path) = &backup {
            message.push_str(&format!(
                " Voice backed up the older {name} first, in {path}/."
            ));
        }
        Ok(dto::OpenedSaved {
            assistant: crate::assistants::assistant(home, made),
            backup,
            message,
        })
    })();
    let _ = std::fs::remove_dir_all(&staged.dir);
    if result.is_err() && staged.made_crew {
        let _ = std::fs::remove_dir(home.join(".claude/crew"));
    }
    result
}

pub fn saved_on_github() -> Result<dto::SavedOnGithub> {
    let home = home(OPEN_SAVED_FAILED)?;
    with_github(OPEN_SAVED_FAILED, |github| {
        saved_on_github_in(&home, github, &Keychain::github())
    })
}

pub fn open_saved(request: dto::OpenSavedRequest) -> Result<dto::OpenedSaved> {
    let home = home(OPEN_SAVED_FAILED)?;
    with_github(OPEN_SAVED_FAILED, |github| {
        open_saved_in(
            &home,
            &crate::use_it::claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
            request,
            &Git::system(),
            github,
            &Keychain::github(),
        )
    })
}

pub fn take_newest_in(
    home: &Path,
    claude_home: &Path,
    request: dto::SlugRequest,
    git: &Git,
    github: Option<&Github<'_>>,
    tokens: &dyn TokenStore,
) -> Result<dto::TookNewest> {
    let lock = voice_ingest::private::lock_state(home, &request.slug);
    let found = listed(home, &request.slug)?;
    let name = &found.member.name;
    let fallback = format!("Voice couldn't take the newest version of {name}. Try again.");
    let _lock = lock.map_err(|error| map_error(error, &fallback))?;
    let member = MemberPaths::load(&found.dir.join("member.json"), home)
        .map_err(|error| map_error(error, &fallback))?;
    let took = Saver {
        git,
        github,
        tokens,
    }
    .take_newest(
        home,
        &member,
        &voice_agent::ledger::local_stamp(SystemTime::now()),
        &FolderPrivacy,
    )
    .map_err(|error| map_error(error, &fallback))?;
    let (changed, mut message) = match took {
        flow::Took::AlreadyNewest { full } => (
            0,
            format!("{name} is already the newest version saved in {full}."),
        ),
        flow::Took::Updated { full, changed } => {
            let files = if changed == 1 {
                "1 file".into()
            } else {
                format!("{changed} files")
            };
            (
                changed,
                format!("Took the newest version of {name} from {full}: {files} changed."),
            )
        }
    };
    if changed > 0 {
        let places = voice_core::install::Places {
            home: home.into(),
            claude_home: claude_home.into(),
        };
        if voice_core::install::status(&places)
            .ok()
            .flatten()
            .is_some_and(|status| crate::use_it::is_this(&status, &found))
        {
            if voice_core::install::turn_on(&member.dir, &places).is_ok() {
                message.push_str(&format!(" Voice refreshed {name} in Claude Code too."));
            } else {
                message.push_str(&format!(
                    " Voice couldn't refresh {name} in Claude Code; turn {name} on again to do it."
                ));
            }
        }
    }
    Ok(dto::TookNewest {
        assistant: crate::assistants::assistant(home, listed(home, &request.slug)?),
        changed: changed as u32,
        message,
    })
}

pub fn take_newest(request: dto::SlugRequest) -> Result<dto::TookNewest> {
    let fallback = "Voice couldn't take the newest version of your assistant. Try again.";
    let home = home(fallback)?;
    with_github(fallback, |github| {
        take_newest_in(
            &home,
            &crate::use_it::claude_home(&home, std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()),
            request,
            &Git::system(),
            github,
            &Keychain::github(),
        )
    })
}

fn with_github<T>(fallback: &str, run: impl FnOnce(Option<&Github<'_>>) -> Result<T>) -> Result<T> {
    with_github_in(github::client_id(), fallback, run)
}
fn with_github_in<T>(
    client_id: Option<&str>,
    fallback: &str,
    run: impl FnOnce(Option<&Github<'_>>) -> Result<T>,
) -> Result<T> {
    let http = UreqHttp::default();
    let github = client_id
        .map(|id| Github::new(&http, Endpoints::github(), id))
        .transpose()
        .map_err(|error| map_error(error, fallback))?;
    run(github.as_ref())
}
pub fn save_options(request: dto::SlugRequest) -> Result<dto::SaveOptions> {
    save_options_in(
        &home(OPTIONS)?,
        request,
        &Git::system(),
        &Keychain::github(),
    )
}
pub fn open_link(request: dto::LinkRequest) -> Result<()> {
    open_link_in(request, |url| {
        let (program, args) = link_opener_args(url);
        Command::new(program).args(args).status().map(|_| ())
    })
}
pub fn github_sign_in_start() -> Result<dto::SignInCode> {
    with_github(SIGN_IN, |github| {
        github_sign_in_start_in(sign_ins(), github)
    })
}
pub fn github_sign_in_finish() -> Result<dto::Said> {
    with_github(SIGN_IN, |github| {
        github_sign_in_finish_in(
            sign_ins(),
            github,
            &Keychain::github(),
            &mut std::thread::sleep,
        )
    })
}
pub fn github_sign_in_cancel() -> Result<()> {
    github_sign_in_cancel_in(sign_ins())
}
pub fn github_sign_out() -> Result<dto::Said> {
    github_sign_out_in(sign_ins(), &Keychain::github())
}
pub fn save_choose(request: dto::SaveChoiceRequest) -> Result<dto::Said> {
    let home = home(CHOOSE)?;
    with_github(CHOOSE, |github| {
        save_choose_in(&home, request, &Git::system(), github, &Keychain::github())
    })
}
pub fn save_push(request: dto::SlugRequest) -> Result<dto::Said> {
    let home = home(SAVE)?;
    with_github(SAVE, |github| {
        save_push_in(&home, request, &Git::system(), github, &Keychain::github())
    })
}
fn link_opener_args(url: &str) -> (&'static str, Vec<OsString>) {
    #[cfg(target_os = "macos")]
    return ("/usr/bin/open", vec![url.into()]);
    #[cfg(target_os = "linux")]
    return ("xdg-open", vec![url.into()]);
    #[cfg(target_os = "windows")]
    return ("explorer", vec![url.into()]);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bad_client_id_reports_not_set_up() {
        let error = with_github_in(Some("invalid-client-id"), SIGN_IN, |_| -> Result<()> {
            panic!("an invalid client ID must not reach the command")
        })
        .unwrap_err();
        assert_eq!(error.message, text::NOT_SET_UP);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_link_is_opened_as_only_argument() {
        assert_eq!(
            link_opener_args(DEVICE_PAGE),
            ("/usr/bin/open", vec![DEVICE_PAGE.into()])
        );
    }
}
