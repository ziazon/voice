//! Skills on this computer and the assistant's private copy.
#[cfg(not(target_os = "macos"))]
use crate::assistants::home;
use crate::{
    assistants::{failure, listed},
    dto,
    history::{BUSY, count},
};
use std::{path::Path, time::SystemTime};
use voice_core::{error::Error, member::Member};
use voice_ingest::{
    anonymize::Rules,
    private,
    skills::{self, Agreements},
};
type Result<T> = std::result::Result<T, dto::Failure>;
pub(crate) const SKILLS_READ: &str =
    "Voice couldn't look through the skills on this computer. Close it and open it again.";
pub(crate) const SKILLS_SAVE: &str =
    "Voice couldn't save your choice. Check there's space on this computer, then try again.";
pub(crate) const GONE: &str = "That skill isn't on this computer any more. Look again.";
pub(crate) const NONE: &str = "Choose a skill first.";
pub(crate) const REFRESH: &str = "Voice couldn't bring your assistant's copy of your skills up to date, and it may be out of date. To see why, run voice skills refresh in a terminal.";
pub(crate) const REFRESH_STALE: &str = "Voice couldn't bring your assistant's copy of your skills up to date, so it marked the copy as out of date and left its files alone. To see what needs fixing, run voice skills refresh in a terminal.";
pub(crate) const REFRESH_REMOVED: &str = "Voice couldn't bring your assistant's copy of your skills up to date, so it removed the copy. To see what needs fixing, run voice skills refresh in a terminal.";

fn locked(home: &Path, slug: &str) -> Result<(Member, private::StateLock)> {
    let listed = listed(home, slug)?;
    let lock = private::lock_state(home, &listed.member.slug).map_err(|error| match error {
        Error::Invalid(text) if text == private::BUSY => failure(BUSY),
        _ => failure(SKILLS_READ),
    })?;
    Ok((listed.member, lock))
}
pub(crate) fn checked(mut result: dto::Skills) -> dto::Skills {
    result.notes.retain(|note| {
        matches!(
            note.as_str(),
            skills::UNREADABLE_NOTE | skills::SYNCED_TWICE_NOTE | skills::PLUGIN_NOTE
        )
    });
    result
}
fn list(home: &Path, member: &Member) -> Result<dto::Skills> {
    let rules = Rules::for_member(member, home).map_err(|_| failure(SKILLS_READ))?;
    let found = skills::discover(home, &rules).map_err(|_| failure(SKILLS_READ))?;
    let agreements = Agreements::load(home, &member.slug).map_err(|_| failure(SKILLS_READ))?;
    let mut rows: Vec<_> = found
        .skills
        .into_iter()
        .enumerate()
        .map(|(index, skill)| {
            let (state, agreed_on) = match agreements.state(&skill, &rules) {
                skills::State::Waiting => (dto::SkillState::Waiting, None),
                skills::State::Agreed(date) => (dto::SkillState::Agreed, Some(date)),
                skills::State::Moved => (dto::SkillState::Moved, None),
                skills::State::NotOffered => (dto::SkillState::NotOffered, None),
                skills::State::NeverCopied => (dto::SkillState::NeverCopied, None),
            };
            dto::SkillRow {
                id: skill.id,
                name: skill.name,
                place: skill.shown,
                state,
                agreed_on,
                number: count(index.saturating_add(1)),
            }
        })
        .collect();
    rows.sort_by_key(|row| row.state != dto::SkillState::Waiting);
    Ok(checked(dto::Skills {
        skills: rows,
        notes: found.notes,
    }))
}

