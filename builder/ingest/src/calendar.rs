//! Bounded calendar exports, read only by the consented reader.
use crate::{
    dates::{civil_from_days, days_from_civil, days_in_month},
    filters::Filters,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    path::Path,
};
use voice_core::error::{Error, Result};
mod ics;
mod read;
mod recur;
pub mod zone;
use ics::{Entry, Frame, Time};
pub use read::read_export;
use recur::Rule;
use zone::Zone;

pub const BAD: &str = "Voice couldn't read this calendar. Check it's a calendar you exported (an .ics file), then choose it again.";
pub const TOO_MANY: &str =
    "This folder has more than 64 calendar files. Choose a folder with fewer, or one file.";
pub const TOO_BIG: &str = "This calendar is bigger than Voice reads (128 MB). Export a shorter stretch of time, then choose it again.";
pub const MAX_EVENTS: usize = 2_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub year: i64,
    pub month: u8,
    pub day: u8,
}
impl Date {
    fn day(self) -> Option<i64> {
        ((1..=9999).contains(&self.year)
            && (1..=12).contains(&self.month)
            && self.day > 0
            && i64::from(self.day) <= days_in_month(self.year, i64::from(self.month)))
        .then(|| days_from_civil(self.year, i64::from(self.month), i64::from(self.day)))
    }
    fn from_day(day: i64) -> Option<Self> {
        let (year, month, day) = civil_from_days(day);
        (1..=9999).contains(&year).then_some(Self {
            year,
            month: month as u8,
            day: day as u8,
        })
    }
}
impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Tentative,
    Confirmed,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub title: String,
    pub place: Option<String>,
    pub start: String,
    pub end: String,
    pub all_day: bool,
    pub status: Option<Status>,
}
#[derive(Debug)]
pub struct Calendar {
    pub events: Vec<Event>,
    pub from: Date,
    pub to: Date,
    pub private_left_out: usize,
    pub not_understood: usize,
    pub more: bool,
}
#[derive(Debug, Clone, Copy)]
pub struct Window {
    pub today: Date,
    pub past_days: u16,
    pub next_days: u16,
}
fn invalid(text: &str) -> Error {
    Error::Invalid(text.into())
}

