use crate::{
    git::{self, Env, FoundRepo, Git, Identity, Remote},
    github::{Github, RepoRef, github_repo_of, valid_repo_name},
    keychain::TokenStore,
    record::{Role, SaveRecord, read_record, write_record},
    secret::Secret,
    text,
};
use std::path::{Path, PathBuf};
use voice_core::{
    error::{Error, Result},
    member::read_member,
};

pub trait PrivacyCheck {
    fn hits(&self, folder: &Path) -> Result<Vec<String>>;
}
pub trait RemoteCheck {
    fn confirm(&self, token: &Secret, push_urls: &[String]) -> Result<()>;
}
pub struct GithubRemotes<'a> {
    pub github: &'a Github<'a>,
}
impl RemoteCheck for GithubRemotes<'_> {
    fn confirm(&self, token: &Secret, push_urls: &[String]) -> Result<()> {
        if push_urls.is_empty() {
            return Err(Error::Invalid(text::NOT_ON_GITHUB.into()));
        }
        for url in push_urls {
            let repo =
                github_repo_of(url).ok_or_else(|| Error::Invalid(text::NOT_ON_GITHUB.into()))?;
            self.github.confirm_private(token, &repo)?;
        }
        Ok(())
    }
}
pub struct MemberPaths {
    pub member: voice_core::member::Member,
    pub dir: PathBuf,
    pub state_dir: PathBuf,
}
impl MemberPaths {
    pub fn load(member_json: &Path, home: &Path) -> Result<Self> {
        let member = read_member(member_json)?;
        let dir = member_json
            .parent()
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|source| Error::Io {
                path: member_json.into(),
                source,
            })?;
        let state_dir = match member.state_dir.strip_prefix("~/") {
            Some(relative) => home.join(relative),
            None => PathBuf::from(&member.state_dir),
        };
        if !state_dir.is_absolute() {
            return Err(Error::Invalid(
                "member config: state_dir must start with ~/ or /".into(),
            ));
        }
        Ok(Self {
            member,
            dir,
            state_dir,
        })
    }
}
pub enum Choice {
    Existing {
        root: PathBuf,
        remote: String,
        label: String,
    },
    Github {
        label: String,
    },
    Local {
        label: String,
    },
}
pub struct SaveOptions {
    pub choices: Vec<Choice>,
    pub selected: usize,
    pub note: Option<&'static str>,
}
fn usable_remote(repo: &FoundRepo) -> Option<&Remote> {
    let usable = |remote: &&Remote| {
        !remote.push_urls.is_empty()
            && remote
                .push_urls
                .iter()
                .all(|url| github_repo_of(url).is_some())
    };
    repo.remotes
        .iter()
        .find(|remote| remote.name == "origin" && usable(remote))
        .or_else(|| repo.remotes.iter().find(usable))
}
pub fn detect(git: &Git, home: &Path, member_dir: &Path) -> Result<Vec<FoundRepo>> {
    let mut found: Vec<FoundRepo> = Vec::new();
    for folder in [home.join(".claude"), member_dir.into()] {
        if let Some(repo) = git.find_repo(&folder)?
            && usable_remote(&repo).is_some()
            && !found.iter().any(|other| other.root == repo.root)
        {
            found.push(repo);
        }
    }
    Ok(found)
}
pub fn shown(root: &Path, home: &Path) -> String {
    voice_core::crew::shown_path(root, home).unwrap_or_else(|_| root.display().to_string())
}
pub fn options(found: &[FoundRepo], home: &Path) -> SaveOptions {
    let mut choices: Vec<Choice> = found
        .iter()
        .filter_map(|repo| {
            usable_remote(repo).map(|remote| Choice::Existing {
                root: repo.root.clone(),
                remote: remote.name.clone(),
                label: text::choice_existing(&shown(&repo.root, home)),
            })
        })
        .collect();
    let note = if choices.is_empty() {
        choices.push(Choice::Github {
            label: text::CHOICE_GITHUB.into(),
        });
        Some(text::NO_GITHUB_ACCOUNT)
    } else {
        None
    };
    choices.push(Choice::Local {
        label: text::CHOICE_LOCAL.into(),
    });
    SaveOptions {
        choices,
        selected: 0,
        note,
    }
}
pub struct Saver<'a> {
    pub git: &'a Git,
    pub github: Option<&'a Github<'a>>,
    pub tokens: &'a dyn TokenStore,
}
pub enum Saved {
    KeptLocal,
    Pushed { message: String },
}
impl Saver<'_> {
    pub fn choose_local(&self, member: &MemberPaths) -> Result<()> {
        refuse_member(member)?;
        write_record(&member.state_dir, &SaveRecord::Local)
    }
    pub fn choose_existing(
        &self,
        member: &MemberPaths,
        home: &Path,
        root: &Path,
        remotes: &dyn RemoteCheck,
    ) -> Result<SaveRecord> {
        refuse_member(member)?;
        let github = self
            .github
            .ok_or_else(|| Error::Invalid(text::NOT_SET_UP.into()))?;
        self.git.check_version()?;
        let found = detect(self.git, home, &member.dir)?;
        let root = root
            .canonicalize()
            .map_err(|_| Error::Invalid(text::NOT_OFFERED.into()))?;
        let repo = found
            .iter()
            .find(|repo| repo.root == root)
            .ok_or_else(|| Error::Invalid(text::NOT_OFFERED.into()))?;
        let remote = usable_remote(repo).ok_or_else(|| Error::Invalid(text::NOT_OFFERED.into()))?;
        let token = self
            .tokens
            .load()?
            .ok_or_else(|| Error::Invalid(text::SIGN_IN_FIRST.into()))?;
        github.account(&token)?;
        remotes.confirm(&token, &remote.push_urls)?;
        let record = SaveRecord::Existing {
            repo: root,
            remote: remote.name.clone(),
            branch: format!("persona/{}", member.member.slug),
        };
        write_record(&member.state_dir, &record)?;
        Ok(record)
    }
    pub fn choose_github(&self, member: &MemberPaths, name: &str) -> Result<RepoRef> {
        refuse_member(member)?;
        let github = self
            .github
            .ok_or_else(|| Error::Invalid(text::NOT_SET_UP.into()))?;
        if !valid_repo_name(name) {
            return Err(Error::Invalid(text::BAD_REPO_NAME.into()));
        }
        self.git.check_version()?;
        let token = self
            .tokens
            .load()?
            .ok_or_else(|| Error::Invalid(text::SIGN_IN_FIRST.into()))?;
        github.account(&token)?;
        let repo =
            if let Some(SaveRecord::Github { owner, name, .. }) = read_record(&member.state_dir)? {
                let repo = RepoRef { owner, name };
                github.confirm_private(&token, &repo)?;
                repo
            } else {
                github.create_private_repo(&token, name)?
            };
        let backup = member.state_dir.join("backup.git");
        if !backup.exists() {
            // The record writer establishes the state folder's private mode.
            // Delay writing the choice until git setup has succeeded.
            crate::record::create_state(&member.state_dir)?;
            self.git.init_bare(&backup)?;
        }
        let remotes = self.git.run(Env::Own, &backup, "remote", &[])?;
        let action = if remotes.lines().any(|name| name == "origin") {
            "set-url"
        } else {
            "add"
        };
        self.git.run(
            Env::Own,
            &backup,
            "remote",
            &[
                action,
                "origin",
                &format!("https://github.com/{}.git", repo.full()),
            ],
        )?;
        write_record(
            &member.state_dir,
            &SaveRecord::Github {
                owner: repo.owner.clone(),
                name: repo.name.clone(),
                branch: "main".into(),
                role: Role::Builder,
                builder: None,
            },
        )?;
        Ok(repo)
    }
    pub fn save_now(
        &self,
        member: &MemberPaths,
        home: &Path,
        privacy: &dyn PrivacyCheck,
        remotes: &dyn RemoteCheck,
    ) -> Result<Saved> {
        self.save_now_from(member, home, privacy, remotes, None)
    }
    pub fn save_now_from(
        &self,
        member: &MemberPaths,
        home: &Path,
        privacy: &dyn PrivacyCheck,
        remotes: &dyn RemoteCheck,
        computer: Option<&str>,
    ) -> Result<Saved> {
        refuse_member(member)?;
        let record = read_record(&member.state_dir)?
            .ok_or_else(|| Error::Invalid(text::CHOOSE_FIRST.into()))?;
        if record == SaveRecord::Local {
            return Ok(Saved::KeptLocal);
        }
        let hits = privacy.hits(&member.dir)?;
        if !hits.is_empty() {
            return Err(Error::Invalid(text::privacy_hits(&places(&hits))));
        }
        let github = self
            .github
            .ok_or_else(|| Error::Invalid(text::NOT_SET_UP.into()))?;
        self.git.check_version()?;
        let token = self
            .tokens
            .load()?
            .ok_or_else(|| Error::Invalid(text::SIGN_IN_FIRST.into()))?;
        let account = github.account(&token)?;
        let who = Identity {
            name: account.login.clone(),
            email: format!("{}+{}@users.noreply.github.com", account.id, account.login),
        };
        let (env, git_dir, remote, branch, push_token, mut message) = match record {
            SaveRecord::Existing {
                repo,
                remote,
                branch,
            } => {
                let found = self
                    .git
                    .find_repo(&repo)?
                    .filter(|found| {
                        found.root == repo && found.remotes.iter().any(|r| r.name == remote)
                    })
                    .ok_or_else(|| Error::Invalid(text::NOT_OFFERED.into()))?;
                let message = text::saved_existing(&shown(&repo, home), &branch);
                (Env::User, found.git_dir, remote, branch, None, message)
            }
            SaveRecord::Github {
                owner,
                name,
                branch,
                ..
            } => {
                let full = RepoRef { owner, name }.full();
                let backup = member.state_dir.join("backup.git");
                let expected = format!("https://github.com/{full}.git");
                if !backup.exists()
                    || !self
                        .git
                        .run(Env::Own, &backup, "remote", &["get-url", "--", "origin"])
                        .is_ok_and(|url| url.trim_end() == expected)
                {
                    return Err(Error::Invalid(text::repo_unconfirmed(&full)));
                }
                (
                    Env::Own,
                    backup,
                    "origin".into(),
                    branch,
                    Some(&token),
                    text::saved_github(&full),
                )
            }
            SaveRecord::Local => unreachable!("local returned before privacy check"),
        };
        let urls = self.git.run(
            env,
            &git_dir,
            "remote",
            &["get-url", "--push", "--all", "--", &remote],
        )?;
        remotes.confirm(&token, &urls.lines().map(str::to_owned).collect::<Vec<_>>())?;
        let committed = git::commit_folder(
            self.git,
            env,
            &git_dir,
            &branch,
            &member.dir,
            &format!("crew/{}", member.member.slug),
            &who,
            &match computer.filter(|value| crate::computer::valid_computer_name(value)) {
                Some(computer) => format!(
                    "Save {}\n\nSaved-on: {}\n",
                    member.member.name,
                    computer.trim()
                ),
                None => format!("Save {}", member.member.name),
            },
        )?;
        git::push(self.git, env, &git_dir, &remote, &branch, push_token)?;
        if !committed.changed {
            message.push(' ');
            message.push_str(text::UP_TO_DATE);
        }
        Ok(Saved::Pushed { message })
    }
}

