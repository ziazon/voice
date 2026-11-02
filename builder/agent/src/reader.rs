//! One sandboxed run of Voice's own reader, the shipped `voice` command; see
//! docs/spikes/VO11-os-sandbox.md.

use crate::{sandbox, spawn, text};
#[cfg(target_os = "macos")]
use std::collections::BTreeSet;
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};
use voice_core::{
    deny::DenyList,
    error::{Error, Result},
};

const PROFILE: &str = include_str!("../sandbox/reader.sb");
pub const MAX_SOURCES: usize = 256;

/// A place the person agreed Voice may read, as the caller found it.
pub enum Source<'a> {
    Folder(&'a Path),
    File(&'a Path),
}

#[cfg(target_os = "macos")]
struct Bounds {
    home: PathBuf,
    member: PathBuf,
    state: PathBuf,
    runs: PathBuf,
}

#[cfg(target_os = "macos")]
fn bounds(home: &Path, member: &Path, state: &Path, runs: &Path) -> Result<Bounds> {
    let folder_error = || Error::Invalid(text::READER_FOLDER.into());
    let member = fs::canonicalize(member).map_err(|_| folder_error())?;
    if !member.is_dir() {
        return Err(folder_error());
    }
    sandbox::private_folder(state).map_err(|_| folder_error())?;
    let state = fs::canonicalize(state).map_err(|_| folder_error())?;
    sandbox::private_folder(&state).map_err(|_| folder_error())?;
    let canonical_home = fs::canonicalize(home).map_err(|_| folder_error())?;
    let canonical_runs = match fs::canonicalize(runs) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = runs.parent().ok_or_else(folder_error)?;
            let name = runs.file_name().ok_or_else(folder_error)?;
            fs::canonicalize(parent)
                .map_err(|_| folder_error())?
                .join(name)
        }
        Err(_) => return Err(folder_error()),
    };
    let overlaps = |a: &Path, b: &Path| a.starts_with(b) || b.starts_with(a);
    if overlaps(&member, &state)
        || overlaps(&member, &canonical_runs)
        || overlaps(&state, &canonical_runs)
    {
        return Err(folder_error());
    }
    Ok(Bounds {
        home: canonical_home,
        member,
        state,
        runs: canonical_runs,
    })
}

#[cfg(target_os = "macos")]
fn accepted(folder: bool, path: &Path, bounds: &Bounds, deny: &DenyList) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let canonical = fs::canonicalize(path).ok()?;
    let metadata = fs::metadata(&canonical).ok()?;
    let builtin = DenyList::builtin();
    if (if folder {
        !metadata.is_dir()
    } else {
        !metadata.is_file()
    }) || canonical == Path::new("/")
        || canonical == bounds.home
        || [&bounds.home, &bounds.member, &bounds.state, &bounds.runs]
            .iter()
            .any(|allowed| *allowed != &canonical && allowed.starts_with(&canonical))
        || canonical.starts_with(&bounds.state)
        || canonical.starts_with(&bounds.runs)
        || builtin.denied(path)
        || builtin.denied(&canonical)
        || deny.denied(path)
        || deny.denied(&canonical)
    {
        return None;
    }
    Some(canonical)
}

/// The sources the reader can accept, in order, up to its source limit.
pub fn usable<'a>(
    home: &Path,
    member: &Path,
    state: &Path,
    runs: &Path,
    sources: &[Source<'a>],
    deny: &DenyList,
) -> Vec<Source<'a>> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (home, member, state, runs, sources, deny);
        Vec::new()
    }
    #[cfg(target_os = "macos")]
    {
        let Ok(bounds) = bounds(home, member, state, runs) else {
            return Vec::new();
        };
        let mut kept = Vec::new();
        let mut seen = Vec::new();
        for source in sources {
            let (folder, path) = match source {
                Source::Folder(path) => (true, *path),
                Source::File(path) => (false, *path),
            };
            if let Some(canonical) = accepted(folder, path, &bounds, deny) {
                let key = (folder, canonical);
                if !seen.contains(&key) {
                    seen.push(key);
                    kept.push(if folder {
                        Source::Folder(path)
                    } else {
                        Source::File(path)
                    });
                    if kept.len() == MAX_SOURCES {
                        break;
                    }
                }
            }
        }
        kept
    }
}

pub struct Reader {
    program: PathBuf,
    member: PathBuf,
    state: PathBuf,
    run: PathBuf,
    sources: Vec<(bool, PathBuf)>,
    ancestors: Vec<PathBuf>,
    home: PathBuf,
}

