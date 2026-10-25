//! Saving boundary: private state and credentials stay outside the webview.
use crate::{
    assistants::{failure, home, listed},
    dto,
};
use persona_builder::error::{Error, Result as CoreResult};
use persona_builder_save::{
    flow::{self, GithubRemotes, MemberPaths, RemoteCheck, Saved, Saver},
    git::Git,
    github::{self, DeviceCode, Endpoints, Github, UreqHttp},
    keychain::{Keychain, TokenStore},
    record::{self, SaveRecord},
    secret::Secret,
    text,
};
use std::{
    ffi::OsString,
    path::Path,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

type Result<T> = std::result::Result<T, dto::Failure>;
const OPTIONS: &str =
    "Persona Builder couldn't check where your assistant can be kept. Close it and open it again.";
const CHANGED: &str =
    "The places to keep your assistant have changed. Look at them again, then choose.";
const SIGN_IN: &str =
    "GitHub sign-in didn't work. Try again later, or keep your assistant on this computer only.";
const START: &str = "Start signing in to GitHub again.";
const SIGN_OUT: &str = "Persona Builder couldn't sign you out of GitHub. Try again.";
const CHOOSE: &str =
    "Persona Builder couldn't keep your choice. Close it and open it again, then try again.";
const SAVE: &str =
    "Persona Builder couldn't save your assistant. Try again, or keep it on this computer only.";
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
        let files = persona_builder_save::git::folder_files(folder)?;
        let member = persona_builder::member::read_member(&folder.join("member.json"))?;
        let filters = persona_builder_ingest::filters::Filters::compile(&member.filters)?;
        Ok(persona_builder_ingest::check::scan(folder, &files, &filters)?.hits)
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
    let chosen = match record::read_record(&member.state_dir).ok().flatten() {
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
            "Persona Builder couldn't open your web browser. Go to https://github.com/signup yourself.",
        ),
        dto::Link::GithubDevice => (
            DEVICE_PAGE,
            "Persona Builder couldn't open your web browser. Go to https://github.com/login/device yourself.",
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
    .save_now(&member, home, &FolderPrivacy, check)
    .map_err(|error| map_error(error, SAVE))?;
    Ok(said(match saved {
        Saved::KeptLocal => text::KEPT_LOCAL.into(),
        Saved::Pushed { message } => message,
    }))
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
