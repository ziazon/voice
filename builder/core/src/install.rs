use crate::{
    block,
    crew::{self, MemberDir},
    install_fs::{ClaudeFile, Journal, check_temporary, exists, io},
    install_record::{self, Added, Record},
    member::refuse_symlink,
};
use std::fs;

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub struct Places {
    pub home: PathBuf,
    pub claude_home: PathBuf,
}
#[derive(Debug)]
pub struct TurnedOn {
    pub name: String,
    pub slug: String,
    pub skill: PathBuf,
    pub claude_md: PathBuf,
    pub import_line: String,
}
#[derive(Debug)]
pub struct TurnedOff {
    pub name: Option<String>,
    pub left: Vec<Left>,
}
#[derive(Debug)]
pub struct Left {
    pub path: PathBuf,
    pub why: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum FileState {
    Present,
    Changed,
    Missing,
}
#[derive(Debug)]
pub struct Status {
    pub name: String,
    pub slug: String,
    pub member_dir: PathBuf,
    pub line: bool,
    pub files: Vec<(PathBuf, FileState)>,
    pub out_of_date: bool,
}

const RULES: &str = "# {{NAME}}\n\nBefore you ask me a question (a choice between options, a go or no-go, a plan's open questions),\nask {{NAME}} first with the /{{SLUG}} skill. {{NAME}} says what I would likely pick and how sure\nthat answer is. Ask me only what {{NAME}} hands back, and show me {{NAME}}'s answer beside the\nquestion.\n";

impl Places {
    fn canonical(&self) -> Result<Self> {
        if !self.claude_home.is_dir() {
            return Err(Error::Invalid(format!(
                "{} is missing: install Claude Code and sign in to it first",
                self.claude_home.display()
            )));
        }
        Ok(Self {
            home: io(&self.home, fs::canonicalize(&self.home))?,
            claude_home: io(&self.claude_home, fs::canonicalize(&self.claude_home))?,
        })
    }
}

fn block_error(path: &Path, error: Error) -> Error {
    Error::Invalid(format!("{}: {error}", path.display()))
}

pub fn turn_on(member_dir: &Path, places: &Places) -> Result<TurnedOn> {
    let opened = MemberDir::open(member_dir)?;
    let member = opened.member;
    crew::check_name("name", &member.name)?;
    crew::check_name("principal", &member.principal)?;
    block::validate_id(&member.slug)?;
    let places = places.canonical()?;
    let member_dir = io(member_dir, fs::canonicalize(member_dir))?;
    let source = member_dir.join("SKILL.md");
    refuse_symlink(&source)?;
    let skill_text = io(&source, fs::read_to_string(&source))?;
    check_skill(&source, &skill_text, &member.slug, &member.name)?;
    let mut journal = Journal::default();
    if let Some(record) = install_record::read(&places.claude_home)? {
        if record.slug != member.slug {
            return Err(Error::Invalid(format!(
                "{0} is on. Turn {0} off first: persona-builder off",
                record.name
            )));
        }
        // Refresh must be able to write its new record before removing the old one.
        check_temporary(&install_record::record_path(&places.claude_home), true)?;
        let turned_off = off(&places, record, &mut journal)?;
        if let Some(left) = turned_off.left.first() {
            return Err(Error::Invalid(format!(
                "{} was left in place ({}). Move it away, then turn {} on again",
                left.path.display(),
                left.why,
                member.name
            )));
        }
    }
    let skill = places.claude_home.join("skills").join(&member.slug);
    let rules_folder = places
        .claude_home
        .join("persona-builder")
        .join(&member.slug);
    for path in [&skill, &rules_folder] {
        install_record::check_path(path, &places.claude_home)?;
        refuse_symlink(path)?;
        if exists(path)? {
            return Err(Error::Invalid(format!(
                "{} exists and Persona Builder did not add it. Move it away, then turn {} on again",
                path.display(),
                member.name
            )));
        }
    }
    let claude_md = places.claude_home.join("CLAUDE.md");
    let claude = ClaudeFile::read(&claude_md)?;
    if block::find(&claude.text, &member.slug)
        .map_err(|error| block_error(&claude_md, error))?
        .is_some()
    {
        return Err(Error::Invalid(format!(
            "{} already has a Persona Builder line for {}. Remove the lines between its persona-builder markers, then turn {} on again",
            claude_md.display(),
            member.slug,
            member.name
        )));
    }
    let rules_path = rules_folder.join("RULES.md");
    let shown = match rules_path.strip_prefix(&places.home) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => rules_path.display().to_string(),
    };
    let import_line = format!("@{}", shown.replace(' ', "\\ "));
    let updated = block::upsert(&claude.text, &member.slug, &import_line)?;
    let rules = crew::fill(RULES, &[("NAME", &member.name), ("SLUG", &member.slug)]);
    let mut record = Record {
        slug: member.slug.clone(),
        name: member.name.clone(),
        member_dir,
        claude_md_created: !claude.existed,
        added: vec![],
    };
    for (folder, file, text) in [
        (&skill, "SKILL.md", &skill_text),
        (&rules_folder, "RULES.md", &rules),
    ] {
        let parent = folder.parent().expect("installation folder has a parent");
        if journal.directory(parent, parent == places.claude_home.join("skills"))? {
            record.added.push(Added::Dir {
                path: parent.to_owned(),
            });
        }
        if journal.directory(folder, false)? {
            record.added.push(Added::Dir {
                path: folder.to_owned(),
            });
        }
        let path = folder.join(file);
        install_record::check_path(&path, &places.claude_home)?;
        journal.new_file(&path, text)?;
        record.added.push(Added::File {
            path,
            text: text.to_owned(),
        });
    }
    journal.replace(&claude.path, &updated, claude.mode, false)?;
    journal.replace(
        &install_record::record_path(&places.claude_home),
        &crate::json::to_python_json(&record)?,
        0o600,
        true,
    )?;
    journal.commit();
    Ok(TurnedOn {
        name: member.name,
        slug: member.slug,
        skill,
        claude_md,
        import_line,
    })
}

