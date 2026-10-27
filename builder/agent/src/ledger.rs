//! The session a build holds: one build per assistant, the empty work folder every call runs in,
//! the ledger of calls (`runs.jsonl`) and the daily cap.
//!
//! Every call is on the ledger before it starts, as a `started` line at its whole budget, and that
//! line is replaced by the call's own result when it ends. So a call in flight counts against the
//! cap, in this process and in any other, and a build that is killed mid-call (its `claude` child
//! runs on in its own process group) still has that call counted at its budget.
use crate::{claude::Usage, text};
use persona_builder::error::{Error, Result};
use persona_builder_ingest::private::{
    StateLock, lock_existing_state, lock_state, private_folder, read_private_optional,
    write_private,
};
use serde_json::{Map, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const LIMIT: &str = "limit.json";

const LEDGER: &str = "runs.jsonl";
const WORK_FOLDER: &str = "agent";
/// The largest typed daily or step limit, in estimated US dollars.
const MOST_USD: f64 = 1000.0;
/// The smallest step budget: Claude Code is given it to the cent.
const LEAST_USD: f64 = 0.01;
static NEXT_CALL: AtomicU64 = AtomicU64::new(0);

/// The daily cap and each call's budget, in US dollars as Claude Code estimates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cap {
    daily_usd: f64,
    call_usd: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    daily_usd: f64,
    call_usd: f64,
}

/// The daily limit is reached: no call may start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reached;

impl Cap {
    pub const DEFAULT_DAILY_USD: f64 = 50.0;
    pub const DEFAULT_CALL_USD: f64 = 5.0;

    /// Finite whole cents: each amount is 0 for no limit, or 0.01 through 1000.
    /// When both are set, the step limit cannot exceed the daily limit.
    pub fn new(daily_usd: f64, call_usd: f64) -> Result<Cap> {
        let valid = |amount: f64| {
            amount.is_finite()
                && (amount == 0.0 || (LEAST_USD..=MOST_USD).contains(&amount))
                && ((amount * 100.0).round() - amount * 100.0).abs() <= 1e-6
        };
        if !valid(daily_usd)
            || !valid(call_usd)
            || (daily_usd > 0.0 && call_usd > 0.0 && call_usd > daily_usd)
        {
            return Err(Error::Invalid("Each amount is 0 for no limit, or from $0.01 to $1,000 in whole cents, like 2.50. When both are set, the most one step may use can't be more than the daily limit.".into()));
        }
        Ok(Cap {
            daily_usd: if daily_usd == 0.0 { 0.0 } else { daily_usd },
            call_usd: if call_usd == 0.0 { 0.0 } else { call_usd },
        })
    }
    pub fn daily_limit(&self) -> Option<f64> {
        (self.daily_usd > 0.0).then_some(self.daily_usd)
    }
    pub fn call_limit(&self) -> Option<f64> {
        (self.call_usd > 0.0).then_some(self.call_usd)
    }
    /// The next call's budget, with calls still running counted at their budget.
    /// `Ok(Some(usd))` is passed as `--max-budget-usd`; `Ok(None)` passes no budget;
    /// `Err(Reached)` means the daily limit is reached.
    pub fn budget_for(&self, spent_usd: f64) -> std::result::Result<Option<f64>, Reached> {
        match (self.daily_limit(), self.call_limit()) {
            (Some(daily), Some(call)) => {
                if spent_usd + call > daily + 1e-9 {
                    Err(Reached)
                } else {
                    Ok(Some(call))
                }
            }
            (None, Some(call)) => Ok(Some(call)),
            (Some(daily), None) => {
                let left = ((daily - spent_usd) * 100.0 + 1e-6).floor() / 100.0;
                if left < LEAST_USD {
                    Err(Reached)
                } else {
                    Ok(Some(left))
                }
            }
            (None, None) => Ok(None),
        }
    }
    /// Read a saved limit without creating any state. Damaged values stop spending.
    pub fn load(home: &Path, slug: &str) -> Result<Cap> {
        let path = home.join(".claude/state").join(slug).join(LIMIT);
        let damaged = || Error::Invalid(text::limit_damaged(&path));
        let Some(bytes) = read_private_optional(home, slug, LIMIT)? else {
            return Ok(Cap::default());
        };
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|_| damaged())?;
        Cap::new(saved.daily_usd, saved.call_usd).map_err(|_| damaged())
    }
    /// Atomically save the limit as 0600. The caller must hold the state lock.
    pub fn save(&self, home: &Path, slug: &str) -> Result<()> {
        let saved = Saved {
            daily_usd: self.daily_usd,
            call_usd: self.call_usd,
        };
        let bytes = serde_json::to_vec(&saved).expect("a validated cap is finite");
        write_private(home, slug, LIMIT, &bytes)?;
        Ok(())
    }
    pub fn daily_usd(&self) -> f64 {
        self.daily_usd
    }
    pub fn call_usd(&self) -> f64 {
        self.call_usd
    }
}

