use crate::{secret::Secret, text};
use base64::{Engine, engine::general_purpose::STANDARD};
use persona_builder::error::{Error, Result};
use std::{
    fs,
    io::Write,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct Git {
    program: PathBuf,
}
impl Git {
    pub fn system() -> Self {
        Self::at("git".into())
    }
    pub fn at(program: PathBuf) -> Self {
        Self { program }
    }
    fn command(&self, env: Env) -> Command {
        let mut command = Command::new(&self.program);
        command.env_clear();
        for name in ["HOME", "PATH"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command.env("LC_ALL", "C").env("GIT_TERMINAL_PROMPT", "0");
        match env {
            Env::User => {
                if let Some(value) = std::env::var_os("SSH_AUTH_SOCK") {
                    command.env("SSH_AUTH_SOCK", value);
                }
            }
            Env::Own => {
                command
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", "/dev/null")
                    .args(["-c", "credential.helper="]);
            }
        }
        command.stdin(Stdio::null());
        command
    }
    pub fn check_version(&self) -> Result<()> {
        let output = self
            .command(Env::User)
            .arg("--version")
            .output()
            .map_err(|_| Error::Invalid(text::NEEDS_GIT.into()))?;
        let version = String::from_utf8_lossy(&output.stdout);
        let mut parts = version
            .strip_prefix("git version ")
            .unwrap_or("")
            .split('.');
        let major = parts.next().and_then(|s| s.parse::<u32>().ok());
        let minor = parts.next().and_then(|s| s.parse::<u32>().ok());
        if output.status.success()
            && matches!((major, minor), (Some(major), Some(minor)) if major > 2 || major == 2 && minor >= 32)
        {
            Ok(())
        } else {
            Err(Error::Invalid(text::GIT_TOO_OLD.into()))
        }
    }
    pub fn find_repo(&self, folder: &Path) -> Result<Option<FoundRepo>> {
        if !folder.exists() {
            return Ok(None);
        }
        let output = self
            .command(Env::User)
            .arg("-C")
            .arg(folder)
            .args(["rev-parse", "--show-toplevel", "--absolute-git-dir"])
            .output()
            .map_err(|_| failed("rev-parse"))?;
        if !output.status.success() {
            return Ok(None);
        }
        let output = String::from_utf8_lossy(&output.stdout);
        let mut lines = output.lines();
        let root = PathBuf::from(lines.next().ok_or_else(|| failed("rev-parse"))?);
        let git_dir = PathBuf::from(lines.next().ok_or_else(|| failed("rev-parse"))?);
        let mut command = self.command(Env::User);
        command.arg("-C").arg(&root).arg("remote");
        let names = checked(&mut command, "remote", None)?;
        let mut remotes = Vec::new();
        for name in names.lines() {
            remotes.push(Remote {
                name: name.into(),
                push_urls: self
                    .run(
                        Env::User,
                        &git_dir,
                        "remote",
                        &["get-url", "--push", "--all", "--", name],
                    )?
                    .lines()
                    .map(str::to_owned)
                    .collect(),
            });
        }
        Ok(Some(FoundRepo {
            root,
            git_dir,
            remotes,
        }))
    }
    pub(crate) fn run(
        &self,
        env: Env,
        git_dir: &Path,
        subcommand: &str,
        args: &[&str],
    ) -> Result<String> {
        checked(
            self.repo_command(env, git_dir, subcommand).args(args),
            subcommand,
            None,
        )
    }
    fn repo_command(&self, env: Env, git_dir: &Path, subcommand: &str) -> Command {
        let mut command = self.command(env);
        command
            .arg(format!("--git-dir={}", git_dir.display()))
            .arg(subcommand);
        command
    }
    pub(crate) fn init_bare(&self, folder: &Path) -> Result<()> {
        checked(
            self.command(Env::Own)
                .args(["init", "-q", "--bare"])
                .arg(folder),
            "init",
            None,
        )?;
        Ok(())
    }
}

fn failed(subcommand: &str) -> Error {
    Error::Invalid(format!("git {subcommand} failed"))
}
fn output(command: &mut Command, subcommand: &str, input: Option<&[u8]>) -> Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().map_err(|_| failed(subcommand))?;
    let writer = input.map(|bytes| {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let bytes = bytes.to_vec();
        std::thread::spawn(move || stdin.write_all(&bytes))
    });
    let result = child.wait_with_output().map_err(|_| failed(subcommand));
    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| failed(subcommand))?
            .map_err(|_| failed(subcommand))?;
    }
    result
}
fn checked(command: &mut Command, subcommand: &str, input: Option<&[u8]>) -> Result<String> {
    let output = output(command, subcommand, input)?;
    if !output.status.success() {
        return Err(failed(subcommand));
    }
    String::from_utf8(output.stdout).map_err(|_| failed(subcommand))
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundRepo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub remotes: Vec<Remote>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub push_urls: Vec<String>,
}
#[derive(Clone, Copy)]
pub enum Env {
    User,
    Own,
}
pub struct Identity {
    pub name: String,
    pub email: String,
}
pub struct Committed {
    pub commit: String,
    pub changed: bool,
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "persona-save-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io(&path, error)),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn list(
    folder: &Path,
    directory: &Path,
    files: &mut Vec<String>,
    deny: &persona_builder::deny::DenyList,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|e| io(directory, e))?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| io(directory, e))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(folder).expect("child of source");
        let rel = relative
            .to_str()
            .filter(|s| !s.contains('\\') && !s.chars().any(char::is_control))
            .ok_or_else(|| Error::Invalid("unsupported file name".into()))?;
        if deny.denied_relative(relative) {
            return Err(Error::Invalid(text::denied_in_folder(rel)));
        }
        let kind = entry.file_type().map_err(|e| io(&path, e))?;
        if kind.is_symlink() {
            return Err(Error::Invalid(text::links_in_folder(rel)));
        }
        if kind.is_dir() {
            list(folder, &path, files, deny)?;
        } else if kind.is_file()
            && !matches!(
                entry.file_name().to_str(),
                Some("private.json" | ".DS_Store")
            )
        {
            files.push(rel.into());
        }
    }
    Ok(())
}