fn check_skill(path: &Path, text: &str, slug: &str, name: &str) -> Result<()> {
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if matches!(character, '\u{0085}' | '\u{2028}' | '\u{2029}')
            || (character.is_control()
                && !matches!(character, '\t' | '\n')
                && !(character == '\r' && characters.peek() == Some(&'\n')))
        {
            return Err(Error::Invalid(format!(
                "{} holds a character Persona Builder can't check (a lone carriage return, a Unicode line break or another control character). Save it as plain text, then turn {name} on again",
                path.display()
            )));
        }
    }
    let lines: Vec<_> = text.lines().collect();
    let end = if lines.first() == Some(&"---") {
        lines
            .iter()
            .skip(1)
            .position(|line| *line == "---")
            .map(|i| i + 1)
    } else {
        None
    };
    let Some(end) = end else {
        return Err(Error::Invalid(format!(
            "{} has no front matter (the lines between ---)",
            path.display()
        )));
    };
    let front_matter = &lines[1..end];
    if !front_matter.contains(&format!("name: {slug}").as_str()) {
        return Err(Error::Invalid(format!(
            "{} must be named {slug} (a line name: {slug})",
            path.display()
        )));
    }
    for line in front_matter {
        if line.starts_with('\t') {
            return Err(Error::Invalid(format!(
                "{} indents a front-matter line with a tab. Use spaces, then turn {name} on again",
                path.display()
            )));
        }
        if line.starts_with(' ') || line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let key = line.split_once(':').map_or(*line, |(key, _)| key).trim();
        let key = key
            .strip_prefix('"')
            .and_then(|key| key.strip_suffix('"'))
            .or_else(|| {
                key.strip_prefix('\'')
                    .and_then(|key| key.strip_suffix('\''))
            })
            .unwrap_or(key);
        if matches!(key, "hooks" | "allowed-tools") {
            return Err(Error::Invalid(format!(
                "{} asks for {key}, which can run commands without asking. Persona Builder turns on only a skill without them",
                path.display()
            )));
        }
        if line.starts_with(['?', '&', '*'])
            || line.starts_with("<<")
            || !line.contains(':')
            || !matches!(
                key,
                "name"
                    | "description"
                    | "argument-hint"
                    | "user-invocable"
                    | "disable-model-invocation"
                    | "model"
                    | "context"
                    | "agent"
                    | "paths"
            )
        {
            return Err(Error::Invalid(format!(
                "{} asks for {key}, which Persona Builder does not turn on. Remove it, then turn {name} on again",
                path.display()
            )));
        }
    }
    if text.contains("!`") {
        return Err(Error::Invalid(format!(
            "{} runs a shell command (!`). Persona Builder turns on only a skill without one",
            path.display()
        )));
    }
    Ok(())
}

