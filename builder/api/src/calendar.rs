//! The chosen calendar export and its grant-checked reader broker.
//!
//! In-process calendar reading is private to the reader dispatch.
//! ```compile_fail,E0603
//! use voice_api::calendar::read_calendar_in;
//! ```
use crate::{
    dto::*,
    history::{self, failure},
    reader,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use std::{path::Path, time::SystemTime};
use voice_agent::{
    grants::{self, Capability},
    ledger, text,
};
use voice_core::{
    assistants,
    deny::DenyList,
    error::Error,
    member::{self, Member},
};
pub use voice_ingest::calendar::{BAD, HARD_LINK, TOO_BIG, TOO_MANY};
use voice_ingest::{
    calendar::{
        self,
        zone::{ZONEINFO, Zone},
    },
    dates::{civil_from_days, days_from_civil, days_in_month},
    filters::Filters,
    private,
};
type Result<T> = std::result::Result<T, Failure>;
pub const NONE: &str = "Choose the calendar file you exported first.";
pub const GONE: &str = "Voice can't find your calendar file any more. Choose it again.";
pub const LINK: &str = "Voice doesn't follow links. Choose the calendar file itself.";
pub const NOT_ICS: &str = "Choose a calendar you exported: an .ics file, or a folder of them.";
pub const ZONE: &str = "Voice couldn't tell which time zone this Mac is set to. Set it in System Settings, then try again.";
pub const LOG: &str = "Voice couldn't write down that it read your calendar, so it didn't read it. Check there's space on this computer, then try again.";
pub const BAD_GRANTS: &str = "Voice's permissions file for this assistant is damaged, so it read nothing. Give permission again.";
pub const NO_STATE: &str =
    "This assistant has no private state yet. Build it once, then choose your calendar.";
pub const MAC_ONLY_CALENDAR: &str =
    "Voice reads your calendar inside the Mac's own sandbox, so for now it works on a Mac only.";
/// Find this assistant without requiring consent to read conversation history.
pub(crate) fn member_in(home: &Path, slug: &str) -> Result<Member> {
    assistants::assistant_dir(home, slug).map_err(|_| failure(history::FIND))?;
    let member = assistants::find(home, slug)
        .map_err(|_| failure(history::FIND))?
        .ok_or_else(|| failure(history::FIND))?
        .member;
    member::private_state_dir(&member, home).ok_or_else(|| failure(history::ELSEWHERE))?;
    Ok(member)
}
const EXPORT: &str = "calendar-export.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportChoice {
    path: String,
    folder: bool,
}
fn named(path: &Path, folder: bool) -> CalendarExport {
    CalendarExport {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        folder,
    }
}
pub(crate) fn export_allowed(home: &Path, member: &Member, path: &Path) -> Result<DenyList> {
    let deny = DenyList::for_member(member, home).map_err(|_| failure(BAD))?;
    deny.check(path).map_err(|_| failure(BAD))?;
    Ok(deny)
}
pub fn choose_calendar_export_in(home: &Path, slug: &str, path: &Path) -> Result<CalendarExport> {
    let member = member_in(home, slug)?;
    let _lock = private::lock_existing_state(home, slug).map_err(|e| match e {
        Error::Invalid(text) if text == private::BUSY => failure(history::BUSY),
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            failure(NO_STATE)
        }
        _ => failure(history::SAVE),
    })?;
    let text = path
        .to_str()
        .filter(|_| path.is_absolute())
        .ok_or_else(|| failure(NOT_ICS))?;
    export_allowed(home, &member, path)?;
    let metadata = fs::symlink_metadata(path).map_err(|e| {
        failure(if e.kind() == std::io::ErrorKind::NotFound {
            GONE
        } else {
            BAD
        })
    })?;
    if metadata.file_type().is_symlink() {
        return Err(failure(LINK));
    }
    let folder = metadata.is_dir();
    if !folder
        && (!metadata.is_file()
            || !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ics")))
    {
        return Err(failure(NOT_ICS));
    }
    let bytes = serde_json::to_vec(&ExportChoice {
        path: text.into(),
        folder,
    })
    .map_err(|_| failure(BAD))?;
    private::write_private(home, slug, EXPORT, &bytes).map_err(|_| failure(history::SAVE))?;
    Ok(named(path, folder))
}
pub(crate) fn chosen_export(home: &Path, slug: &str) -> Result<Option<(PathBuf, bool)>> {
    private::read_private_owned_optional_at_max(home, slug, EXPORT, 64 * 1024)
        .map_err(|_| failure(BAD))?
        .map(|bytes| {
            let choice: ExportChoice = serde_json::from_slice(&bytes).map_err(|_| failure(BAD))?;
            let path = PathBuf::from(choice.path);
            if !path.is_absolute() {
                return Err(failure(BAD));
            }
            fs::metadata(&path).map_err(|e| {
                failure(if e.kind() == std::io::ErrorKind::NotFound {
                    GONE
                } else {
                    BAD
                })
            })?;
            Ok((path, choice.folder))
        })
        .transpose()
}
pub fn calendar_export_in(home: &Path, slug: &str) -> Result<Option<CalendarExport>> {
    member_in(home, slug)?;
    Ok(chosen_export(home, slug)?.map(|(path, folder)| named(&path, folder)))
}
// Shape checks precede string slicing, including on untrusted UTF-8 reader output.
fn day(value: &str) -> Option<i64> {
    let b = value.as_bytes();
    if b.len() != 10
        || !b.iter().enumerate().all(|(i, b)| {
            if matches!(i, 4 | 7) {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return None;
    }
    let year = value[..4].parse().ok()?;
    let month = value[5..7].parse().ok()?;
    let date = value[8..].parse().ok()?;
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || date < 1
        || date > days_in_month(year, month)
    {
        return None;
    }
    Some(days_from_civil(year, month, date))
}
fn date(value: i64) -> calendar::Date {
    let (year, month, day) = civil_from_days(value);
    calendar::Date {
        year,
        month: month as u8,
        day: day as u8,
    }
}
fn bounds(today: i64, window: grants::Window) -> (String, String) {
    (
        date(today - i64::from(window.past_days())).to_string(),
        date(today + i64::from(window.next_days())).to_string(),
    )
}
pub(crate) fn read_calendar_in(home: &Path, request: reader::CalendarStep) -> Result<CalendarRead> {
    let member = member_in(home, &request.slug)?;
    let today = day(&request.today).ok_or_else(|| failure(reader::REQUEST))?;
    let own = ledger::utc_stamp(SystemTime::now());
    if (today - day(&own[..10]).ok_or_else(|| failure(reader::REQUEST))?).abs() > 1 {
        return Err(failure(reader::REQUEST));
    }
    let zone = Zone::load(Path::new(ZONEINFO), &request.zone).ok_or_else(|| failure(ZONE))?;
    let grant = grants::load(home, &request.slug)
        .map_err(|_| failure(BAD_GRANTS))?
        .into_iter()
        .find(|g| g.capability == Capability::CalendarRead)
        .ok_or_else(|| failure(&text::no_grant(Capability::CalendarRead)))?;
    let (path, folder) = chosen_export(home, &request.slug)?.ok_or_else(|| failure(NONE))?;
    let deny = export_allowed(home, &member, &path)?;
    let files = calendar::read_export(&path, &deny, folder).map_err(|e| failure(&e.to_string()))?;
    let filters = Filters::compile(&member.filters).map_err(|_| failure(BAD))?;
    let read = calendar::read(
        &files,
        &zone,
        Path::new(ZONEINFO),
        calendar::Window {
            today: date(today),
            past_days: grant.window.past_days(),
            next_days: grant.window.next_days(),
        },
        &filters,
    )
    .map_err(|_| failure(BAD))?;
    Ok(CalendarRead {
        events: read
            .events
            .into_iter()
            .map(|e| CalendarEvent {
                title: e.title,
                place: e.place,
                start: e.start,
                end: e.end,
                all_day: e.all_day,
                status: e.status.map(|s| match s {
                    calendar::Status::Tentative => EventStatus::Tentative,
                    calendar::Status::Confirmed => EventStatus::Confirmed,
                    calendar::Status::Cancelled => EventStatus::Cancelled,
                }),
            })
            .collect(),
        from: read.from.to_string(),
        to: read.to.to_string(),
        private_left_out: history::count(read.private_left_out),
        not_understood: history::count(read.not_understood),
        more: read.more,
    })
}
fn local_zone() -> Result<String> {
    let setting = std::env::var_os("TZ").filter(|s| !s.is_empty());
    let name = if let Some(setting) = setting {
        let setting = setting.to_str().ok_or_else(|| failure(ZONE))?;
        let setting = setting.strip_prefix(':').unwrap_or(setting);
        if setting.starts_with('/') {
            setting
                .rsplit_once("zoneinfo/")
                .map(|(_, name)| name)
                .ok_or_else(|| failure(ZONE))?
        } else {
            setting
        }
        .to_owned()
    } else {
        let path = fs::read_link("/etc/localtime").map_err(|_| failure(ZONE))?;
        path.to_str()
            .and_then(|path| path.rsplit_once("zoneinfo/").map(|(_, name)| name))
            .ok_or_else(|| failure(ZONE))?
            .to_owned()
    };
    Zone::load(Path::new(ZONEINFO), &name).ok_or_else(|| failure(ZONE))?;
    Ok(name)
}
fn instant(value: &str, all_day: bool) -> Option<i64> {
    if all_day {
        return day(value)?.checked_mul(1440);
    }
    let b = value.as_bytes();
    if b.len() != 16
        || b[10] != b'T'
        || b[13] != b':'
        || !b[11..13].iter().chain(&b[14..16]).all(u8::is_ascii_digit)
    {
        return None;
    }
    let date = day(value.get(..10)?)?;
    let hour: i64 = value[11..13].parse().ok()?;
    let minute: i64 = value[14..16].parse().ok()?;
    (hour < 24 && minute < 60).then_some(date * 1440 + hour * 60 + minute)
}
fn clean(value: &str) -> bool {
    value.chars().count() <= 201 && !value.chars().any(voice_ingest::text::hidden)
}
fn checked(answer: &CalendarRead, today: i64, window: grants::Window) -> bool {
    let (from, to) = bounds(today, window);
    if answer.from != from || answer.to != to || answer.events.len() > calendar::MAX_EVENTS {
        return false;
    }
    let first = today - i64::from(window.past_days());
    let last = today + i64::from(window.next_days());
    answer.events.iter().all(|event| {
        let Some(start) = instant(&event.start, event.all_day) else {
            return false;
        };
        let Some(end) = instant(&event.end, event.all_day) else {
            return false;
        };
        let final_minute = if !event.all_day && end > start && end.rem_euclid(1440) == 0 {
            end - 1
        } else {
            end
        };
        end >= start
            && start.div_euclid(1440) <= last
            && final_minute.div_euclid(1440) >= first
            && clean(&event.title)
            && event.place.as_ref().is_none_or(|place| clean(place))
    })
}
pub fn read_calendar_through(
    home: &Path,
    slug: &str,
    step: &str,
    now: SystemTime,
    run: impl FnOnce(&reader::CalendarStep) -> Result<CalendarRead>,
) -> Result<CalendarRead> {
    member_in(home, slug)?;
    chosen_export(home, slug)?.ok_or_else(|| failure(NONE))?;
    let grant =
        grants::require(home, slug, Capability::CalendarRead, step, now).map_err(|e| match e {
            Error::Invalid(text) => failure(&text),
            _ => failure(LOG),
        })?;
    let today = ledger::local_day(now);
    let day = day(&today).ok_or_else(|| failure(ZONE))?;
    let answer = run(&reader::CalendarStep {
        slug: slug.into(),
        today,
        zone: local_zone()?,
    })?;
    if !checked(&answer, day, grant.window) {
        return Err(failure(reader::STOPPED));
    }
    grants::record_read(
        home,
        slug,
        Capability::CalendarRead,
        step,
        answer.events.len() as u64,
        now,
    )
    .map_err(|_| failure(LOG))?;
    Ok(answer)
}
pub fn read_calendar(request: SlugRequest, step: &str) -> Result<CalendarRead> {
    #[cfg(target_os = "macos")]
    {
        history::with_reader(|home, place| {
            crate::reader_client::read_calendar(home, place, &request.slug, step)
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (request, step);
        Err(failure(MAC_ONLY_CALENDAR))
    }
}

#[cfg(test)]
mod tests;
