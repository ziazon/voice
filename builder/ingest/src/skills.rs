//! Skills discovered on this computer and the private permission to copy them.
use crate::{anonymize::Rules, private};
use serde_json::{Value, json};
use voice_core::{
    deny::DenyList,
    error::{Error, Result},
};
mod copy;
mod mirror;
pub use copy::{Copied, copy_agreed, load_copy};
pub use mirror::{MIRROR_REMOVED, Manifest, Refreshed, refresh};
mod output;
mod read;
pub use output::{
    prepare_output, prepare_output_with_projects, remove_copy, write_manifest, write_skill,
};
pub use read::{read, read_with_rules};

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    Claude,
    Synced,
    Plugin,
    Project,
    Codex,
}
#[derive(Debug, Clone)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub root: Root,
    pub folder: PathBuf,
    pub shown: String,
}
#[derive(Debug, Default)]
pub struct Found {
    pub skills: Vec<Skill>,
    pub notes: Vec<String>,
}
#[derive(Debug)]
pub enum Source {
    Bytes(Vec<u8>),
    Link,
    TooLarge,
    Unreadable,
}
pub const SKILL_TOO_LARGE: &str =
    "it has more than 500 files, more than 20 MB or more than 8 folders inside each other";
pub const SKILL_UNREADABLE: &str = "a folder in it couldn't be read";
const PLUGIN_NOTE: &str =
    "Couldn't read Claude Code's list of plugins, so their skills aren't listed.";
fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}
fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
pub fn display_path(path: &Path, home: &Path) -> String {
    path.strip_prefix(home)
        .or_else(|original| {
            home.canonicalize()
                .ok()
                .and_then(|canonical| path.strip_prefix(canonical).ok())
                .ok_or(original)
        })
        .map(|p| Path::new("~").join(p))
        .unwrap_or_else(|_| path.into())
        .to_string_lossy()
        .into_owned()
}
fn children(path: &Path) -> Result<Vec<(String, PathBuf)>> {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(io(path, e)),
    };
    let mut paths = vec![];
    for entry in entries {
        let entry = entry.map_err(|e| io(path, e))?;
        if let Some(name) = entry.file_name().to_str().filter(|n| !n.starts_with('.')) {
            paths.push((name.to_owned(), entry.path()));
        }
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(paths)
}
fn add(
    found: &mut BTreeMap<String, Skill>,
    notes: &mut Vec<String>,
    home: &Path,
    root: Root,
    prefix: &str,
    name: &str,
    path: &Path,
) {
    let Ok(folder) = path.canonicalize() else {
        return;
    };
    if DenyList::builtin().denied(&folder) || !folder.is_dir() {
        return;
    }
    match fs::metadata(folder.join("SKILL.md")) {
        Ok(metadata) if metadata.is_file() => {}
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            let note =
                "A skill folder on this computer couldn't be read, so it isn't listed.".to_owned();
            if !notes.contains(&note) {
                notes.push(note);
            }
            return;
        }
        _ => return,
    }
    let id = format!("{prefix}:{name}");
    found.entry(id.clone()).or_insert_with(|| Skill {
        id,
        name: name.into(),
        root,
        shown: display_path(&folder, home),
        folder,
    });
}
fn root_skills(
    found: &mut BTreeMap<String, Skill>,
    notes: &mut Vec<String>,
    home: &Path,
    root: Root,
    prefix: &str,
    path: &Path,
) -> Result<()> {
    for (name, path) in children(path)? {
        if root == Root::Claude && name == "synced" {
            continue;
        }
        if root == Root::Plugin {
            let Some(install) = path
                .parent()
                .and_then(Path::parent)
                .and_then(|p| p.canonicalize().ok())
            else {
                continue;
            };
            if !path
                .canonicalize()
                .is_ok_and(|folder| folder.starts_with(install))
            {
                continue;
            }
        }
        add(found, notes, home, root, prefix, &name, &path);
    }
    Ok(())
}
fn plugin_roots(home: &Path) -> Result<Vec<(String, PathBuf)>> {
    let base = home.join(".claude/plugins");
    let path = base.join("installed_plugins.json");
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err(invalid(PLUGIN_NOTE)),
    };
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= 4 << 20)
    {
        return Err(invalid(PLUGIN_NOTE));
    }
    let mut bytes = vec![];
    (&mut file)
        .take((4 << 20) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid(PLUGIN_NOTE))?;
    if bytes.len() > 4 << 20 {
        return Err(invalid(PLUGIN_NOTE));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| invalid(PLUGIN_NOTE))?;
    let plugins = value
        .get("plugins")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(PLUGIN_NOTE))?;
    let base = base.canonicalize().map_err(|_| invalid(PLUGIN_NOTE))?;
    let mut roots = vec![];
    let mut entries = plugins.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(k, _)| *k);
    for (key, entries) in entries {
        let entries = entries.as_array().ok_or_else(|| invalid(PLUGIN_NOTE))?;
        let plugin = key.split('@').next().unwrap_or(key);
        for entry in entries {
            let Some(path) = entry.get("installPath").and_then(Value::as_str) else {
                continue;
            };
            let Ok(path) = Path::new(path).canonicalize() else {
                continue;
            };
            if path.is_dir() && path.starts_with(&base) && !DenyList::builtin().denied(&path) {
                roots.push((plugin.into(), path));
            }
        }
    }
    Ok(roots)
}
pub fn discover(home: &Path, rules: &Rules) -> Result<Found> {
    let mut skills = BTreeMap::new();
    let mut notes = vec![];
    root_skills(
        &mut skills,
        &mut notes,
        home,
        Root::Claude,
        "claude",
        &home.join(".claude/skills"),
    )?;
    for (_, account) in children(&home.join(".claude/skills/synced"))? {
        if !account.is_dir() || DenyList::builtin().denied(&account) {
            continue;
        }
        for (name, path) in children(&account)? {
            let id = format!("synced:{name}");
            let existed = skills.contains_key(&id);
            add(
                &mut skills,
                &mut notes,
                home,
                Root::Synced,
                "synced",
                &name,
                &path,
            );
            if existed && path.join("SKILL.md").is_file() && !DenyList::builtin().denied(&path) {
                let note = "Some claude.ai skills are in more than one synced folder; each is listed once, from the first folder by name.".to_owned();
                if !notes.contains(&note) {
                    notes.push(note);
                }
            }
        }
    }
    match plugin_roots(home) {
        Ok(roots) => {
            for (name, path) in roots {
                root_skills(
                    &mut skills,
                    &mut notes,
                    home,
                    Root::Plugin,
                    &format!("plugin:{name}"),
                    &path.join("skills"),
                )?
            }
        }
        Err(_) => notes.push(PLUGIN_NOTE.into()),
    }
    for project in rules.projects() {
        let Some(name) = project.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if DenyList::builtin().denied(project) {
            continue;
        }
        root_skills(
            &mut skills,
            &mut notes,
            home,
            Root::Project,
            &format!("project:{name}"),
            &project.join(".claude/skills"),
        )?;
    }
    root_skills(
        &mut skills,
        &mut notes,
        home,
        Root::Codex,
        "codex",
        &home.join(".codex/skills"),
    )?;
    Ok(Found {
        skills: skills.into_values().collect(),
        notes,
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    NeverCopied,
    Agreed(String),
    Moved,
    NotOffered,
    Waiting,
}
impl State {
    pub fn words(&self) -> String {
        match self {
            Self::NeverCopied => "never copied: on your never_copy list".into(),
            Self::Agreed(date) => format!("agreed {date}"),
            Self::Moved => "moved since you agreed: agree again to copy it".into(),
            Self::NotOffered => "a claude.ai skill: not offered, agree by name to copy it".into(),
            Self::Waiting => "waiting for your yes".into(),
        }
    }
}
#[derive(Debug, Default)]
pub struct Agreements {
    records: BTreeMap<String, (String, String)>,
}
impl Agreements {
    pub fn load(home: &Path, slug: &str) -> Result<Self> {
        let Some(bytes) = private::read_private_optional_at_max(
            home,
            slug,
            Some("anonymizer"),
            "skills.json",
            4 << 20,
        )?
        else {
            return Ok(Self::default());
        };
        let bad = || {
            invalid(
                "The anonymizer's skills.json file can't be read. Move it away and agree to your skills again.",
            )
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| bad())?;
        let object = value.as_object().ok_or_else(bad)?;
        if object.len() != 2 || object.get("version") != Some(&json!(1)) {
            return Err(bad());
        }
        let entries = object
            .get("skills")
            .and_then(Value::as_object)
            .ok_or_else(bad)?;
        let mut records = BTreeMap::new();
        for (id, value) in entries {
            let object = value.as_object().ok_or_else(bad)?;
            if object.len() != 2 {
                return Err(bad());
            }
            let date = object
                .get("agreed_on")
                .and_then(Value::as_str)
                .ok_or_else(bad)?;
            let folder = object
                .get("folder")
                .and_then(Value::as_str)
                .ok_or_else(bad)?;
            if date.len() != 10
                || !date.bytes().enumerate().all(|(n, c)| {
                    if matches!(n, 4 | 7) {
                        c == b'-'
                    } else {
                        c.is_ascii_digit()
                    }
                })
                || folder.is_empty()
                || folder.chars().any(char::is_control)
                || id.is_empty()
                || id.chars().any(char::is_control)
            {
                return Err(bad());
            }
            records.insert(id.clone(), (date.into(), folder.into()));
        }
        Ok(Self { records })
    }
    pub fn save(&self, home: &Path, slug: &str) -> Result<()> {
        let entries = self
            .records
            .iter()
            .map(|(id, (date, folder))| (id.clone(), json!({"agreed_on":date,"folder":folder})))
            .collect::<serde_json::Map<_, _>>();
        private::write_private_at(
            home,
            slug,
            Some("anonymizer"),
            "skills.json",
            &serde_json::to_vec(&json!({"version":1,"skills":entries})).expect("JSON values"),
        )?;
        Ok(())
    }
    pub fn agree(&mut self, skill: &Skill, date: &str) {
        self.records
            .insert(skill.id.clone(), (date.into(), skill.shown.clone()));
    }
    pub fn withdraw(&mut self, id: &str) {
        self.records.remove(id);
    }
    pub fn state(&self, skill: &Skill, rules: &Rules) -> State {
        if rules.never_copy(&skill.name) {
            return State::NeverCopied;
        }
        match self.records.get(&skill.id) {
            Some((date, folder)) if folder == &skill.shown => State::Agreed(date.clone()),
            Some(_) => State::Moved,
            None if skill.root == Root::Synced => State::NotOffered,
            None => State::Waiting,
        }
    }
}