pub fn turn_off(places: &Places) -> Result<TurnedOff> {
    let places = places.canonical()?;
    let Some(record) = install_record::read(&places.claude_home)? else {
        return Ok(TurnedOff {
            name: None,
            left: vec![],
        });
    };
    let mut journal = Journal::default();
    let result = off(&places, record, &mut journal)?;
    journal.commit();
    Ok(result)
}

fn off(places: &Places, record: Record, journal: &mut Journal) -> Result<TurnedOff> {
    let claude_md = places.claude_home.join("CLAUDE.md");
    let claude = ClaudeFile::read(&claude_md)?;
    let updated = block::remove(&claude.text, &record.slug)
        .map_err(|error| block_error(&claude_md, error))?;
    if claude.existed {
        if record.claude_md_created && updated.is_empty() && !claude.symlink {
            journal.remove_file(&claude.path, true)?;
        } else if updated != claude.text {
            journal.replace(&claude.path, &updated, claude.mode, false)?;
        }
    }
    let mut left = vec![];
    for added in record.added.iter().rev() {
        if let Added::File { path, text } = added {
            if !exists(path)? {
                continue;
            }
            let why = if fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink()) {
                Some("it is a symlink")
            } else if io(path, fs::read(path))? == text.as_bytes() {
                journal.remove_file(path, false)?;
                None
            } else {
                Some("you changed it after it was turned on")
            };
            if let Some(why) = why {
                left.push(Left {
                    path: path.clone(),
                    why: why.into(),
                });
            }
        }
    }
    journal.remove_file(&install_record::record_path(&places.claude_home), false)?;
    for added in record.added.iter().rev() {
        if let Added::Dir { path } = added {
            if !exists(path)? {
                continue;
            }
            let why = if fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink()) {
                Some("it is a symlink")
            } else if journal.remove_dir(path)? {
                None
            } else {
                Some("it still holds other files")
            };
            if let Some(why) = why {
                left.push(Left {
                    path: path.clone(),
                    why: why.into(),
                });
            }
        }
    }
    Ok(TurnedOff {
        name: Some(record.name),
        left,
    })
}

fn file_state(path: &Path, text: &str) -> Result<FileState> {
    if !exists(path)? {
        return Ok(FileState::Missing);
    }
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink()) {
        return Ok(FileState::Changed);
    }
    Ok(if io(path, fs::read(path))? == text.as_bytes() {
        FileState::Present
    } else {
        FileState::Changed
    })
}

pub fn status(places: &Places) -> Result<Option<Status>> {
    let places = places.canonical()?;
    let Some(record) = install_record::read(&places.claude_home)? else {
        return Ok(None);
    };
    let claude_md = places.claude_home.join("CLAUDE.md");
    let claude = ClaudeFile::read(&claude_md)?;
    let line = block::find(&claude.text, &record.slug)
        .map_err(|error| block_error(&claude_md, error))?
        .is_some();
    let mut files = vec![];
    let mut out_of_date = false;
    for added in &record.added {
        if let Added::File { path, text } = added {
            files.push((path.clone(), file_state(path, text)?));
            if path
                == &places
                    .claude_home
                    .join("skills")
                    .join(&record.slug)
                    .join("SKILL.md")
            {
                out_of_date =
                    file_state(&record.member_dir.join("SKILL.md"), text)? != FileState::Present;
            }
        }
    }
    Ok(Some(Status {
        name: record.name,
        slug: record.slug,
        member_dir: record.member_dir,
        line,
        files,
        out_of_date,
    }))
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