pub fn folder_files(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    list(
        folder,
        folder,
        &mut files,
        &persona_builder::deny::DenyList::builtin(),
    )?;
    let mut paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    paths.sort();
    Ok(paths)
}

/// Uses a separate index and git plumbing; see https://git-scm.com/docs/git-update-index
/// and https://git-scm.com/docs/git-commit-tree.
#[allow(clippy::too_many_arguments)] // Signature fixed by the PB6 brief.
pub fn commit_folder(
    git: &Git,
    env: Env,
    git_dir: &Path,
    branch: &str,
    folder: &Path,
    prefix: &str,
    who: &Identity,
    message: &str,
) -> Result<Committed> {
    if !(branch == "main" || branch.strip_prefix("persona/").is_some_and(slug))
        || !prefix.strip_prefix("crew/").is_some_and(slug)
    {
        return Err(Error::Invalid("bad branch or folder".into()));
    }
    let reference = format!("refs/heads/{branch}");
    let worktrees = checked(
        git.repo_command(env, git_dir, "worktree")
            .args(["list", "--porcelain"]),
        "worktree",
        None,
    )?;
    if worktrees
        .lines()
        .any(|line| line.strip_prefix("branch ") == Some(reference.as_str()))
    {
        return Err(Error::Invalid(text::branch_in_use(branch)));
    }
    let files = folder_files(folder)?;
    let folder = folder.canonicalize().map_err(|e| io(folder, e))?;
    let scratch = Scratch::new()?;
    let run = |subcommand: &str, args: &[&str], input: Option<&[u8]>| -> Result<String> {
        checked(
            git.repo_command(env, git_dir, subcommand)
                .env("GIT_INDEX_FILE", scratch.0.join("index"))
                .args(args),
            subcommand,
            input,
        )
    };
    let parent_output = output(
        git.repo_command(env, git_dir, "rev-parse").args([
            "--verify",
            "-q",
            &format!("{reference}^{{commit}}"),
        ]),
        "rev-parse",
        None,
    )?;
    let parent = if parent_output.status.success() {
        Some(
            String::from_utf8(parent_output.stdout)
                .map_err(|_| failed("rev-parse"))?
                .trim()
                .to_owned(),
        )
    } else {
        None
    };
    run("read-tree", &[parent.as_deref().unwrap_or("--empty")], None)?;
    let mut index = String::new();
    if let Some(parent) = &parent {
        let paths = run(
            "ls-tree",
            &[
                "-r",
                "-z",
                "--name-only",
                parent,
                "--",
                &format!("{prefix}/"),
            ],
            None,
        )?;
        for path in paths.split('\0').filter(|s| !s.is_empty()) {
            index.push_str(&format!("0 {}\t{path}\0", "0".repeat(40)));
        }
    }
    // Quote stdin paths using git's C-style path syntax, including clone paths with spaces.
    let paths: String = files
        .iter()
        .map(|rel| {
            let path = folder.join(rel);
            let path = path.to_string_lossy();
            format!(
                "\"{}\"\n",
                path.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t")
            )
        })
        .collect();
    let blobs = run(
        "hash-object",
        &["-w", "--no-filters", "--stdin-paths"],
        Some(paths.as_bytes()),
    )?;
    if blobs.lines().count() != files.len() {
        return Err(failed("hash-object"));
    }
    for (rel, blob) in files.iter().zip(blobs.lines()) {
        // NUL-delimited index entries preserve literal quotes and spaces.
        index.push_str(&format!("100644 {blob}\t{prefix}/{}\0", rel.display()));
    }
    run(
        "update-index",
        &["--add", "-z", "--index-info"],
        Some(index.as_bytes()),
    )?;
    let tree = run("write-tree", &[], None)?.trim().to_owned();
    if let Some(parent) = &parent
        && tree == run("rev-parse", &[&format!("{parent}^{{tree}}")], None)?.trim()
    {
        return Ok(Committed {
            commit: parent.clone(),
            changed: false,
        });
    }
    let mut command = git.repo_command(env, git_dir, "commit-tree");
    command
        .env("GIT_INDEX_FILE", scratch.0.join("index"))
        .args(["--no-gpg-sign", &tree]);
    if let Some(parent) = &parent {
        command.args(["-p", parent]);
    }
    command
        .args(["-F", "-"])
        .env("GIT_AUTHOR_NAME", &who.name)
        .env("GIT_AUTHOR_EMAIL", &who.email)
        .env("GIT_COMMITTER_NAME", &who.name)
        .env("GIT_COMMITTER_EMAIL", &who.email);
    let commit = checked(&mut command, "commit-tree", Some(message.as_bytes()))?
        .trim()
        .to_owned();
    run(
        "update-ref",
        &[&reference, &commit, parent.as_deref().unwrap_or("")],
        None,
    )?;
    Ok(Committed {
        commit,
        changed: true,
    })
}