impl Default for Cap {
    fn default() -> Self {
        Cap {
            daily_usd: Self::DEFAULT_DAILY_USD,
            call_usd: Self::DEFAULT_CALL_USD,
        }
    }
}

/// The clock a session reads the day from.
pub type Clock = Box<dyn Fn() -> SystemTime + Send + Sync>;

/// One build's hold on an assistant: every call goes through it.
pub struct Session {
    home: PathBuf,
    slug: String,
    _lock: StateLock,
    cap: Cap,
    clock: Clock,
    /// One ledger read-and-rewrite at a time.
    ledger: Mutex<()>,
}

/// One ledger line, before its time and id are stamped. Never any prompt or reply text.
pub(crate) struct Line<'a> {
    pub agent: &'a str,
    pub step: &'a str,
    pub model: &'a str,
    pub outcome: &'a str,
    pub plan: Option<&'a str>,
    pub seconds: f64,
    pub usage: Usage,
    pub cost_usd: f64,
    pub cost_from: &'a str,
}

/// A call on the ledger as `started`, at its whole budget, until [`Session::finish`] or
/// [`Session::discard`] replaces or removes its line. Dropped without either, the line stays.
pub(crate) struct Started {
    id: String,
    budget: Option<f64>,
}

impl Started {
    pub(crate) fn budget(&self) -> Option<f64> {
        self.budget
    }
}

impl Session {
    pub fn open(home: &Path, slug: &str, cap: Cap) -> Result<Session> {
        Self::open_with_clock(home, slug, cap, Box::new(SystemTime::now))
    }

    /// [`Session::open`], but only on a state folder that already exists: a missing one is an
    /// `Error::Io` whose source is `NotFound`, and nothing is created.
    pub fn open_existing(home: &Path, slug: &str, cap: Cap) -> Result<Session> {
        Self::open_at(
            home,
            slug,
            cap,
            Box::new(SystemTime::now),
            lock_existing_state,
        )
    }

    /// [`Session::open`] reading the day from `clock` (tests).
    pub fn open_with_clock(home: &Path, slug: &str, cap: Cap, clock: Clock) -> Result<Session> {
        Self::open_at(home, slug, cap, clock, lock_state)
    }

    fn open_at(
        home: &Path,
        slug: &str,
        cap: Cap,
        clock: Clock,
        lock: fn(&Path, &str) -> Result<StateLock>,
    ) -> Result<Session> {
        let lock = lock(home, slug)?;
        private_folder(home, slug, WORK_FOLDER)?;
        Ok(Session {
            home: home.to_owned(),
            slug: slug.to_owned(),
            _lock: lock,
            cap,
            clock,
            ledger: Mutex::new(()),
        })
    }

    pub fn cap(&self) -> Cap {
        self.cap
    }

    /// The estimated dollars spent today, calls still running counted at their whole budget.
    pub fn spent_today(&self) -> Result<f64> {
        let _held = self.held();
        self.recorded_today()
    }

