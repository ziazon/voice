//! The session a build holds: one build per assistant, the empty work folder every call runs in,
//! the ledger of calls (`runs.jsonl`) and the daily cap.
use crate::text;
use persona_builder::error::{Error, Result};
use persona_builder_ingest::private::{
    StateLock, lock_state, private_folder, read_private_optional, write_private,
};
use serde_json::{Map, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

const LEDGER: &str = "runs.jsonl";
const WORK_FOLDER: &str = "agent";
/// The largest cap Persona Builder accepts, in estimated US dollars a day.
const MOST_USD: f64 = 1000.0;

/// The daily cap and each call's budget, in US dollars as Claude Code estimates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cap {
    daily_usd: f64,
    call_usd: f64,
}

impl Cap {
    pub const DEFAULT_DAILY_USD: f64 = 50.0;
    pub const DEFAULT_CALL_USD: f64 = 5.0;

    /// Both finite, and 0 < `call_usd` <= `daily_usd` <= 1000.
    pub fn new(daily_usd: f64, call_usd: f64) -> Result<Cap> {
        if !(daily_usd.is_finite()
            && call_usd.is_finite()
            && call_usd > 0.0
            && call_usd <= daily_usd
            && daily_usd <= MOST_USD)
        {
            return Err(Error::Invalid(format!(
                "the daily limit must be more than 0 and at most ${MOST_USD:.0}, and each step's limit more than 0 and no more than the daily limit"
            )));
        }
        Ok(Cap {
            daily_usd,
            call_usd,
        })
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

#[derive(Default)]
struct Held {
    /// The budgets of this session's calls in flight.
    reserved: f64,
    /// What this session's calls may have spent that the ledger failed to record.
    unrecorded: f64,
}

/// One build's hold on an assistant: every call goes through it.
pub struct Session {
    home: PathBuf,
    slug: String,
    _lock: StateLock,
    cap: Cap,
    clock: Clock,
    held: Mutex<Held>,
}

/// One ledger line, before its time is stamped. Never any prompt or reply text.
pub(crate) struct Line<'a> {
    pub agent: &'a str,
    pub step: &'a str,
    pub model: &'a str,
    pub outcome: &'a str,
    pub seconds: f64,
    pub usage: crate::claude::Usage,
    pub cost_usd: f64,
    pub cost_from: &'a str,
}

/// A call's budget, held against the cap until the call is recorded (or released on drop).
pub(crate) struct Reservation<'a> {
    session: &'a Session,
    budget: f64,
    open: bool,
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if self.open {
            self.session.held().reserved -= self.budget;
        }
    }
}

impl Session {
    pub fn open(home: &Path, slug: &str, cap: Cap) -> Result<Session> {
        Self::open_with_clock(home, slug, cap, Box::new(SystemTime::now))
    }

    /// [`Session::open`] reading the day from `clock` (tests).
    pub fn open_with_clock(home: &Path, slug: &str, cap: Cap, clock: Clock) -> Result<Session> {
        let lock = lock_state(home, slug)?;
        private_folder(home, slug, WORK_FOLDER)?;
        Ok(Session {
            home: home.to_owned(),
            slug: slug.to_owned(),
            _lock: lock,
            cap,
            clock,
            held: Mutex::new(Held::default()),
        })
    }

    pub fn cap(&self) -> Cap {
        self.cap
    }

    /// The estimated dollars spent today: the ledger's lines for today, plus anything this
    /// session spent that the ledger failed to record.
    pub fn spent_today(&self) -> Result<f64> {
        let held = self.held();
        Ok(self.recorded_today()? + held.unrecorded)
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

    /// Hold one call's budget, or refuse when today's spend, the calls in flight and this call
    /// could pass the daily cap.
    pub(crate) fn reserve(&self) -> Result<Reservation<'_>> {
        let mut held = self.held();
        let spent = self.recorded_today()? + held.unrecorded;
        let budget = self.cap.call_usd;
        if spent + held.reserved + budget > self.cap.daily_usd + 1e-9 {
            return Err(Error::Invalid(text::cap_reached(spent, self.cap.daily_usd)));
        }
        held.reserved += budget;
        Ok(Reservation {
            session: self,
            budget,
            open: true,
        })
    }

    pub(crate) fn budget(reservation: &Reservation) -> f64 {
        reservation.budget
    }

    /// Append the call's line and release its budget. When the ledger cannot be written, the
    /// whole budget stays counted for this session, since the money may have been spent.
    pub(crate) fn record(&self, mut reservation: Reservation, line: &Line) -> Result<()> {
        let mut held = self.held();
        reservation.open = false;
        held.reserved -= reservation.budget;
        let written = self.append(line);
        if written.is_err() {
            held.unrecorded += reservation.budget;
        }
        written
    }

    fn append(&self, line: &Line) -> Result<()> {
        let now = (self.clock)();
        let mut object = Map::new();
        object.insert("at".into(), Value::from(utc_stamp(now)));
        object.insert("day".into(), Value::from(local_day(now)));
        object.insert("agent".into(), Value::from(line.agent));
        object.insert("step".into(), Value::from(line.step));
        object.insert("model".into(), Value::from(line.model));
        object.insert("outcome".into(), Value::from(line.outcome));
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
        let mut bytes = self.ledger_bytes()?.unwrap_or_default();
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(Value::Object(object).to_string().as_bytes());
        bytes.push(b'\n');
        write_private(&self.home, &self.slug, LEDGER, &bytes)?;
        Ok(())
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn ledger_path(&self) -> PathBuf {
        self.home
            .join(".claude/state")
            .join(&self.slug)
            .join(LEDGER)
    }

    fn ledger_bytes(&self) -> Result<Option<Vec<u8>>> {
        read_private_optional(&self.home, &self.slug, LEDGER)
    }

    /// The sum of today's `cost_usd`; any line that is not a ledger line stops all spending.
    fn recorded_today(&self) -> Result<f64> {
        let Some(bytes) = self.ledger_bytes()? else {
            return Ok(0.0);
        };
        let damaged = || Error::Invalid(text::ledger_damaged(&self.ledger_path()));
        let text = std::str::from_utf8(&bytes).map_err(|_| damaged())?;
        let text = text.strip_suffix('\n').unwrap_or(text);
        let today = local_day((self.clock)());
        let mut spent = 0.0;
        if text.is_empty() {
            return Ok(spent);
        }
        for raw in text.split('\n') {
            let line: Value = serde_json::from_str(raw).map_err(|_| damaged())?;
            let day = line
                .get("day")
                .and_then(Value::as_str)
                .ok_or_else(damaged)?;
            let cost = line
                .get("cost_usd")
                .and_then(Value::as_f64)
                .filter(|cost| cost.is_finite() && *cost >= 0.0)
                .ok_or_else(damaged)?;
            if day == today {
                spent += cost;
            }
        }
        Ok(spent)
    }
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
