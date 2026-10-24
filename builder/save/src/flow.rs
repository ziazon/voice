use crate::{
    git::{self, Env, FoundRepo, Git, Identity, Remote},
    github::{Github, RepoRef, github_repo_of, valid_repo_name},
    keychain::TokenStore,
    record::{SaveRecord, read_record, write_record},
    secret::Secret,
    text,
};
use persona_builder::{
    error::{Error, Result},
    member::read_member,
};
use std::path::{Path, PathBuf};

pub trait PrivacyCheck {
    fn hits(&self, folder: &Path) -> Result<Vec<String>>;
}
pub struct NotBuiltYet;
impl PrivacyCheck for NotBuiltYet {
    fn hits(&self, _folder: &Path) -> Result<Vec<String>> {
        Err(Error::Invalid(text::PRIVACY_NOT_BUILT.into()))
    }
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
    pub member: persona_builder::member::Member,
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
    persona_builder::crew::shown_path(root, home).unwrap_or_else(|_| root.display().to_string())
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
        write_record(&member.state_dir, &SaveRecord::Local)
    }
    pub fn choose_existing(
        &self,
        member: &MemberPaths,
        home: &Path,
        root: &Path,
        remotes: &dyn RemoteCheck,
    ) -> Result<SaveRecord> {
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
        let record = read_record(&member.state_dir)?
            .ok_or_else(|| Error::Invalid(text::CHOOSE_FIRST.into()))?;
        if record == SaveRecord::Local {
            return Ok(Saved::KeptLocal);
        }
        let hits = privacy.hits(&member.dir)?;
        if !hits.is_empty() {
            let mut places = hits.iter().take(20).cloned().collect::<Vec<_>>().join(", ");
            if hits.len() > 20 {
                places.push_str(&format!(" and {} more", hits.len() - 20));
            }
            return Err(Error::Invalid(text::privacy_hits(&places)));
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
            &format!("Save {}", member.member.name),
        )?;
        git::push(self.git, env, &git_dir, &remote, &branch, push_token)?;
        if !committed.changed {
            message.push(' ');
            message.push_str(text::UP_TO_DATE);
        }
        Ok(Saved::Pushed { message })
    }
}
