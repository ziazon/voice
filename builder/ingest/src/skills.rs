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
pub use mirror::{
    MIRROR_REMOVED, MIRROR_STALE, Manifest, Refreshed, Removal, drop_manifest, mirror_in_use,
    refresh, remove_after_withdraw,
};
mod output;
mod read;
pub use output::{
    check_output, finish_output, prepare_output, prepare_output_with_projects, remove_copy,
    write_manifest, write_skill,
};
pub use read::{read, read_with_rules};

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
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
    pub protected: Vec<PathBuf>,
    pub notes: Vec<String>,
}
#[derive(Debug)]
pub enum Source {
    Bytes(Vec<u8>),
    Link,
    HardLink,
    TooLarge,
    Unreadable,
}
pub const SKILL_TOO_LARGE: &str =
    "it has more than 500 files, more than 20 MB or more than 8 folders inside each other";
pub const SKILL_UNREADABLE: &str = "a folder in it couldn't be read";
pub const UNREADABLE_NOTE: &str =
    "A skill folder on this computer couldn't be read, so it isn't listed.";
pub const SYNCED_TWICE_NOTE: &str = "Some claude.ai skills are in more than one synced folder; each is listed once, from the first folder by name.";
pub const PLUGIN_NOTE: &str =
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
fn children(path: &Path, notes: &mut Vec<String>) -> Result<Vec<(String, PathBuf)>> {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            if !notes.iter().any(|note| note == UNREADABLE_NOTE) {
                notes.push(UNREADABLE_NOTE.into());
            }
            return Ok(vec![]);
        }
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
fn protect(protected: &mut Vec<PathBuf>, path: &Path, notes: &mut Vec<String>) -> Result<()> {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            if !notes.iter().any(|note| note == UNREADABLE_NOTE) {
                notes.push(UNREADABLE_NOTE.into());
            }
            return Ok(());
        }
        Err(e) => return Err(io(path, e)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| io(path, e))?;
        if let Ok(folder) = entry.path().canonicalize() {
            protected.push(folder);
        }
    }
    Ok(())
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
            let note = UNREADABLE_NOTE.to_owned();
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
    details: &mut Found,
    home: &Path,
    root: Root,
    prefix: &str,
    path: &Path,
) -> Result<()> {
    protect(&mut details.protected, path, &mut details.notes)?;
    for (name, path) in children(path, &mut details.notes)? {
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
        add(found, &mut details.notes, home, root, prefix, &name, &path);
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
        .is_ok_and(|m| m.is_file() && m.len() <= 4 << 20 && m.nlink() <= 1)
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
/// The existing skill roots and linked folders discovery needs to read.
pub fn sources(home: &Path, rules: &Rules) -> Vec<(PathBuf, bool)> {
    fn keep(
        paths: &mut Vec<(PathBuf, bool)>,
        canonical: &mut Vec<PathBuf>,
        path: PathBuf,
        folder: bool,
    ) -> bool {
        if fs::metadata(&path).is_ok_and(|m| if folder { m.is_dir() } else { m.is_file() }) {
            if !paths.iter().any(|(kept, _)| kept == &path) {
                if let Ok(real) = path.canonicalize() {
                    canonical.push(real);
                }
                paths.push((path, folder));
            }
            return true;
        }
        false
    }
    fn holds_skill(path: &Path, depth: u8) -> bool {
        if depth == 0 {
            fs::symlink_metadata(path.join("SKILL.md")).is_ok_and(|m| m.is_file())
        } else {
            children(path, &mut vec![]).is_ok_and(|entries| {
                entries
                    .iter()
                    .any(|(_, child)| holds_skill(child, depth - 1))
            })
        }
    }
    fn linked(
        paths: &mut Vec<(PathBuf, bool)>,
        canonical: &mut Vec<PathBuf>,
        path: &Path,
        skill_depth: u8,
        install: Option<&Path>,
        deny: &DenyList,
    ) {
        if !fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return;
        }
        let Ok(target) = path.canonicalize() else {
            return;
        };
        let has_skill = holds_skill(&target, skill_depth);
        if install.is_some_and(|install| !target.starts_with(install))
            || !has_skill
            || deny.denied(&target)
            || canonical.iter().any(|kept| target.starts_with(kept))
        {
            return;
        }
        if target.is_dir() {
            paths.push((target.clone(), true));
            canonical.push(target);
        }
    }
    let deny = DenyList::builtin();
    let mut paths = vec![];
    let mut canonical = vec![];
    let mut roots = vec![];
    let claude = home.join(".claude/skills");
    if keep(&mut paths, &mut canonical, claude.clone(), true) {
        roots.push(claude);
    }
    keep(
        &mut paths,
        &mut canonical,
        home.join(".claude/plugins/installed_plugins.json"),
        false,
    );
    if let Ok(plugins) = plugin_roots(home) {
        for (_, install) in plugins {
            let root = install.join("skills");
            if keep(&mut paths, &mut canonical, root.clone(), true)
                && let Ok(entries) = children(&root, &mut vec![])
            {
                for (_, child) in entries {
                    linked(&mut paths, &mut canonical, &child, 0, Some(&install), &deny);
                }
            }
        }
    }
    for project in rules.projects() {
        if !deny.denied(project) {
            let root = project.join(".claude/skills");
            if keep(&mut paths, &mut canonical, root.clone(), true) {
                roots.push(root);
            }
        }
    }
    let codex = home.join(".codex/skills");
    if keep(&mut paths, &mut canonical, codex.clone(), true) {
        roots.push(codex);
    }
    let mut notes = vec![];
    for root in roots {
        if let Ok(entries) = children(&root, &mut notes) {
            for (name, child) in entries {
                // A synced root holds accounts; each account holds skills.
                let skill_depth = if root == home.join(".claude/skills") && name == "synced" {
                    2
                } else {
                    0
                };
                linked(&mut paths, &mut canonical, &child, skill_depth, None, &deny);
            }
        }
    }
    if let Ok(accounts) = children(&home.join(".claude/skills/synced"), &mut notes) {
        for (_, account) in accounts {
            if !account.is_dir() || deny.denied(&account) {
                continue;
            }
            linked(&mut paths, &mut canonical, &account, 1, None, &deny);
            if let Ok(entries) = children(&account, &mut notes) {
                for (_, child) in entries {
                    linked(&mut paths, &mut canonical, &child, 0, None, &deny);
                }
            }
        }
    }
    paths
}
pub fn discover(home: &Path, rules: &Rules) -> Result<Found> {
    let mut skills = BTreeMap::new();
    let mut found = Found::default();
    root_skills(
        &mut skills,
        &mut found,
        home,
        Root::Claude,
        "claude",
        &home.join(".claude/skills"),
    )?;
    protect(
        &mut found.protected,
        &home.join(".claude/skills/synced"),
        &mut found.notes,
    )?;
    for (_, account) in children(&home.join(".claude/skills/synced"), &mut found.notes)? {
        if !account.is_dir() || DenyList::builtin().denied(&account) {
            continue;
        }
        protect(&mut found.protected, &account, &mut found.notes)?;
        for (name, path) in children(&account, &mut found.notes)? {
            let id = format!("synced:{name}");
            let existed = skills.contains_key(&id);
            add(
                &mut skills,
                &mut found.notes,
                home,
                Root::Synced,
                "synced",
                &name,
                &path,
            );
            if existed && path.join("SKILL.md").is_file() && !DenyList::builtin().denied(&path) {
                let note = SYNCED_TWICE_NOTE.to_owned();
                if !found.notes.contains(&note) {
                    found.notes.push(note);
                }
            }
        }
    }
    match plugin_roots(home) {
        Ok(roots) => {
            for (name, path) in roots {
                root_skills(
                    &mut skills,
                    &mut found,
                    home,
                    Root::Plugin,
                    &format!("plugin:{name}"),
                    &path.join("skills"),
                )?
            }
        }
        Err(_) => found.notes.push(PLUGIN_NOTE.into()),
    }
    for project in rules.projects() {
        protect(
            &mut found.protected,
            &project.join(".claude/skills"),
            &mut found.notes,
        )?;
        let Some(name) = project.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if DenyList::builtin().denied(project) {
            continue;
        }
        root_skills(
            &mut skills,
            &mut found,
            home,
            Root::Project,
            &format!("project:{name}"),
            &project.join(".claude/skills"),
        )?;
    }
    root_skills(
        &mut skills,
        &mut found,
        home,
        Root::Codex,
        "codex",
        &home.join(".codex/skills"),
    )?;
    found.skills = skills.into_values().collect();
    found.protected.sort();
    found.protected.dedup();
    Ok(found)
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
    /// Whether no skills have been agreed to.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
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
    pub fn withdraw(&mut self, id: &str) -> bool {
        self.records.remove(id).is_some()
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
