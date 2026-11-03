//! RFC 5545 §3.3.10: a bounded subset, expanded in DTSTART's civil frame.
use super::ics::{Time, number};
use crate::dates::{civil_from_days, days_from_civil, days_in_month};
use std::collections::BTreeMap;

const MAX_STEPS: usize = 500_000;
#[derive(Debug)]
enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}
#[derive(Debug)]
pub(super) struct Rule {
    frequency: Frequency,
    interval: i64,
    pub count: Option<usize>,
    pub until: Option<String>,
    week_start: i64,
    weekdays: Vec<i64>,
    monthdays: Vec<i64>,
    ordinal_days: Vec<(i64, i64)>,
}
fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}
fn day_name(day: &str) -> Option<i64> {
    ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]
        .iter()
        .position(|name| *name == day)
        .map(|d| d as i64)
}
fn positive(text: &str, max: i64) -> Option<i64> {
    number(text.as_bytes()).filter(|n| (1..=max).contains(n))
}
impl Rule {
    pub fn parse(text: &str, start: &Time) -> Option<Self> {
        let mut parts = BTreeMap::new();
        for part in text.split(';') {
            let (key, value) = part.split_once('=')?;
            if parts
                .insert(key.to_ascii_uppercase(), value.to_ascii_uppercase())
                .is_some()
            {
                return None;
            }
        }
        if parts.keys().any(|key| {
            ![
                "FREQ",
                "INTERVAL",
                "COUNT",
                "UNTIL",
                "BYDAY",
                "WKST",
                "BYMONTHDAY",
                "BYMONTH",
            ]
            .contains(&key.as_str())
        }) {
            return None;
        }
        let frequency = match parts.get("FREQ")?.as_str() {
            "DAILY" => Frequency::Daily,
            "WEEKLY" => Frequency::Weekly,
            "MONTHLY" => Frequency::Monthly,
            "YEARLY" => Frequency::Yearly,
            _ => return None,
        };
        let interval = parts
            .get("INTERVAL")
            .map_or(Some(1), |v| positive(v, 1000))?;
        let count = match parts.get("COUNT") {
            Some(v) => Some(positive(v, 100_000)? as usize),
            None => None,
        };
        let until = parts.get("UNTIL").cloned();
        if count.is_some() && until.is_some() {
            return None;
        }
        let mut rule = Self {
            frequency,
            interval,
            count,
            until,
            week_start: 0,
            weekdays: Vec::new(),
            monthdays: Vec::new(),
            ordinal_days: Vec::new(),
        };
        if let Some(wkst) = parts.get("WKST") {
            if !matches!(rule.frequency, Frequency::Weekly) {
                return None;
            }
            rule.week_start = day_name(wkst)?;
        }
        if let Some(days) = parts.get("BYDAY") {
            for day in days.split(',') {
                match rule.frequency {
                    Frequency::Weekly => rule.weekdays.push(day_name(day)?),
                    Frequency::Monthly => {
                        let split = day.len().checked_sub(2)?;
                        if !day.is_ascii() {
                            return None;
                        }
                        let (ordinal, name) = day.split_at(split);
                        let ordinal = if let Some(n) = ordinal.strip_prefix('-') {
                            -positive(n, 5)?
                        } else {
                            positive(ordinal.strip_prefix('+').unwrap_or(ordinal), 5)?
                        };
                        rule.ordinal_days.push((ordinal, day_name(name)?));
                    }
                    _ => return None,
                }
            }
        }
        if let Some(days) = parts.get("BYMONTHDAY") {
            if !matches!(rule.frequency, Frequency::Monthly) || !rule.ordinal_days.is_empty() {
                return None;
            }
            rule.monthdays = days
                .split(',')
                .map(|d| positive(d, 31))
                .collect::<Option<_>>()?;
        }
        if let Some(month) = parts.get("BYMONTH")
            && (!matches!(rule.frequency, Frequency::Yearly)
                || positive(month, 12)? != civil_from_days(start.wall.div_euclid(86400)).1)
        {
            return None;
        }
        rule.weekdays.sort_unstable();
        rule.weekdays.dedup();
        rule.monthdays.sort_unstable();
        rule.monthdays.dedup();
        Some(rule)
    }
    pub fn expand(
        &self,
        start: &Time,
        last_day: i64,
        mut visit: impl FnMut(i64) -> Option<bool>,
    ) -> Option<()> {
        // DTSTART counts first even if it does not satisfy the BY selectors (§3.3.10).
        if !visit(start.wall)? || self.count == Some(1) {
            return Some(());
        }
        let start_day = start.wall.div_euclid(86400);
        let time = start.wall.rem_euclid(86400);
        let (year, month, day) = civil_from_days(start_day);
        let week = start_day - (weekday(start_day) - self.week_start).rem_euclid(7);
        let mut emitted = 1;
        let mut steps = 0;
        for period in 0i64.. {
            steps += 1;
            if steps > MAX_STEPS {
                return None;
            }
            let n = period.checked_mul(self.interval)?;
            let (first, mut days) = match self.frequency {
                Frequency::Daily => {
                    let d = start_day.checked_add(n)?;
                    (d, vec![d])
                }
                Frequency::Weekly => {
                    let first = week.checked_add(n.checked_mul(7)?)?;
                    let days = if self.weekdays.is_empty() {
                        vec![first + (weekday(start_day) - self.week_start).rem_euclid(7)]
                    } else {
                        self.weekdays
                            .iter()
                            .map(|d| first + (d - self.week_start).rem_euclid(7))
                            .collect()
                    };
                    (first, days)
                }
                Frequency::Monthly => {
                    let serial = year
                        .checked_mul(12)?
                        .checked_add(month - 1)?
                        .checked_add(n)?;
                    let y = serial.div_euclid(12);
                    let m = serial.rem_euclid(12) + 1;
                    let first = days_from_civil(y, m, 1);
                    let last = days_in_month(y, m);
                    let days = if !self.monthdays.is_empty() {
                        self.monthdays
                            .iter()
                            .filter(|d| **d <= last)
                            .map(|d| first + d - 1)
                            .collect()
                    } else if !self.ordinal_days.is_empty() {
                        self.ordinal_days
                            .iter()
                            .filter_map(|(ordinal, dow)| {
                                let d = if *ordinal > 0 {
                                    1 + (dow - weekday(first)).rem_euclid(7) + (ordinal - 1) * 7
                                } else {
                                    last - (weekday(first + last - 1) - dow).rem_euclid(7)
                                        + (ordinal + 1) * 7
                                };
                                (1..=last).contains(&d).then_some(first + d - 1)
                            })
                            .collect()
                    } else if day <= last {
                        vec![first + day - 1]
                    } else {
                        Vec::new()
                    };
                    (first, days)
                }
                Frequency::Yearly => {
                    let y = year.checked_add(n)?;
                    let first = days_from_civil(y, month, 1);
                    (
                        first,
                        if day <= days_in_month(y, month) {
                            vec![first + day - 1]
                        } else {
                            Vec::new()
                        },
                    )
                }
            };
            if first > last_day {
                return Some(());
            }
            days.sort_unstable();
            days.dedup();
            for d in days {
                let wall = d.checked_mul(86400)?.checked_add(time)?;
                if wall <= start.wall {
                    continue;
                }
                if d > last_day {
                    return Some(());
                }
                if !visit(wall)? {
                    return Some(());
                }
                emitted += 1;
                if self.count.is_some_and(|count| emitted >= count) {
                    return Some(());
                }
            }
        }
        Some(())
    }
}