/// The HTTPS header follows actions/checkout's git-auth-helper.ts:
/// https://github.com/actions/checkout/blob/main/src/git-auth-helper.ts
pub fn push(
    git: &Git,
    env: Env,
    git_dir: &Path,
    remote: &str,
    branch: &str,
    token: Option<&Secret>,
) -> Result<()> {
    let mut command = git.repo_command(env, git_dir, "push");
    command.args([
        "--porcelain",
        "--",
        remote,
        &format!("refs/heads/{branch}:refs/heads/{branch}"),
    ]);
    let credential =
        token.map(|token| STANDARD.encode(format!("x-access-token:{}", token.expose())));
    if let Some(credential) = &credential {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.https://github.com/.extraheader")
            .env(
                "GIT_CONFIG_VALUE_0",
                format!("AUTHORIZATION: basic {credential}"),
            );
    }
    let output = output(&mut command, "push", None)
        .map_err(|_| Error::Invalid(text::push_failed("git push failed")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut detail = stderr
        .lines()
        .rev()
        .find(|s| !s.trim().is_empty())
        .unwrap_or("")
        .to_owned();
    if let (Some(token), Some(credential)) = (token, credential) {
        detail = detail.replace(&credential, "[hidden]");
        if !token.expose().is_empty() {
            detail = detail.replace(token.expose(), "[hidden]");
        }
    }
    let mut start = 0;
    while let Some(scheme) = detail[start..].find("://") {
        let authority = start + scheme + 3;
        let end = detail[authority..]
            .find(['/', ' '])
            .map_or(detail.len(), |end| authority + end);
        if let Some(at) = detail[authority..end].find('@') {
            detail.replace_range(authority..authority + at + 1, "");
        }
        start = authority;
    }
    Err(Error::Invalid(text::push_failed(
        &detail.chars().take(300).collect::<String>(),
    )))
}
