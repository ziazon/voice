//! Back up and remove an older skill and only its matching instructions before turn-on.
use crate::{
    backup::{self, Item},
    block,
    error::{Error, Result},
    existing, install,
    install_fs::{ClaudeFile, Journal, exists, io},
};
use std::{fs, path::PathBuf};

pub const SKILL_NOT_FOLDER: &str = "The older skill isn't a plain folder, so Voice left it. Move it away yourself, then turn the assistant on.";
pub const CHANGED: &str = backup::CHANGED;
pub const NO_OLDER_START: &str = "Voice found no older ";
pub const NO_OLDER_END: &str = " to replace.";

pub const ROLLED_BACK_FAILED_START: &str = "Voice couldn't put the older ";
pub const ROLLED_BACK_FAILED_END: &str = " skill back.";

pub fn rolled_back_failed(slug: &str) -> String {
    format!(
        "{ROLLED_BACK_FAILED_START}{slug}{ROLLED_BACK_FAILED_END} It's at ~/.claude/skills/.{slug}.voice-replacing-<stamp>, and a copy is in ~/.claude/voice-backups/{slug}/<stamp>/."
    )
}

pub fn says(message: &str) -> bool {
    message
        .strip_prefix(ROLLED_BACK_FAILED_START)
        .and_then(|rest| rest.split_once(ROLLED_BACK_FAILED_END))
        .is_some_and(|(slug, _)| {
            block::validate_id(slug).is_ok() && message == rolled_back_failed(slug)
        })
        || matches!(message, SKILL_NOT_FOLDER | CHANGED)
        || message
            .strip_prefix(NO_OLDER_START)
            .and_then(|rest| rest.strip_suffix(NO_OLDER_END))
            .is_some_and(|slug| block::validate_id(slug).is_ok())
}

pub fn old_skill(name: &str, slug: &str) -> String {
    format!(
        "Claude Code already has a skill called {slug} that Voice didn't add, so Voice didn't turn {name} on. Choose Replace the old one, and Voice backs it up and moves it aside first."
    )
}

#[derive(Debug)]
pub struct OldOne {
    pub skill: Option<PathBuf>,
    pub claude_md: PathBuf,
    pub lines: Vec<(usize, String)>,
}
impl OldOne {
    pub fn any(&self) -> bool {
        self.skill.is_some() || !self.lines.is_empty()
    }
}
#[derive(Debug)]
pub struct Replaced {
    pub backup: PathBuf,
    pub skill: bool,
    pub lines: usize,
    pub leftover: Option<PathBuf>,
}

pub fn find(places: &install::Places, slug: &str) -> Result<OldOne> {
    block::validate_id(slug)?;
    let places = places.canonical()?;
    let skill_path = places.claude_home.join("skills").join(slug);
    let skill = (exists(&skill_path)?
        && !existing::skill_claimed(&places.claude_home, slug, &skill_path))
    .then_some(skill_path);
    let claude_md = places.claude_home.join("CLAUDE.md");
    let lines = existing::old_lines(&claude_md, slug)?;
    Ok(OldOne {
        skill,
        claude_md,
        lines,
    })
}

pub fn replace(places: &install::Places, slug: &str, stamp: &str) -> Result<Replaced> {
    replace_checked(
        places,
        slug,
        stamp,
        #[cfg(test)]
        &|| {},
        &|| {},
    )
}

fn replace_checked(
    places: &install::Places,
    slug: &str,
    stamp: &str,
    #[cfg(test)] before_backup: &dyn Fn(),
    after_copy: &dyn Fn(),
) -> Result<Replaced> {
    let found = find(places, slug)?;
    if !found.any() {
        return Err(Error::Invalid(format!(
            "{NO_OLDER_START}{slug}{NO_OLDER_END}"
        )));
    }
    let places = places.canonical()?;
    let aside = found
        .skill
        .as_ref()
        .map(|path| path.with_file_name(format!(".{slug}.voice-replacing-{stamp}")));
    if let Some(skill) = &found.skill {
        if !io(skill, fs::symlink_metadata(skill))?.is_dir() {
            return Err(Error::Invalid(SKILL_NOT_FOLDER.into()));
        }
        if exists(aside.as_ref().expect("skill has an aside path"))? {
            return Err(Error::Invalid(CHANGED.into()));
        }
    }
    if !found.lines.is_empty() {
        ClaudeFile::read(&found.claude_md)?;
    }
    let mut items = Vec::new();
    if let Some(skill) = &found.skill {
        items.push(Item {
            name: "skill",
            source: skill,
            follow: false,
        });
    }
    if !found.lines.is_empty() {
        items.push(Item {
            name: "CLAUDE.md",
            source: &found.claude_md,
            follow: true,
        });
    }
    #[cfg(test)]
    before_backup();
    let backup_parent = places.home.join(".claude/voice-backups").join(slug);
    let new_parent = !exists(&backup_parent)?;
    let clean_parent = || {
        if new_parent {
            // remove_dir only removes an empty folder; keep all existing backups.
            let _ = fs::remove_dir(&backup_parent);
        }
    };
    let backup =
        backup::back_up(&places.home, slug, stamp, &items).inspect_err(|_| clean_parent())?;
    let mut moved = false;
    let result = (|| {
        after_copy();
        if let (Some(skill), Some(aside)) = (&found.skill, &aside) {
            io(skill, fs::rename(skill, aside))?;
            moved = true;
            backup::verify_entry(aside, &backup.join("skill"))?;
        }
        let mut journal = Journal::default();
        if !found.lines.is_empty() {
            let claude = ClaudeFile::read(&found.claude_md)?;
            let copied = backup.join("CLAUDE.md");
            if claude.text.as_bytes() != io(&copied, fs::read(&copied))? {
                return Err(Error::Invalid(CHANGED.into()));
            }
            let lines: Vec<_> = claude.text.split_inclusive('\n').collect();
            for (number, expected) in &found.lines {
                let actual = lines.get(number - 1).map(|line| {
                    line.strip_suffix('\n')
                        .map(|line| line.strip_suffix('\r').unwrap_or(line))
                        .unwrap_or(line)
                });
                if actual != Some(expected.as_str()) {
                    return Err(Error::Invalid(CHANGED.into()));
                }
            }
            let text: String = claude
                .text
                .split_inclusive('\n')
                .enumerate()
                .filter(|(index, _)| {
                    found
                        .lines
                        .binary_search_by_key(&(index + 1), |(number, _)| *number)
                        .is_err()
                })
                .map(|(_, line)| line)
                .collect();
            journal.replace(&claude.path, &text, claude.mode, false)?;
        }
        journal.commit();
        Ok(())
    })();
    if let Err(error) = result {
        if moved {
            let skill = found.skill.as_ref().expect("moved skill exists");
            if fs::rename(aside.as_ref().expect("moved skill has aside"), skill).is_err() {
                return Err(Error::Invalid(rolled_back_failed(slug)));
            }
        }
        io(&backup, fs::remove_dir_all(&backup))?;
        clean_parent();
        return Err(error);
    }
    // From here on the verified backup is retained even if removing the aside fails.
    let leftover = aside.filter(|path| fs::remove_dir_all(path).is_err());
    Ok(Replaced {
        backup,
        skill: moved,
        lines: found.lines.len(),
        leftover,
    })
}

#[cfg(test)]
#[path = "replace_old_tests.rs"]
mod tests;