    /// The work folder, checked again before each call: a real 0700 folder, and empty, so no
    /// settings, hooks, MCP servers or instructions planted there reach a call.
    pub(crate) fn work_folder(&self) -> Result<PathBuf> {
        let folder = private_folder(&self.home, &self.slug, WORK_FOLDER)?;
        let mut entries = std::fs::read_dir(&folder).map_err(|source| Error::Io {
            path: folder.clone(),
            source,
        })?;
        if entries.next().is_some() {
            return Err(Error::Invalid(text::work_folder_not_empty(&folder)));
        }
        Ok(folder)
    }

    /// Put a call on the ledger at its whole budget, or refuse when today's spend (calls still
    /// running included) and this call could pass the daily cap.
    pub(crate) fn start(&self, agent: &str, step: &str, model: &str) -> Result<Started> {
        let _held = self.held();
        let (spent, stopped) = self.today()?;
        if stopped {
            return Err(Error::Invalid(text::PLAN_STOPPED.into()));
        }
        let budget = self
            .cap
            .budget_for(spent)
            .map_err(|Reached| Error::Invalid(text::cap_reached(spent, self.cap.daily_usd)))?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = format!(
            "{}-{nanos}-{}",
            std::process::id(),
            NEXT_CALL.fetch_add(1, Ordering::Relaxed)
        );
        let line = Line {
            agent,
            step,
            model,
            outcome: "started",
            plan: None,
            seconds: 0.0,
            usage: Usage::default(),
            cost_usd: budget.unwrap_or(0.0),
            cost_from: "budget",
        };
        let mut lines = self.ledger_lines()?;
        lines.push(self.render(&id, &line));
        self.write(&lines)?;
        Ok(Started { id, budget })
    }

    /// Replace the call's `started` line with its result.
    pub(crate) fn finish(&self, started: Started, line: &Line) -> Result<()> {
        let _held = self.held();
        let rendered = self.render(&started.id, line);
        let mut lines = self.ledger_lines()?;
        match lines
            .iter()
            .position(|raw| line_id(raw).as_deref() == Some(started.id.as_str()))
        {
            Some(at) => lines[at] = rendered,
            None => lines.push(rendered),
        }
        self.write(&lines)
    }

    /// Remove the line of a call that never started: it spent nothing.
    pub(crate) fn discard(&self, started: Started) -> Result<()> {
        let _held = self.held();
        let mut lines = self.ledger_lines()?;
        lines.retain(|raw| line_id(raw).as_deref() != Some(started.id.as_str()));
        self.write(&lines)
    }

    fn render(&self, id: &str, line: &Line) -> String {
        let now = (self.clock)();
        let mut object = Map::new();
        object.insert("at".into(), Value::from(utc_stamp(now)));
        object.insert("id".into(), Value::from(id));
        object.insert("day".into(), Value::from(local_day(now)));
        object.insert("agent".into(), Value::from(line.agent));
        object.insert("step".into(), Value::from(line.step));
        object.insert("model".into(), Value::from(line.model));
        object.insert("outcome".into(), Value::from(line.outcome));
        if let Some(plan) = line.plan {
            object.insert("plan".into(), Value::from(plan));
        }
        object.insert(
            "seconds".into(),
            Value::from((line.seconds * 10.0).round() / 10.0),
        );
        let usage = line.usage;
        object.insert("input_tokens".into(), Value::from(usage.input_tokens));
        object.insert("output_tokens".into(), Value::from(usage.output_tokens));
        object.insert(
            "cache_creation_input_tokens".into(),
            Value::from(usage.cache_creation_input_tokens),
        );
        object.insert(
            "cache_read_input_tokens".into(),
            Value::from(usage.cache_read_input_tokens),
        );
        object.insert("cost_usd".into(), Value::from(line.cost_usd));
        object.insert("cost_from".into(), Value::from(line.cost_from));
        Value::Object(object).to_string()
    }