fn refuse_member(member: &MemberPaths) -> Result<()> {
    if let Some(SaveRecord::Github {
        role: Role::Member,
        builder,
        ..
    }) = read_record(&member.state_dir).ok().flatten()
    {
        return Err(Error::Invalid(text::member_computer(
            &member.member.name,
            builder.as_deref(),
        )));
    }
    Ok(())
}

pub struct Staged {
    pub dir: PathBuf,
    pub member_dir: PathBuf,
    pub git_dir: PathBuf,
    pub member: voice_core::member::Member,
    pub builder: Option<String>,
}
fn open_io<T>(path: &Path, result: std::io::Result<T>) -> Result<T> {
    result.map_err(|source| Error::Io {
        path: path.into(),
        source,
    })
}
// Check every ancestor before creating anything: create_dir_all must never traverse a link.
fn plain_parents(home: &Path, path: &Path) -> Result<()> {
    let mut current = home.to_owned();
    for component in path
        .strip_prefix(home)
        .map_err(|_| Error::Invalid("invalid assistant folder".into()))?
        .components()
    {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(Error::Invalid(
                    "assistant folder isn't a plain folder".into(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return open_io(&current, Err(error)),
        }
    }
    Ok(())
}
impl Saver<'_> {
    pub fn stage_open(
        &self,
        home: &Path,
        repo: &RepoRef,
        slug: &str,
        stamp: &str,
        privacy: &dyn PrivacyCheck,
    ) -> Result<Staged> {
        use std::{fs, os::unix::fs::DirBuilderExt};
        if voice_core::crew::slugify(slug).as_deref() != Some(slug)
            || !crate::github::valid_owner(&repo.owner)
            || !valid_repo_name(&repo.name)
            || !voice_core::backup::valid_stamp(stamp)
        {
            return Err(Error::Invalid("invalid saved assistant".into()));
        }
        let github = self
            .github
            .ok_or_else(|| Error::Invalid(text::NOT_SET_UP.into()))?;
        self.git.check_version()?;
        let token = self
            .tokens
            .load()?
            .ok_or_else(|| Error::Invalid(text::SIGN_IN_FIRST.into()))?;
        github.account(&token)?;
        github.confirm_private(&token, repo)?;
        let crew = home.join(".claude/crew");
        plain_parents(home, &crew)?;
        let made_crew = fs::symlink_metadata(&crew).is_err();
        open_io(&crew, fs::create_dir_all(&crew))?;
        let dir = crew.join(format!(
            ".{slug}.voice-opening-{stamp}-{}",
            std::process::id()
        ));
        if let Err(error) = open_io(&dir, fs::DirBuilder::new().mode(0o700).create(&dir)) {
            if made_crew {
                let _ = fs::remove_dir(&crew);
            }
            return Err(error);
        }
        let result = (|| {
            let git_dir = dir.join("backup.git");
            self.git.init_bare(&git_dir)?;
            self.git.run(
                Env::Own,
                &git_dir,
                "remote",
                &[
                    "add",
                    "origin",
                    &format!("https://github.com/{}.git", repo.full()),
                ],
            )?;
            git::fetch_main(self.git, &git_dir, &token)?;
            let member_dir = dir.join("member");
            let entries = opening_entries(self.git, &git_dir, repo, slug)?;
            open_io(
                &member_dir,
                fs::DirBuilder::new().mode(0o700).create(&member_dir),
            )?;
            for (rel, oid, executable) in entries {
                write_opened_blob(self.git, &git_dir, &member_dir, &rel, &oid, executable)?;
            }
            let full = repo.full();
            match fs::symlink_metadata(member_dir.join("private.json")) {
                Ok(_) => {
                    return Err(Error::Invalid(text::saved_copy_refused(
                        &full,
                        "private.json",
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return open_io(&member_dir.join("private.json"), Err(error)),
            }
            let unusable = || Error::Invalid(text::saved_copy_unusable(&full));
            let member = read_member(&member_dir.join("member.json")).map_err(|_| unusable())?;
            if member.slug != slug
                || voice_core::member::private_state_dir(&member, home)
                    != Some(home.join(".claude/state").join(slug))
            {
                return Err(unusable());
            }
            voice_core::crew::check_name("name", &member.name)?;
            voice_core::crew::check_name("principal", &member.principal)?;
            let hits = privacy.hits(&member_dir)?;
            if !hits.is_empty() {
                return Err(Error::Invalid(text::saved_copy_private(
                    &repo.full(),
                    &places(&hits),
                )));
            }
            let trailer = self.git.run(
                Env::Own,
                &git_dir,
                "log",
                &[
                    "-1",
                    "--format=%(trailers:key=Saved-on,valueonly)",
                    "refs/heads/main",
                ],
            )?;
            let trailer = trailer.trim();
            let builder = crate::computer::valid_computer_name(trailer).then(|| trailer.to_owned());
            Ok(Staged {
                dir: dir.clone(),
                member_dir,
                git_dir,
                member,
                builder,
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&dir);
            if made_crew {
                let _ = fs::remove_dir(&crew);
            }
        }
        result
    }
}
fn places(hits: &[String]) -> String {
    let mut places = hits.iter().take(20).cloned().collect::<Vec<_>>().join(", ");
    if hits.len() > 20 {
        places.push_str(&format!(" and {} more", hits.len() - 20));
    }
    places
}
type OpeningEntry = (String, String, bool);
fn opening_entries(
    git: &Git,
    git_dir: &Path,
    repo: &RepoRef,
    slug: &str,
) -> Result<Vec<OpeningEntry>> {
    let prefix = format!("crew/{slug}/");
    let bytes = git.run_bytes(
        git_dir,
        "ls-tree",
        &["-r", "-z", "--full-tree", "refs/heads/main", "--", &prefix],
    )?;
    let full = repo.full();
    let refused = |rel: &str| {
        let shown = if rel.split('/').all(opening_part) {
            rel
        } else {
            "a file with an unusual name"
        };
        Error::Invalid(text::saved_copy_refused(&full, shown))
    };
    let large = || Error::Invalid(text::saved_copy_large(&full));
    let mut entries = Vec::new();
    let mut total = 0u64;
    let mut count = 0;
    let mut has_member = false;
    for entry in bytes.split(|b| *b == 0).filter(|entry| !entry.is_empty()) {
        count += 1;
        if count > 2000 {
            return Err(large());
        }
        let entry =
            std::str::from_utf8(entry).map_err(|_| refused("a file with an unusual name"))?;
        let (head, path) = entry
            .split_once('\t')
            .ok_or_else(|| refused("a file with an unusual name"))?;
        let rel = path.strip_prefix(&prefix).ok_or_else(|| refused(path))?;
        let mut head = head.split(' ');
        let mode = head.next().unwrap_or("");
        let kind = head.next().unwrap_or("");
        let oid = head.next().unwrap_or("");
        if rel.split('/').any(|part| part == ".DS_Store") {
            continue;
        }
        if !matches!(mode, "100644" | "100755")
            || kind != "blob"
            || rel.split('/').any(|part| !opening_part(part))
            || voice_core::deny::DenyList::builtin().denied_relative(Path::new(rel))
        {
            return Err(refused(rel));
        }
        // Refuse ASCII case aliases even on a case-sensitive staging disk.
        if rel
            .split('/')
            .any(|part| part != "private.json" && part.eq_ignore_ascii_case("private.json"))
        {
            return Err(refused("private.json"));
        }
        let size: u64 = git
            .run(Env::Own, git_dir, "cat-file", &["-s", oid])?
            .trim()
            .parse()
            .map_err(|_| refused(rel))?;
        total = total.checked_add(size).ok_or_else(large)?;
        if size > 10 * 1024 * 1024 || total > 50 * 1024 * 1024 {
            return Err(large());
        }
        if rel == "member.json" {
            has_member = true;
        }
        if rel
            .split('/')
            .any(|part| matches!(part, "private.json" | ".DS_Store"))
        {
            continue;
        }
        entries.push((rel.to_owned(), oid.to_owned(), mode == "100755"));
    }
    if !has_member {
        return Err(Error::Invalid(text::saved_copy_missing(&full, slug)));
    }
    Ok(entries)
}
fn write_opened_blob(
    git: &Git,
    git_dir: &Path,
    member_dir: &Path,
    rel: &str,
    oid: &str,
    executable: bool,
) -> Result<()> {
    use std::{
        fs,
        io::Write,
        os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    };
    let mut folder = member_dir.to_owned();
    let parts: Vec<_> = rel.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        folder.push(part);
        match fs::symlink_metadata(&folder) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(Error::Invalid("saved copy folder isn't plain".into())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                open_io(&folder, fs::DirBuilder::new().mode(0o700).create(&folder))?;
            }
            Err(error) => return open_io(&folder, Err(error)),
        }
    }
    let path = member_dir.join(rel);
    let bytes = git.run_bytes(git_dir, "cat-file", &["blob", oid])?;
    let mut file = open_io(
        &path,
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW)
            .mode(if executable { 0o755 } else { 0o644 })
            .open(&path),
    )?;
    open_io(&path, file.write_all(&bytes))
}
fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    #[cfg(target_os = "macos")]
    // SAFETY: both C strings remain live and flags request an exclusive rename.
    let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(target_os = "linux")]
    // SAFETY: both C strings remain live; AT_FDCWD resolves their absolute paths.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let from = Path::new(std::ffi::OsStr::from_bytes(from.as_bytes()));
        let to = Path::new(std::ffi::OsStr::from_bytes(to.as_bytes()));
        if std::fs::symlink_metadata(to).is_ok() {
            return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
        }
        std::fs::rename(from, to)
    }
}
pub fn place_opened(staged: &Staged, home: &Path, repo: &RepoRef) -> Result<MemberPaths> {
    use std::fs;
    let slug = &staged.member.slug;
    let member_dir = home.join(".claude/crew").join(slug);
    plain_parents(home, member_dir.parent().expect("crew parent"))?;
    if fs::symlink_metadata(&member_dir).is_ok() {
        return Err(Error::Exists(slug.clone()));
    }
    let state = home.join(".claude/state").join(slug);
    plain_parents(home, &state)?;
    let state_parent = state.parent().expect("state parent");
    let made_state_parent = fs::symlink_metadata(state_parent).is_err();
    let made_state = fs::symlink_metadata(&state).is_err();
    let result = (|| {
        crate::record::create_state(&state)?;
        let backup = state.join("backup.git");
        if [&backup, &state.join("save.json")]
            .iter()
            .any(|path| fs::symlink_metadata(path).is_ok())
        {
            return Err(Error::Invalid(text::state_in_use(slug)));
        }
        open_io(&backup, rename_new(&staged.git_dir, &backup))?;
        if let Err(error) = rename_new(&staged.member_dir, &member_dir) {
            open_io(&staged.git_dir, rename_new(&backup, &staged.git_dir))?;
            return if error.kind() == std::io::ErrorKind::AlreadyExists {
                Err(Error::Exists(slug.clone()))
            } else {
                open_io(&member_dir, Err(error))
            };
        }
        if let Err(error) = write_record(
            &state,
            &SaveRecord::Github {
                owner: repo.owner.clone(),
                name: repo.name.clone(),
                branch: "main".into(),
                role: Role::Member,
                builder: staged.builder.clone(),
            },
        ) {
            open_io(
                &staged.member_dir,
                rename_new(&member_dir, &staged.member_dir),
            )?;
            open_io(&staged.git_dir, rename_new(&backup, &staged.git_dir))?;
            return Err(error);
        }
        let _ = fs::remove_dir(&staged.dir);
        MemberPaths::load(&member_dir.join("member.json"), home)
    })();
    if result.is_err() {
        if made_state {
            let _ = fs::remove_dir(&state);
        }
        if made_state_parent {
            let _ = fs::remove_dir(state_parent);
        }
    }
    result
}

pub(crate) fn opening_part(part: &str) -> bool {
    (1..=255).contains(&part.len())
        && !part.starts_with(['.', ' '])
        && !part.ends_with(['.', ' '])
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._- ".contains(&b))
}
#[cfg(test)]
mod opening_tests {
    use super::opening_part;
    #[test]
    fn only_plain_ascii_path_parts_are_opened() {
        for part in [
            ".git",
            ".GIT",
            ".hidden",
            "a.",
            "a ",
            " a",
            "é.md",
            "private.jſon",
            "bad\n\x1b",
            "a/b",
            "a\\b",
            "",
            ".",
            "..",
        ] {
            assert!(!opening_part(part), "{part:?}");
        }
        assert!(!opening_part(&"a".repeat(256)));
        for part in ["notes", "identity.md", "A file_1-2.md", &"a".repeat(255)] {
            assert!(opening_part(part), "{part:?}");
        }
    }
}