pub fn list_skills_in(home: &Path, request: dto::SlugRequest) -> Result<dto::Skills> {
    let (member, _lock) = locked(home, &request.slug)?;
    list(home, &member)
}
pub fn agree_skills_in(home: &Path, request: dto::SkillsRequest) -> Result<dto::Skills> {
    let (member, _lock) = locked(home, &request.slug)?;
    if request.ids.is_empty() {
        return Err(failure(NONE));
    }
    let rules = Rules::for_member(&member, home).map_err(|_| failure(SKILLS_READ))?;
    let found = skills::discover(home, &rules).map_err(|_| failure(SKILLS_READ))?;
    let mut agreements = Agreements::load(home, &member.slug).map_err(|_| failure(SKILLS_READ))?;
    let chosen = request
        .ids
        .iter()
        .map(|id| {
            found
                .skills
                .iter()
                .find(|skill| &skill.id == id)
                .ok_or_else(|| failure(GONE))
        })
        .collect::<Result<Vec<_>>>()?;
    let date = voice_agent::ledger::local_day(SystemTime::now());
    for skill in chosen {
        agreements.agree(skill, &date);
    }
    agreements
        .save(home, &member.slug)
        .map_err(|_| failure(SKILLS_SAVE))?;
    list(home, &member)
}
pub fn withdraw_skills_in(
    home: &Path,
    request: dto::SkillsRequest,
) -> Result<dto::SkillsWithdrawn> {
    let (member, _lock) = locked(home, &request.slug)?;
    if request.ids.is_empty() {
        return Err(failure(NONE));
    }
    let rules = Rules::for_member(&member, home).map_err(|_| failure(SKILLS_READ))?;
    let found = skills::discover(home, &rules).map_err(|_| failure(SKILLS_READ))?;
    let mut agreements = Agreements::load(home, &member.slug).map_err(|_| failure(SKILLS_READ))?;
    let mut withdrew = false;
    for id in &request.ids {
        withdrew |= agreements.withdraw(id);
    }
    agreements
        .save(home, &member.slug)
        .map_err(|_| failure(SKILLS_SAVE))?;
    let mirror = if withdrew {
        match skills::remove_after_withdraw(home, &member.slug, &found, &rules) {
            skills::Removal::NotThere => dto::MirrorAfter::NotThere,
            skills::Removal::Removed => dto::MirrorAfter::Removed,
            skills::Removal::NotRemoved => dto::MirrorAfter::NotRemoved,
        }
    } else {
        dto::MirrorAfter::NotThere
    };
    Ok(dto::SkillsWithdrawn {
        skills: list(home, &member)?,
        mirror,
    })
}
pub fn refresh_skills_in(home: &Path, request: dto::SlugRequest) -> Result<dto::SkillsRefreshed> {
    let (member, _lock) = locked(home, &request.slug)?;
    refresh(home, &member)
}
/// For a running panel: take the state lock if free, or trust the caller holds it
/// when it is busy. Anyone else uses `refresh_skills_in`.
pub fn refresh_held_in(home: &Path, request: dto::SlugRequest) -> Result<dto::SkillsRefreshed> {
    let member = listed(home, &request.slug)?.member;
    let _lock = match private::lock_state(home, &member.slug) {
        Ok(lock) => Some(lock),
        Err(Error::Invalid(text)) if text == private::BUSY => None,
        Err(_) => return Err(failure(SKILLS_READ)),
    };
    refresh(home, &member)
}
fn refresh(home: &Path, member: &Member) -> Result<dto::SkillsRefreshed> {
    let date = voice_agent::ledger::local_day(SystemTime::now());
    let refreshed = skills::refresh(home, member, &date).map_err(|error| match error {
        Error::Invalid(text) if text.ends_with(skills::MIRROR_STALE) => failure(REFRESH_STALE),
        Error::Invalid(text) if text.ends_with(skills::MIRROR_REMOVED) => failure(REFRESH_REMOVED),
        _ => failure(REFRESH),
    })?;
    let copied = refreshed.copied;
    Ok(dto::SkillsRefreshed {
        lines: copied
            .lines
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
        copied: count(copied.copied),
        agreed: count(copied.agreed),
        waiting: count(copied.waiting),
        new: count(refreshed.new),
        updated: count(refreshed.updated),
        unchanged: count(refreshed.unchanged),
        removed: count(refreshed.removed),
    })
}

pub fn list_skills(request: dto::SlugRequest) -> Result<dto::Skills> {
    #[cfg(target_os = "macos")]
    {
        crate::history::with_reader(|home, place| {
            crate::reader_client::list_skills(home, place, request)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        list_skills_in(&home(SKILLS_READ)?, request)
    }
}
pub fn agree_skills(request: dto::SkillsRequest) -> Result<dto::Skills> {
    #[cfg(target_os = "macos")]
    {
        crate::history::with_reader(|home, place| {
            crate::reader_client::agree_skills(home, place, request)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        agree_skills_in(&home(SKILLS_READ)?, request)
    }
}
pub fn withdraw_skills(request: dto::SkillsRequest) -> Result<dto::SkillsWithdrawn> {
    #[cfg(target_os = "macos")]
    {
        crate::history::with_reader(|home, place| {
            crate::reader_client::withdraw_skills(home, place, request)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        withdraw_skills_in(&home(SKILLS_READ)?, request)
    }
}
pub fn refresh_skills(request: dto::SlugRequest) -> Result<dto::SkillsRefreshed> {
    #[cfg(target_os = "macos")]
    {
        crate::history::with_reader(|home, place| {
            crate::reader_client::refresh_skills(home, place, request)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        refresh_skills_in(&home(SKILLS_READ)?, request)
    }
}