    fn write(&self, lines: &[String]) -> Result<()> {
        let mut text = lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        write_private(&self.home, &self.slug, LEDGER, text.as_bytes())?;
        Ok(())
    }

    fn held(&self) -> MutexGuard<'_, ()> {
        self.ledger
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn ledger_path(&self) -> PathBuf {
        self.home
            .join(".claude/state")
            .join(&self.slug)
            .join(LEDGER)
    }

    fn damaged(&self) -> Error {
        Error::Invalid(text::ledger_damaged(&self.ledger_path()))
    }

    /// The ledger's lines, each checked to be a ledger line; any other line stops all spending.
    fn ledger_lines(&self) -> Result<Vec<String>> {
        let Some(bytes) = read_private_optional(&self.home, &self.slug, LEDGER)? else {
            return Ok(Vec::new());
        };
        let text = String::from_utf8(bytes).map_err(|_| self.damaged())?;
        let text = text.strip_suffix('\n').unwrap_or(&text);
        if text.is_empty() {
            return Ok(Vec::new());
        }
        text.split('\n')
            .map(|raw| {
                parse_line(raw)
                    .map(|_| raw.to_owned())
                    .ok_or_else(|| self.damaged())
            })
            .collect()
    }

    /// The sum of today's `cost_usd`.
    fn recorded_today(&self) -> Result<f64> {
        Ok(self.today()?.0)
    }

    /// Today's spend and plan stop use the same local calendar day.
    fn today(&self) -> Result<(f64, bool)> {
        let today = local_day((self.clock)());
        let mut spent = 0.0;
        let mut stopped = false;
        for raw in self.ledger_lines()? {
            let (day, cost, plan_stopped) = parse_line(&raw).ok_or_else(|| self.damaged())?;
            if day == today {
                spent += cost;
                stopped |= plan_stopped;
            }
        }
        Ok((spent, stopped))
    }
}

/// A ledger line's day, cost and plan stop, or `None` when it is not a ledger line.
fn parse_line(raw: &str) -> Option<(String, f64, bool)> {
    let line: Value = serde_json::from_str(raw).ok()?;
    let day = line.get("day")?.as_str()?.to_owned();
    let cost = line
        .get("cost_usd")?
        .as_f64()
        .filter(|cost| cost.is_finite() && *cost >= 0.0)?;
    let stopped = match line.get("plan") {
        None => false,
        Some(Value::String(plan)) if matches!(plan.as_str(), "allowed_warning" | "rejected") => {
            true
        }
        _ => return None,
    };
    Some((day, cost, stopped))
}

fn line_id(raw: &str) -> Option<String> {
    serde_json::from_str::<Value>(raw)
        .ok()?
        .get("id")?
        .as_str()
        .map(str::to_owned)
}

fn seconds(time: SystemTime) -> libc::time_t {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    libc::time_t::try_from(seconds).unwrap_or(libc::time_t::MAX)
}

/// The person's local calendar day, `YYYY-MM-DD`.
pub fn local_day(time: SystemTime) -> String {
    let when = seconds(time);
    // SAFETY: tm is plain data; all-zero is valid and localtime_r fills it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: tzset reads the time zone setting; localtime_r gets valid pointers to live values.
    let filled = unsafe {
        tzset();
        !libc::localtime_r(&when, &mut tm).is_null()
    };
    if !filled {
        return utc_stamp(time)[..10].to_owned();
    }
    format!(
        "{:04}-{:02}-{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday
    )
}

unsafe extern "C" {
    fn tzset();
}

/// UTC to the second, `YYYY-MM-DDTHH:MM:SSZ`.
fn utc_stamp(time: SystemTime) -> String {
    let when = seconds(time);
    // SAFETY: tm is plain data; all-zero is valid and gmtime_r fills it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are to live values for the duration of the call.
    unsafe { libc::gmtime_r(&when, &mut tm) };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}