impl Reader {
    #[cfg(not(target_os = "macos"))]
    pub fn new(
        _program: &Path,
        _home: &Path,
        _member: &Path,
        _state: &Path,
        _runs: &Path,
        _sources: &[Source<'_>],
        _deny: &DenyList,
    ) -> Result<Reader> {
        Err(Error::Invalid(text::MAC_ONLY.into()))
    }

    #[cfg(target_os = "macos")]
    pub fn new(
        program: &Path,
        home: &Path,
        member: &Path,
        state: &Path,
        runs: &Path,
        sources: &[Source<'_>],
        deny: &DenyList,
    ) -> Result<Reader> {
        let program = sandbox::resolve_program(program)?;
        let bounds = bounds(home, member, state, runs)?;
        let source_error = || Error::Invalid(text::READER_SOURCE.into());
        if sources.len() > MAX_SOURCES {
            return Err(source_error());
        }
        let mut kept = Vec::new();
        for source in sources {
            let (folder, path) = match source {
                Source::Folder(path) => (true, *path),
                Source::File(path) => (false, *path),
            };
            let canonical = accepted(folder, path, &bounds, deny).ok_or_else(source_error)?;
            let source = (folder, canonical);
            if !kept.contains(&source) {
                kept.push(source);
            }
        }
        let run = sandbox::fresh_folder_in(runs)
            .map_err(|_| Error::Invalid(text::RUN_FOLDER_FAILED.into()))?;
        let mut ancestors = BTreeSet::new();
        for path in [&program, &bounds.member, &bounds.state, &run]
            .into_iter()
            .chain(kept.iter().map(|(_, path)| path))
        {
            ancestors.extend(path.ancestors().skip(1).map(Path::to_owned));
        }
        if ancestors.is_empty() {
            let _ = fs::remove_dir_all(&run);
            return Err(Error::Invalid(text::RUN_FOLDER_FAILED.into()));
        }
        Ok(Reader {
            program,
            member: bounds.member,
            state: bounds.state,
            run,
            sources: kept,
            ancestors: ancestors.into_iter().collect(),
            home: home.to_owned(),
        })
    }

    pub fn run_folder(&self) -> &Path {
        &self.run
    }

    pub fn profile(&self) -> String {
        let mut rules = String::new();
        if !self.sources.is_empty() {
            rules.push_str("; Sources the person agreed to, read-only.\n(allow file-read*");
            for (index, (folder, _)) in self.sources.iter().enumerate() {
                let filter = if *folder { "subpath" } else { "literal" };
                rules.push_str(&format!("\n    ({filter} (param \"SOURCE_{index}\"))"));
            }
            rules.push_str(")\n");
        }
        if !self.ancestors.is_empty() {
            rules.push_str("; The folders above each allowed path, metadata only, so the reader can resolve paths.\n(allow file-read-metadata");
            for index in 0..self.ancestors.len() {
                rules.push_str(&format!("\n    (literal (param \"ANCESTOR_{index}\"))"));
            }
            rules.push(')');
        }
        PROFILE.replacen(";; sources", &rules, 1)
    }

    fn launch(&self, args: &[&str]) -> (Vec<OsString>, Vec<(OsString, OsString)>, &Path) {
        let mut argv = vec![OsString::from("-p"), self.profile().into()];
        for (name, path) in [
            ("PROGRAM", &self.program),
            ("MEMBER_DIR", &self.member),
            ("STATE_DIR", &self.state),
            ("RUN_DIR", &self.run),
        ] {
            let mut parameter = OsString::from(format!("{name}="));
            parameter.push(path);
            argv.extend([OsString::from("-D"), parameter]);
        }
        for (index, (_, path)) in self.sources.iter().enumerate() {
            let mut parameter = OsString::from(format!("SOURCE_{index}="));
            parameter.push(path);
            argv.extend([OsString::from("-D"), parameter]);
        }
        for (index, path) in self.ancestors.iter().enumerate() {
            let mut parameter = OsString::from(format!("ANCESTOR_{index}="));
            parameter.push(path);
            argv.extend([OsString::from("-D"), parameter]);
        }
        argv.push(self.program.as_os_str().to_owned());
        argv.extend(args.iter().map(OsString::from));
        let mut env = vec![(OsString::from("HOME"), self.home.as_os_str().to_owned())];
        if let Some(login) = sandbox::login_name() {
            env.extend([
                (OsString::from("USER"), login.clone().into()),
                (OsString::from("LOGNAME"), login.into()),
            ]);
        }
        env.extend([
            (
                OsString::from("PATH"),
                OsString::from("/usr/bin:/bin:/usr/sbin:/sbin"),
            ),
            (OsString::from("LANG"), sandbox::language().into()),
        ]);
        let mut tmp = self.run.as_os_str().to_owned();
        tmp.push("/");
        env.push((OsString::from("TMPDIR"), tmp));
        (argv, env, &self.run)
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let (argv, env, dir) = self.launch(args);
        let mut command = Command::new(sandbox::SANDBOX_EXEC);
        command.args(argv).env_clear().envs(env).current_dir(dir);
        command
    }

    pub fn spawn(&self, args: &[&str]) -> io::Result<spawn::Spawned> {
        let (argv, env, dir) = self.launch(args);
        spawn::spawn(Path::new(sandbox::SANDBOX_EXEC), &argv, &env, dir)
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.run);
    }
}
