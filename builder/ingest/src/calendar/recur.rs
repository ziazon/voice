//! RFC 5545 §3.3.10: a bounded subset, expanded in DTSTART's civil frame.
use super::ics::{Time, number};
use crate::dates::{civil_from_days, days_from_civil, days_in_month};
use std::collections::BTreeMap;

const MAX_STEPS: usize = 500_000;
// Count empty periods as well as candidate visits, so sparse rules are bounded too.
fn step(remaining: &mut usize, steps: &mut usize) -> Option<()> {
    if *steps == MAX_STEPS {
        return None;
    }
    *remaining = remaining.checked_sub(1)?;
    *steps += 1;
    Some(())
}
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
            // RFC 5545 §3.3.10 allows WKST on every frequency; only WEEKLY uses it.
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
            // RFC 5545 §3.3.10: negative month days count back from the month's end.
            match rule.frequency {
                Frequency::Monthly if rule.ordinal_days.is_empty() => {
                    rule.monthdays = days
                        .split(',')
                        .map(|d| {
                            if let Some(d) = d.strip_prefix('-') {
                                Some(-positive(d, 31)?)
                            } else {
                                positive(d.strip_prefix('+').unwrap_or(d), 31)
                            }
                        })
                        .collect::<Option<_>>()?;
                }
                Frequency::Yearly
                    if positive(days, 31)? == civil_from_days(start.wall.div_euclid(86400)).2 => {}
                _ => return None,
            }
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
        rule.ordinal_days.sort_unstable();
        rule.ordinal_days.dedup();
        Some(rule)
    }
    pub fn expand(
        &self,
        start: &Time,
        first_day: i64,
        last_day: i64,
        remaining: &mut usize,
        mut visit: impl FnMut(i64) -> Option<bool>,
    ) -> Option<()> {
        let mut steps = 0;
        step(remaining, &mut steps)?;
        // DTSTART counts first even if it does not satisfy the BY selectors (§3.3.10).
        if !visit(start.wall)? || self.count == Some(1) {
            return Some(());
        }
        let start_day = start.wall.div_euclid(86400);
        let time = start.wall.rem_euclid(86400);
        let (year, month, day) = civil_from_days(start_day);
        let week = start_day - (weekday(start_day) - self.week_start).rem_euclid(7);
        let mut emitted = 1;
        // Without COUNT, earlier periods cannot affect membership or the occurrence count.
        let target = first_day.saturating_sub(2);
        let (target_year, target_month, _) = civil_from_days(target);
        let period = if self.count.is_some() {
            0
        } else {
            match self.frequency {
                Frequency::Daily => (target - start_day).div_euclid(self.interval),
                Frequency::Weekly => (target - week).div_euclid(7 * self.interval),
                Frequency::Monthly => {
                    ((target_year - year) * 12 + target_month - month).div_euclid(self.interval)
                }
                Frequency::Yearly => (target_year - year).div_euclid(self.interval),
            }
            .max(0)
        };
        for period in period.. {
            step(remaining, &mut steps)?;
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
                            .map(|d| if *d < 0 { last + d + 1 } else { *d })
                            .filter(|d| (1..=last).contains(d))
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
                step(remaining, &mut steps)?;
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

#[cfg(test)]
mod tests {
    use super::super::ics::Frame;
    use super::*;

    #[test]
    fn per_rule_cap_counts_candidates_and_does_not_spend_other_rules_budget() {
        let start = Time {
            wall: 0,
            frame: Frame::Utc,
        };
        let rule = Rule::parse("FREQ=DAILY", &start).unwrap();
        let mut remaining = 5_000_000;
        assert_eq!(
            rule.expand(&start, 0, 600_000, &mut remaining, |_| Some(true)),
            None
        );
        assert_eq!(remaining, 4_500_000);
    }

    #[test]
    fn shared_budget_counts_weekday_candidates_not_just_periods() {
        let start = Time {
            wall: 1_793_620_800,
            frame: Frame::Utc,
        };
        let rule = Rule::parse(
            "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR,SA,SU;COUNT=100000",
            &start,
        )
        .unwrap();
        let mut visits = 0;
        assert_eq!(
            rule.expand(&start, 20_759, 30_000, &mut 30, |_| {
                visits += 1;
                Some(true)
            }),
            None
        );
        assert!(
            visits <= 30,
            "visited {visits} candidates on a 30-step budget"
        );
    }

    #[test]
    fn duplicate_ordinal_selectors_do_not_multiply_candidate_work() {
        let start = Time {
            wall: 1_793_620_800,
            frame: Frame::Utc,
        };
        let rule = Rule::parse(
            &format!("FREQ=MONTHLY;BYDAY={}", vec!["-1FR"; 10_000].join(",")),
            &start,
        )
        .unwrap();
        assert_eq!(rule.ordinal_days.len(), 1);
    }

    #[test]
    fn ancient_rules_skip_to_the_window_with_bounded_steps() {
        let start = Time {
            wall: -30_610_224_000,
            frame: Frame::Utc,
        }; // 1000-01-01
        let rule = Rule::parse("FREQ=DAILY", &start).unwrap();
        let mut remaining = 5_000_000;
        for _ in 0..2_000 {
            let before = remaining;
            let result = rule.expand(&start, 20_757, 20_775, &mut remaining, |_| Some(true));
            assert!(
                before - remaining < 50,
                "walk used {} steps",
                before - remaining
            );
            assert_eq!(result, Some(()));
        }
        assert!(5_000_000 - remaining < 100_000);
    }

    #[test]
    fn weekly_skip_ahead_preserves_interval_phase_and_dtstart() {
        let start = Time {
            wall: 631_188_000,
            frame: Frame::Utc,
        }; // 1990-01-01 10:00
        let mut full = Vec::new();
        let mut skipped = Vec::new();
        for (rule, first, out) in [
            (
                "FREQ=WEEKLY;INTERVAL=3;BYDAY=MO,WE;WKST=SU;COUNT=100000",
                0,
                &mut full,
            ),
            (
                "FREQ=WEEKLY;INTERVAL=3;BYDAY=MO,WE;WKST=SU",
                20_759,
                &mut skipped,
            ),
        ] {
            assert_eq!(
                Rule::parse(rule, &start).unwrap().expand(
                    &start,
                    first,
                    20_780,
                    &mut 5_000_000,
                    |wall| {
                        if wall == start.wall || wall.div_euclid(86400) >= 20_757 {
                            out.push(wall);
                        }
                        Some(true)
                    }
                ),
                Some(())
            );
        }
        assert_eq!(skipped, full);
        assert_eq!(skipped.first(), Some(&631_188_000));
    }

    #[test]
    fn expansion_refuses_a_spent_shared_budget_and_preserves_count_walks() {
        let start = Time {
            wall: 1_793_620_800,
            frame: Frame::Utc,
        };
        let rule = Rule::parse("FREQ=DAILY;COUNT=3", &start).unwrap();
        let mut remaining = 2;
        assert_eq!(
            rule.expand(&start, 20_759, 20_780, &mut remaining, |_| Some(true)),
            None
        );
        assert_eq!(remaining, 0);
        let mut called = false;
        assert_eq!(
            rule.expand(&start, 20_759, 20_780, &mut remaining, |_| {
                called = true;
                Some(true)
            }),
            None
        );
        assert!(!called);
    }
}
