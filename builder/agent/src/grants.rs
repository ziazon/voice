//! Dated read permissions and their content-free, append-only audit log.
use crate::{ledger, text};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::SystemTime};
use voice_core::{
    ceilings::Source,
    error::{Error, Result},
};
use voice_ingest::private::{
    append_private, read_private_owned_optional_at_max, read_private_tail_optional, remove_private,
    write_private,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    CalendarRead,
}
impl Capability {
    pub const ALL: [Self; 1] = [Self::CalendarRead];
    pub fn id(self) -> &'static str {
        match self {
            Self::CalendarRead => "calendar.read",
        }
    }
    pub fn source(self) -> Source {
        match self {
            Self::CalendarRead => Source::Calendar,
        }
    }
    pub fn words(self) -> &'static str {
        match self {
            Self::CalendarRead => "read your calendar",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.id() == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    past_days: u16,
    next_days: u16,
}
impl Window {
    pub const DEFAULT: Window = Window {
        past_days: 0,
        next_days: 14,
    };
    pub fn new(past_days: u16, next_days: u16) -> Result<Window> {
        if past_days > 31 || !(1..=92).contains(&next_days) {
            return Err(Error::Invalid(text::WINDOW_RANGE.into()));
        }
        Ok(Window {
            past_days,
            next_days,
        })
    }
    pub fn past_days(self) -> u16 {
        self.past_days
    }
    pub fn next_days(self) -> u16 {
        self.next_days
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub capability: Capability,
    pub given_on: String,
    pub window: Window,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u8,
    grants: Vec<SavedGrant>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedGrant {
    capability: String,
    given_on: String,
    past_days: u16,
    next_days: u16,
}

fn day(day: &str) -> bool {
    let b = day.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
        && ("01"..="12").contains(&&day[5..7])
        && ("01"..="31").contains(&&day[8..10])
}
fn stamp(at: &str) -> bool {
    let b = at.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            13 | 16 => *b == b':',
            19 => *b == b'Z',
            _ => b.is_ascii_digit(),
        })
        && day(&at[..10])
        && &at[11..13] <= "23"
        && &at[14..16] <= "59"
        && &at[17..19] <= "59"
}
fn valid_step(step: &str) -> bool {
    (1..=32).contains(&step.len())
        && step.as_bytes()[0].is_ascii_lowercase()
        && step
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn check_step(step: &str) -> Result<()> {
    if valid_step(step) {
        Ok(())
    } else {
        Err(Error::Invalid(text::AUDIT_STEP.into()))
    }
}
fn loaded(home: &Path, slug: &str) -> Result<(Vec<Grant>, Option<Vec<u8>>)> {
    let path = home.join(".claude/state").join(slug).join("grants.json");
    let damaged = || Error::Invalid(text::grants_damaged(&path));
    let bytes = read_private_owned_optional_at_max(home, slug, "grants.json", 64 * 1024)
        .map_err(|_| damaged())?;
    let Some(raw) = &bytes else {
        return Ok((Vec::new(), None));
    };
    let saved: Saved = serde_json::from_slice(raw).map_err(|_| damaged())?;
    if saved.version != 1 {
        return Err(damaged());
    }
    let mut grants = Vec::new();
    for grant in saved.grants {
        let capability = Capability::from_id(&grant.capability).ok_or_else(damaged)?;
        if !day(&grant.given_on) || grants.iter().any(|g: &Grant| g.capability == capability) {
            return Err(damaged());
        }
        grants.push(Grant {
            capability,
            given_on: grant.given_on,
            window: Window::new(grant.past_days, grant.next_days).map_err(|_| damaged())?,
        });
    }
    grants.sort_by_key(|grant| grant.capability);
    Ok((grants, bytes))
}
/// Read without creating state. A damaged file always refuses permission.
pub fn load(home: &Path, slug: &str) -> Result<Vec<Grant>> {
    loaded(home, slug).map(|(grants, _)| grants)
}
fn save(home: &Path, slug: &str, grants: &[Grant]) -> Result<()> {
    if grants.is_empty() {
        remove_private(home, slug, "grants.json")?;
    } else {
        let saved = Saved {
            version: 1,
            grants: Capability::ALL
                .into_iter()
                .filter_map(|cap| grants.iter().find(|g| g.capability == cap))
                .map(|g| SavedGrant {
                    capability: g.capability.id().into(),
                    given_on: g.given_on.clone(),
                    past_days: g.window.past_days,
                    next_days: g.window.next_days,
                })
                .collect(),
        };
        write_private(
            home,
            slug,
            "grants.json",
            &serde_json::to_vec(&saved).expect("grants serialize"),
        )?;
    }
    Ok(())
}
fn restore(home: &Path, slug: &str, bytes: Option<&[u8]>) -> Result<()> {
    match bytes {
        Some(bytes) => {
            write_private(home, slug, "grants.json", bytes)?;
        }
        None => {
            remove_private(home, slug, "grants.json")?;
        }
    }
    Ok(())
}
fn after_failed_append(
    restored: Result<()>,
    remove: impl FnOnce() -> Result<bool>,
    appended: Error,
) -> Error {
    if restored.is_err() && remove().is_ok() {
        Error::Invalid(text::GRANT_NOT_LOGGED.into())
    } else {
        appended
    }
}
/// Save and log consent; the caller holds the state lock.
pub fn give(
    home: &Path,
    slug: &str,
    capability: Capability,
    window: Window,
    now: SystemTime,
) -> Result<Grant> {
    let (mut grants, before) = loaded(home, slug)?;
    let grant = Grant {
        capability,
        given_on: ledger::local_day(now),
        window,
    };
    grants.retain(|g| g.capability != capability);
    grants.push(grant.clone());
    save(home, slug, &grants)?;
    if let Err(error) = append(
        home,
        slug,
        RawEvent::Grant {
            capability: capability.id().into(),
            past_days: window.past_days,
            next_days: window.next_days,
        },
        now,
    ) {
        return Err(after_failed_append(
            restore(home, slug, before.as_deref()),
            || remove_private(home, slug, "grants.json"),
            error,
        ));
    }
    Ok(grant)
}
/// Take back consent and log it; the caller holds the state lock.
pub fn revoke(home: &Path, slug: &str, capability: Capability, now: SystemTime) -> Result<bool> {
    let (mut grants, before) = loaded(home, slug)?;
    let count = grants.len();
    grants.retain(|g| g.capability != capability);
    if count == grants.len() {
        return Ok(false);
    }
    save(home, slug, &grants)?;
    if let Err(error) = append(
        home,
        slug,
        RawEvent::Revoke {
            capability: capability.id().into(),
        },
        now,
    ) {
        // Keep the append error even if restoring the previous file also fails.
        let _ = restore(home, slug, before.as_deref());
        return Err(error);
    }
    Ok(true)
}
/// Check consent before a capability read, logging every refusal.
pub fn require(
    home: &Path,
    slug: &str,
    capability: Capability,
    step: &str,
    now: SystemTime,
) -> Result<Grant> {
    check_step(step)?;
    let (reason, error) = match load(home, slug) {
        Ok(grants) => match grants.into_iter().find(|g| g.capability == capability) {
            Some(grant) => return Ok(grant),
            None => (Reason::NoGrant, Error::Invalid(text::no_grant(capability))),
        },
        Err(error) => (Reason::Damaged, error),
    };
    let logged = append(
        home,
        slug,
        RawEvent::Refused {
            capability: capability.id().into(),
            step: step.into(),
            reason: reason.id().into(),
        },
        now,
    );
    match logged {
        Err(Error::Io { source, .. })
            if reason == Reason::NoGrant && source.kind() == std::io::ErrorKind::NotFound => {}
        result => result?,
    }
    Err(error)
}
pub fn record_read(
    home: &Path,
    slug: &str,
    capability: Capability,
    step: &str,
    items: u64,
    now: SystemTime,
) -> Result<()> {
    check_step(step)?;
    append(
        home,
        slug,
        RawEvent::Read {
            capability: capability.id().into(),
            step: step.into(),
            items,
        },
        now,
    )
}

#[derive(Debug)]
pub struct Audit {
    pub entries: Vec<Entry>,
    pub unreadable: usize,
    pub older: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub at: String,
    pub event: Event,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Grant {
        capability: Capability,
        window: Window,
    },
    Revoke {
        capability: Capability,
    },
    Read {
        capability: Capability,
        step: String,
        items: u64,
    },
    Refused {
        capability: Capability,
        step: String,
        reason: Reason,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NoGrant,
    Damaged,
}
impl Reason {
    fn id(self) -> &'static str {
        match self {
            Self::NoGrant => "no-grant",
            Self::Damaged => "damaged",
        }
    }
    fn from_id(id: &str) -> Option<Self> {
        match id {
            "no-grant" => Some(Self::NoGrant),
            "damaged" => Some(Self::Damaged),
            _ => None,
        }
    }
}
#[derive(Serialize, Deserialize)]
struct RawEntry {
    at: String,
    #[serde(flatten)]
    event: RawEvent,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase", deny_unknown_fields)]
enum RawEvent {
    Grant {
        capability: String,
        past_days: u16,
        next_days: u16,
    },
    Revoke {
        capability: String,
    },
    Read {
        capability: String,
        step: String,
        items: u64,
    },
    Refused {
        capability: String,
        step: String,
        reason: String,
    },
}
fn append(home: &Path, slug: &str, event: RawEvent, now: SystemTime) -> Result<()> {
    let mut bytes = serde_json::to_vec(&RawEntry {
        at: ledger::utc_stamp(now),
        event,
    })
    .expect("audit entries serialize");
    bytes.push(b'\n');
    append_private(home, slug, "audit.jsonl", &bytes)?;
    Ok(())
}
fn entry(bytes: &[u8]) -> Option<Entry> {
    let raw: RawEntry = serde_json::from_slice(bytes).ok()?;
    if !stamp(&raw.at) {
        return None;
    }
    let event = match raw.event {
        RawEvent::Grant {
            capability,
            past_days,
            next_days,
        } => Event::Grant {
            capability: Capability::from_id(&capability)?,
            window: Window::new(past_days, next_days).ok()?,
        },
        RawEvent::Revoke { capability } => Event::Revoke {
            capability: Capability::from_id(&capability)?,
        },
        RawEvent::Read {
            capability,
            step,
            items,
        } => {
            if !valid_step(&step) {
                return None;
            }
            Event::Read {
                capability: Capability::from_id(&capability)?,
                step,
                items,
            }
        }
        RawEvent::Refused {
            capability,
            step,
            reason,
        } => {
            if !valid_step(&step) {
                return None;
            }
            Event::Refused {
                capability: Capability::from_id(&capability)?,
                step,
                reason: Reason::from_id(&reason)?,
            }
        }
    };
    Some(Entry { at: raw.at, event })
}
/// The newest complete entries from at most the last MiB; malformed lines are counted, never shown.
pub fn audit(home: &Path, slug: &str, last: usize) -> Result<Audit> {
    if !(1..=1000).contains(&last) {
        return Err(Error::Invalid(text::AUDIT_LAST.into()));
    }
    let mut audit = Audit {
        entries: Vec::new(),
        unreadable: 0,
        older: false,
    };
    let Some((bytes, skipped)) =
        read_private_tail_optional(home, slug, "audit.jsonl", 1024 * 1024)?
    else {
        return Ok(audit);
    };
    audit.older = skipped;
    let mut bytes = bytes.as_slice();
    if skipped {
        bytes = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(&[], |i| &bytes[i + 1..]);
    }
    if bytes.is_empty() {
        return Ok(audit);
    }
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    for line in bytes.rsplit(|b| *b == b'\n') {
        match entry(line) {
            Some(entry) if audit.entries.len() < last => audit.entries.push(entry),
            Some(_) => audit.older = true,
            None => audit.unreadable += 1,
        }
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_grants_keep_the_append_error_without_removing() {
        let error = after_failed_append(
            Ok(()),
            || panic!("a successful restore must not remove grants"),
            Error::Invalid("append failed".into()),
        );
        assert_eq!(error.to_string(), "append failed");
    }

    #[test]
    fn failed_restore_removes_grants_and_explains_for_either_remove_result() {
        for removed in [true, false] {
            let called = std::cell::Cell::new(false);
            let error = after_failed_append(
                Err(Error::Invalid("restore failed".into())),
                || {
                    called.set(true);
                    Ok(removed)
                },
                Error::Invalid("append failed".into()),
            );
            assert!(called.get());
            assert_eq!(
                error.to_string(),
                "Voice couldn't log this, so it took back every permission for this assistant. Give permission again once there's space on this computer."
            );
        }
    }

    #[test]
    fn failed_restore_and_remove_keep_the_append_error() {
        let called = std::cell::Cell::new(false);
        let error = after_failed_append(
            Err(Error::Invalid("restore failed".into())),
            || {
                called.set(true);
                Err(Error::Invalid("remove failed".into()))
            },
            Error::Invalid("append failed".into()),
        );
        assert!(called.get());
        assert_eq!(error.to_string(), "append failed");
    }
}