struct Zones<'a> {
    local: &'a Zone,
    root: &'a Path,
    loaded: HashMap<String, Option<Zone>>,
}
impl Zones<'_> {
    fn utc(&mut self, time: &Time) -> Option<i64> {
        match &time.frame {
            Frame::Date | Frame::Utc => Some(time.wall),
            Frame::Local => self.local.to_utc(time.wall),
            Frame::Named(name) => self
                .loaded
                .entry(name.clone())
                .or_insert_with(|| Zone::load(self.root, name))
                .as_ref()?
                .to_utc(time.wall),
        }
    }
    fn local(&self, utc: i64) -> Option<i64> {
        utc.checked_add(i64::from(self.local.offset_at(utc)?))
    }
}
struct Parsed {
    start: Time,
    duration: i64,
    rule: Option<Rule>,
    until: Option<i64>,
    exceptions: HashSet<i64>,
    id: Option<Time>,
    uid: Option<String>,
    title: String,
    place: Option<String>,
    status: Option<Status>,
}
impl Parsed {
    fn entry(entry: &Entry, zones: &mut Zones<'_>) -> Option<Self> {
        let f = &entry.fields;
        if entry.bad || f.contains_key("RDATE") || f.contains_key("EXRULE") {
            return None;
        }
        let start = f.get("DTSTART")?.time()?;
        let rule = match f.get("RRULE") {
            Some(p) => Some(Rule::parse(&p.value, &start)?),
            None => None,
        };
        let uid = f
            .get("UID")
            .map(|p| p.value.clone())
            .filter(|v| !v.is_empty());
        let id = if let Some(p) = f.get("RECURRENCE-ID") {
            if p.params.contains_key("RANGE") || rule.is_some() || uid.is_none() {
                return None;
            }
            Some(p.time()?)
        } else {
            None
        };
        let first = zones.utc(&start)?;
        let duration = if let Some(end) = f.get("DTEND") {
            let end = end.time()?;
            if end.all_day() != start.all_day() {
                return None;
            }
            zones.utc(&end)?.checked_sub(first)?
        } else if let Some(duration) = f.get("DURATION") {
            let n = ics::duration(&duration.value)?;
            if start.all_day() && n % 86400 != 0 {
                return None;
            }
            n
        } else if start.all_day() {
            86400
        } else {
            0
        };
        if duration < 0 || start.all_day() && duration == 0 {
            return None;
        }
        let mut exceptions = HashSet::new();
        for p in &entry.exceptions {
            for v in p.value.split(',') {
                let mut p = p.clone();
                p.value = v.into();
                let ex = p.time()?;
                if ex.frame != start.frame {
                    return None;
                }
                exceptions.insert(ex.wall);
            }
        }
        let until = if let Some(text) = rule.as_ref().and_then(|r| r.until.as_ref()) {
            let wall = ics::value(text, start.all_day())?;
            let frame = if start.all_day() {
                Frame::Date
            } else if text.ends_with('Z') {
                Frame::Utc
            } else {
                start.frame.clone()
            };
            Some(zones.utc(&Time { wall, frame })?)
        } else {
            None
        };
        let title = f
            .get("SUMMARY")
            .map_or_else(String::new, |p| ics::clean(&p.value));
        let place = f
            .get("LOCATION")
            .map(|p| ics::clean(&p.value))
            .filter(|s| !s.is_empty());
        let status = f
            .get("STATUS")
            .and_then(|p| match p.value.to_ascii_uppercase().as_str() {
                "TENTATIVE" => Some(Status::Tentative),
                "CONFIRMED" => Some(Status::Confirmed),
                "CANCELLED" => Some(Status::Cancelled),
                _ => None,
            });
        Some(Self {
            start,
            duration,
            rule,
            until,
            exceptions,
            id,
            uid,
            title,
            place,
            status,
        })
    }
    fn event(
        &self,
        wall: i64,
        zones: &mut Zones<'_>,
        first_day: i64,
        last_day: i64,
    ) -> Option<Option<Event>> {
        let all_day = self.start.all_day();
        let utc = zones.utc(&Time {
            wall,
            frame: self.start.frame.clone(),
        })?;
        let end = utc.checked_add(self.duration)?;
        let (start, end, last) = if all_day {
            (utc, end - 86400, end - 86400)
        } else {
            let start = zones.local(utc)?;
            let end = zones.local(end)?;
            // DTEND is exclusive (§3.6.1); a zero-length event still belongs to its start day.
            (
                start,
                end,
                if end.rem_euclid(86400) == 0 && self.duration != 0 {
                    end - 1
                } else {
                    end
                },
            )
        };
        if start.div_euclid(86400) > last_day || last.div_euclid(86400) < first_day {
            return Some(None);
        }
        Some(Some(Event {
            title: self.title.clone(),
            place: self.place.clone(),
            start: shown(start, all_day)?,
            end: shown(end, all_day)?,
            all_day,
            status: self.status,
        }))
    }
}
fn shown(wall: i64, all_day: bool) -> Option<String> {
    let date = Date::from_day(wall.div_euclid(86400))?;
    if all_day {
        Some(date.to_string())
    } else {
        let seconds = wall.rem_euclid(86400);
        Some(format!(
            "{date}T{:02}:{:02}",
            seconds / 3600,
            seconds % 3600 / 60
        ))
    }
}
fn could_touch(entry: &Entry, first: i64, last: i64) -> bool {
    let Some(start) = entry.fields.get("DTSTART").and_then(|p| p.time()) else {
        return true;
    };
    let day = start.wall.div_euclid(86400);
    if ["RRULE", "RDATE", "EXRULE"]
        .iter()
        .any(|key| entry.fields.contains_key(*key))
    {
        day <= last + 1
    } else {
        day >= first - 1 && day <= last + 1
    }
}
#[derive(Default)]
struct Kept {
    events: BTreeMap<(String, String, String, usize), Event>,
    private: usize,
    sequence: usize,
    more: bool,
}
impl Kept {
    fn add(&mut self, mut event: Event, filters: &Filters) {
        if filters.kind(&event.title).is_some()
            || event
                .place
                .as_ref()
                .is_some_and(|p| filters.kind(p).is_some())
        {
            self.private += 1;
            return;
        }
        event.title = ics::cut(&event.title);
        event.place = event.place.map(|place| ics::cut(&place));
        self.insert(event);
    }
    fn insert(&mut self, event: Event) {
        self.sequence += 1;
        self.events.insert(
            (
                event.start.clone(),
                event.end.clone(),
                event.title.clone(),
                self.sequence,
            ),
            event,
        );
        if self.events.len() > MAX_EVENTS {
            self.events.pop_last();
            self.more = true;
        }
    }
    fn append(&mut self, other: Kept) {
        self.private += other.private;
        self.more |= other.more;
        for event in other.events.into_values() {
            self.insert(event);
        }
    }
}
pub fn read(
    files: &[Vec<u8>],
    local: &Zone,
    zones: &Path,
    window: Window,
    filters: &Filters,
) -> Result<Calendar> {
    let today = window.today.day().ok_or_else(|| invalid(BAD))?;
    let first = today - i64::from(window.past_days);
    let last = today + i64::from(window.next_days);
    let from = Date::from_day(first).ok_or_else(|| invalid(BAD))?;
    let to = Date::from_day(last).ok_or_else(|| invalid(BAD))?;
    let entries = ics::entries(files)?;
    let series_uids: HashSet<_> = entries
        .iter()
        .filter(|entry| !entry.fields.contains_key("RECURRENCE-ID"))
        .filter_map(|entry| entry.fields.get("UID").map(|uid| uid.value.as_str()))
        .collect();
    let mut zones = Zones {
        local,
        root: zones,
        loaded: HashMap::new(),
    };
    let mut not_understood = 0;
    let parsed: Vec<_> = entries
        .iter()
        .map(|entry| {
            let parsed = Parsed::entry(entry, &mut zones);
            if parsed.is_none() && could_touch(entry, first, last) {
                not_understood += 1;
            }
            parsed
        })
        .collect();
    let mut overrides: HashMap<&str, HashMap<Time, Vec<usize>>> = HashMap::new();
    for (i, p) in parsed.iter().enumerate() {
        if let Some(p) = p
            && let (Some(uid), Some(id)) = (&p.uid, &p.id)
        {
            overrides
                .entry(uid.as_str())
                .or_default()
                .entry(id.clone())
                .or_default()
                .push(i);
        }
    }
    let mut used = HashSet::new();
    let mut kept = Kept::default();
    for (i, p) in parsed.iter().enumerate() {
        let Some(p) = p else {
            continue;
        };
        if p.id.is_some() {
            if p.uid
                .as_ref()
                .is_some_and(|uid| !series_uids.contains(uid.as_str()))
                && let Some(event) = p.event(p.start.wall, &mut zones, first, last)
            {
                if let Some(event) = event {
                    kept.add(event, filters);
                }
                used.insert(i);
            }
            continue;
        }
        let mut series = Kept::default();
        let mut series_used = Vec::new();
        let choices = p.uid.as_ref().and_then(|uid| overrides.get(uid.as_str()));
        let replacements: HashMap<i64, usize> = choices
            .into_iter()
            .flat_map(|choices| choices.iter())
            .filter(|(id, ids)| id.frame == p.start.frame && ids.len() == 1)
            .map(|(id, ids)| (id.wall, ids[0]))
            .collect();
        // The ordinary expansion stops at last + 2. The same bounded walk checks membership
        // of moved overrides beyond that horizon, without converting unrelated candidates.
        let membership_end = replacements
            .keys()
            .map(|wall| wall.div_euclid(86400))
            .max()
            .unwrap_or(last + 2);
        let mut visit = |wall: i64| {
            // Even an old UNTIL stops the walk, without converting skipped wall times.
            // TZif offsets are at most 93599 seconds (RFC 9636 §3.2).
            if p.until
                .is_some_and(|until| wall.saturating_sub(93_599) > until)
            {
                return Some(false);
            }
            if let Some(&index) = replacements.get(&wall) {
                if p.until.is_some_and(|until| {
                    zones
                        .utc(&Time {
                            wall,
                            frame: p.start.frame.clone(),
                        })
                        .is_none_or(|utc| utc > until)
                }) {
                    return Some(false);
                }
                if p.exceptions.contains(&wall) {
                    series_used.push(index);
                    return Some(true);
                }
                let moved = parsed[index].as_ref().expect("parsed override");
                if let Some(event) = moved.event(moved.start.wall, &mut zones, first, last) {
                    if let Some(event) = event {
                        series.add(event, filters);
                    }
                    series_used.push(index);
                    return Some(true);
                }
            }
            let day = wall.div_euclid(86400);
            if p.rule.is_some() && (day < first - 2 || day > last + 2) {
                return Some(true);
            }
            let time = Time {
                wall,
                frame: p.start.frame.clone(),
            };
            let utc = zones.utc(&time)?;
            if p.until.is_some_and(|until| utc > until) {
                return Some(false);
            }
            if !p.exceptions.contains(&wall)
                && let Some(event) = p.event(wall, &mut zones, first, last)?
            {
                series.add(event, filters);
            }
            Some(true)
        };
        let understood = if let Some(rule) = &p.rule {
            rule.expand(&p.start, (last + 2).max(membership_end), &mut visit)
        } else {
            visit(p.start.wall).map(|_| ())
        };
        if understood.is_none() {
            if could_touch(&entries[i], first, last) {
                not_understood += 1;
            }
        } else {
            kept.append(series);
            used.extend(series_used);
        }
    }
    for (i, p) in parsed.iter().enumerate() {
        if p.as_ref().is_some_and(|p| p.id.is_some())
            && !used.contains(&i)
            && could_touch(&entries[i], first, last)
        {
            not_understood += 1;
        }
    }
    Ok(Calendar {
        events: kept.events.into_values().collect(),
        from,
        to,
        private_left_out: kept.private,
        not_understood,
        more: kept.more,
    })
}
