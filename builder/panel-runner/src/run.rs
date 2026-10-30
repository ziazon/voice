//! Convene, resume and read a panel.
use crate::{
    room::Room,
    seats::Seat,
    turns::{TurnUsage, Turns},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};
use voice_panels::{DeliberationState, RoundReport};

/// The question and the assistants invited to discuss it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Convene {
    pub question: String,
    pub background: String,
    pub kind: String,
    pub seats: Vec<String>,
    pub time_box: Option<String>,
}

/// A request to continue an existing panel.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resume {
    pub id: String,
    pub steer: Option<String>,
    pub time_box: Option<String>,
}

/// A saved panel's question and reopen context.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Convened {
    #[serde(flatten)]
    pub input: Convene,
    pub created_at: String,
    #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
    pub where_left_off: Option<String>,
    #[serde(default)]
    pub box_offset_s: f64,
}

/// Progress after starting or finishing a round.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    RoundStarted {
        round: u32,
    },
    RoundDone {
        round: u32,
        d: f64,
        withheld: Vec<String>,
        failed: Vec<String>,
    },
}

/// The saved outcome of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ran {
    pub id: String,
    pub status: String,
    pub outcome: Option<String>,
}

/// The wall and elapsed clocks used by a panel.
pub trait Clock {
    /// UTC time for saved records.
    fn now_iso(&self) -> String;
    /// Monotonic elapsed seconds for the time box.
    fn seconds(&self) -> f64;
}

/// The system clock for a real panel.
pub struct SystemClock(std::time::Instant);
impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}
impl SystemClock {
    /// Start an elapsed clock.
    pub fn new() -> Self {
        Self(std::time::Instant::now())
    }
}
impl Clock for SystemClock {
    fn now_iso(&self) -> String {
        persona_builder_agent::ledger::utc_stamp(std::time::SystemTime::now())
    }
    fn seconds(&self) -> f64 {
        self.0.elapsed().as_secs_f64()
    }
}

/// A panel's saved status, read without taking its lock.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub status: String,
    pub round: u32,
    pub outcome: Option<String>,
    pub updated_at: String,
}

/// A panel in the newest-first list.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub id: String,
    pub question: String,
    #[serde(flatten)]
    pub status: Status,
}

/// A saved round with its timings.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedRound {
    #[serde(flatten)]
    pub report: RoundReport,
    pub duration_s: f64,
    pub elapsed_s: f64,
}

/// Withheld categories and failed assistants in a saved round.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoundDetail {
    pub round: u32,
    pub d: f64,
    pub withheld: BTreeMap<String, Vec<String>>,
    pub failed: Vec<String>,
}

/// Read-only detail for the panel screen.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    pub summary: Summary,
    pub convene: Convened,
    pub rounds: Vec<RoundDetail>,
    pub asked: bool,
    pub result: Option<serde_json::Value>,
    pub markdown: Option<String>,
}

/// Per-assistant totals in a closed result.
pub type Usage = BTreeMap<String, TurnUsage>;

/// A post or a direction kept in the private transcript.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum Entry {
    Post {
        round: u32,
        seat: String,
        text: String,
        block: Option<Box<voice_panels::Block>>,
        malformed: bool,
        error: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        withheld: Vec<String>,
        usage: Option<TurnUsage>,
        at: String,
    },
    Steer {
        round: u32,
        author: String,
        text: String,
        at: String,
    },
}
impl Entry {
    fn round(&self) -> u32 {
        match self {
            Self::Post { round, .. } | Self::Steer { round, .. } => *round,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ask {
    asked_at: String,
    question: String,
    options: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    time_box: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedTrajectory {
    #[serde(rename = "D")]
    d: Vec<f64>,
    rounds: u32,
    elapsed_s: f64,
    usage: Usage,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedResult {
    id: String,
    label: String,
    outcome: String,
    kind: String,
    answer: Option<voice_panels::Answer>,
    groups: Vec<voice_panels::Group>,
    dissent: Vec<voice_panels::Dissent>,
    trajectory: SavedTrajectory,
}

/// Validate and start a panel with the supplied model and clock.
#[allow(clippy::too_many_arguments)]
pub fn convene(
    home: &Path,
    request: &Convene,
    consented: &dyn Fn(&Path, &str) -> bool,
    turns: &dyn Turns,
    clock: &dyn Clock,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Ran, String> {
    convene_named(
        home,
        &crate::room::new_id()?,
        request,
        consented,
        turns,
        clock,
        stop,
        progress,
    )
}

/// Start a panel with a name reserved by the app's running registry.
#[allow(clippy::too_many_arguments)]
pub fn convene_named(
    home: &Path,
    id: &str,
    request: &Convene,
    consented: &dyn Fn(&Path, &str) -> bool,
    turns: &dyn Turns,
    clock: &dyn Clock,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Ran, String> {
    let (kind, n) = validate(request)?;
    let seats = crate::seats::load_for(home, &request.seats, consented, turns)?;
    let mut input = request.clone();
    input.question = voice_panels::clean(&crate::guard::scrub(&input.question), 200);
    input.background = crate::guard::scrub(&input.background);
    let metadata = Convened {
        input,
        created_at: clock.now_iso(),
        where_left_off: None,
        box_offset_s: 0.0,
    };
    let mut state = DeliberationState::new(kind, n, request.seats.clone())
        .map_err(|_| crate::text::BAD_SEATS)?;
    let room = Room::open(home, id, true)?;
    let _lock = room.lock()?;
    room.write_json("convene.json", &metadata)?;
    room.write_json("state.json", &state.to_json())?;
    room.write("rounds.jsonl", b"")?;
    room.write("transcript.jsonl", b"")?;
    save_status(&room, &state, "running", None, clock)?;
    run(
        &room, &mut state, &metadata, &seats, turns, clock, stop, progress,
    )
}

/// Check the question, answer format and time limit before starting model calls.
pub fn validate(request: &Convene) -> Result<(voice_panels::Kind, Option<i64>), String> {
    let kind = if request.kind == "text" {
        (voice_panels::Kind::Text, None)
    } else {
        let n = request
            .kind
            .strip_prefix("list:")
            .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|n| (1..=12).contains(n))
            .ok_or(crate::text::BAD_KIND)?;
        (voice_panels::Kind::List, Some(n))
    };
    let question = voice_panels::clean(&request.question, usize::MAX);
    if question.is_empty() || question.chars().count() > 200 {
        return Err(crate::text::BAD_QUESTION.into());
    }
    if request.background.len() > 4096 {
        return Err(crate::text::BAD_BACKGROUND.into());
    }
    crate::guard::guard_input(&request.question)?;
    crate::guard::guard_input(&request.background)?;
    parse_time(request.time_box.as_deref())?;
    Ok(kind)
}
fn parse_time(word: Option<&str>) -> Result<Option<voice_panels::TimeBox>, String> {
    word.map(voice_panels::TimeBox::parse)
        .transpose()
        .map_err(|_| crate::text::BAD_TIME.into())
}
fn answer(room: &Room) -> Result<Option<String>, String> {
    if !room.exists("answer.json")? {
        return Ok(None);
    }
    let saved: Answer = room.read_json("answer.json")?;
    parse_time(Some(&saved.time_box))?;
    Ok(Some(saved.time_box))
}
fn asked(room: &Room) -> Result<bool, String> {
    if !room.exists("ask.json")? {
        return Ok(false);
    }
    let _: Ask = room.read_json("ask.json")?;
    Ok(true)
}

/// Validate and continue a panel, recovering only completed rounds.
#[allow(clippy::too_many_arguments)]
pub fn resume(
    home: &Path,
    request: &Resume,
    consented: &dyn Fn(&Path, &str) -> bool,
    turns: &dyn Turns,
    clock: &dyn Clock,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Ran, String> {
    let room = Room::open(home, &request.id, false)?;
    read_snapshot(&room)?;
    let _lock = room.lock()?;
    let (mut metadata, mut state, saved) = read_snapshot(&room)?;
    let seats = crate::seats::load_for(home, &metadata.input.seats, consented, turns)?;
    if saved.status == "closed" && request.steer.is_none() {
        return Err(crate::text::NEED_STEER.into());
    }
    let steer = request
        .steer
        .as_deref()
        .map(|s| {
            crate::guard::guard_input(s)?;
            Ok::<_, String>(voice_panels::clean(&crate::guard::scrub(s), 8000))
        })
        .transpose()?;
    let waiting = asked(&room)? && answer(&room)?.is_none();
    if let Some(time) = &request.time_box {
        parse_time(Some(time))?;
        if !waiting {
            return Err(crate::text::NO_ASK.into());
        }
    } else if saved.status == "paused" && waiting {
        return Err(crate::text::ANSWER_FIRST.into());
    }
    let mut reports: Vec<TimedRound> = room.lines("rounds.jsonl")?;
    let mut entries: Vec<Entry> = room.lines("transcript.jsonl")?;
    reports.retain(|r| r.report.round <= state.round_no);
    entries.retain(|entry| {
        entry.round() <= state.round_no
            || (saved.status != "running"
                && matches!(entry, Entry::Steer { .. })
                && state.round_no.checked_add(1) == Some(entry.round()))
    });
    if let Some(time) = &request.time_box {
        room.write_json(
            "answer.json",
            &Answer {
                time_box: time.clone(),
            },
        )?;
    }
    room.write_lines("rounds.jsonl", &reports)?;
    room.write_lines("transcript.jsonl", &entries)?;
    if saved.status != "closed"
        && let Some(last) = reports.last()
    {
        let effective = answer(&room)?.or_else(|| metadata.input.time_box.clone());
        let decision = voice_panels::timebox_decision(
            last.elapsed_s - metadata.box_offset_s,
            &reports.iter().map(|r| r.duration_s).collect::<Vec<_>>(),
            &state.d_history,
            parse_time(effective.as_deref())?,
            asked(&room)?,
            effective.is_some(),
        );
        if state.closed.is_some() || decision.expired {
            let ran = close(
                &room,
                &state,
                &last.report,
                last.elapsed_s,
                decision.expired && state.closed.is_none(),
                clock,
            )?;
            if steer.is_none() {
                return Ok(ran);
            }
        }
    }
    if saved.status == "closed"
        || state.closed.is_some()
        || room.read_json::<Status>("status.json")?.status == "closed"
    {
        reopen_room(&room, &mut state, &mut metadata)?;
    }
    if let Some(text) = steer {
        entries.push(Entry::Steer {
            round: state
                .round_no
                .checked_add(1)
                .ok_or(crate::text::READ_STATE)?,
            author: "convener".into(),
            text,
            at: clock.now_iso(),
        });
        room.write_lines("transcript.jsonl", &entries)?;
    }
    save_status(&room, &state, "running", None, clock)?;
    run(
        &room, &mut state, &metadata, &seats, turns, clock, stop, progress,
    )
}

fn read_snapshot(room: &Room) -> Result<(Convened, DeliberationState, Status), String> {
    let metadata: Convened = room.read_json("convene.json")?;
    let state = room.load_state(&metadata)?;
    let saved = read_status(room)?;
    Ok((metadata, state, saved))
}
fn read_status(room: &Room) -> Result<Status, String> {
    let saved: Status = room.read_json("status.json")?;
    if !["closed", "paused", "stopped", "failed", "running"].contains(&saved.status.as_str()) {
        return Err(crate::text::READ_STATE.into());
    }
    Ok(saved)
}
fn reopen_room(
    room: &Room,
    state: &mut DeliberationState,
    metadata: &mut Convened,
) -> Result<(), String> {
    let result: SavedResult = room.read_json("result.json")?;
    metadata.where_left_off =
        Some(String::from_utf8(room.read("result.md")?).map_err(|_| crate::text::READ_STATE)?);
    metadata.box_offset_s = result.trajectory.elapsed_s;
    voice_panels::reopen(state);
    if answer(room)?.is_none() {
        room.delete("ask.json")?;
    }
    room.write_json("convene.json", metadata)?;
    room.write_json("state.json", &state.to_json())?;
    Ok(())
}

fn save_status(
    room: &Room,
    state: &DeliberationState,
    status: &str,
    outcome: Option<String>,
    clock: &dyn Clock,
) -> Result<Ran, String> {
    room.write_json(
        "status.json",
        &Status {
            status: status.into(),
            round: state.round_no,
            outcome: outcome.clone(),
            updated_at: clock.now_iso(),
        },
    )?;
    Ok(Ran {
        id: room.id.clone(),
        status: status.into(),
        outcome,
    })
}

/// Run rounds while the caller holds the room and assistants.
#[allow(clippy::too_many_arguments)]
pub fn run(
    room: &Room,
    state: &mut DeliberationState,
    convene: &Convened,
    seats: &[Seat],
    turns: &dyn Turns,
    clock: &dyn Clock,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Ran, String> {
    let result = run_rounds(room, state, convene, seats, turns, clock, stop, progress);
    if result.is_err() {
        save_status(room, state, "failed", None, clock)?;
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_rounds(
    room: &Room,
    state: &mut DeliberationState,
    convene: &Convened,
    seats: &[Seat],
    turns: &dyn Turns,
    clock: &dyn Clock,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Ran, String> {
    let mut reports: Vec<TimedRound> = room.lines("rounds.jsonl")?;
    let mut entries: Vec<Entry> = room.lines("transcript.jsonl")?;
    let elapsed_before = reports.last().map_or(0.0, |r| r.elapsed_s);
    let start = clock.seconds() - elapsed_before;
    loop {
        let round = state
            .round_no
            .checked_add(1)
            .ok_or(crate::text::READ_STATE)?;
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            return save_status(room, state, "stopped", None, clock);
        }
        progress(Progress::RoundStarted { round });
        let briefs = round_briefs(&entries, state, convene, seats, &reports);
        let turn_start = clock.seconds();
        let posts = parallel_turns(seats, &briefs, turns);
        let duration = clock.seconds() - turn_start;
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            return save_status(room, state, "stopped", None, clock);
        }
        let kept = keep_posts(room, state, seats, posts, round, clock)?;
        entries.extend(kept.entries);
        room.write_lines("transcript.jsonl", &entries)?;
        if kept.failed.len() == seats.len() {
            return save_status(room, state, "failed", None, clock);
        }
        let report =
            voice_panels::step_round(state, &kept.fresh).map_err(|_| crate::text::READ_STATE)?;
        let elapsed = clock.seconds() - start;
        reports.push(TimedRound {
            report: report.clone(),
            duration_s: duration,
            elapsed_s: elapsed,
        });
        room.write_lines("rounds.jsonl", &reports)?;
        room.write_json("state.json", &state.to_json())?;
        save_status(room, state, "running", None, clock)?;
        progress(Progress::RoundDone {
            round,
            d: report.d,
            withheld: kept.flagged,
            failed: kept.failed,
        });
        if state.closed.is_some() {
            return close(room, state, &report, elapsed, false, clock);
        }
        if let Some(ran) = timebox(room, state, convene, &reports, &report, elapsed, clock)? {
            return Ok(ran);
        }
    }
}

fn timebox(
    room: &Room,
    state: &DeliberationState,
    convene: &Convened,
    reports: &[TimedRound],
    report: &RoundReport,
    elapsed: f64,
    clock: &dyn Clock,
) -> Result<Option<Ran>, String> {
    let answer = answer(room)?;
    let effective = answer.as_deref().or(convene.input.time_box.as_deref());
    let decision = voice_panels::timebox_decision(
        elapsed - convene.box_offset_s,
        &reports.iter().map(|r| r.duration_s).collect::<Vec<_>>(),
        &state.d_history,
        parse_time(effective)?,
        asked(room)?,
        answer.is_some(),
    );
    if decision.ask {
        room.write_json(
            "ask.json",
            &Ask {
                asked_at: clock.now_iso(),
                question: crate::text::TIME_QUESTION.into(),
                options: vec!["15m".into(), "1h".into(), "deadlock".into()],
            },
        )?;
    }
    if decision.pause {
        return save_status(room, state, "paused", None, clock).map(Some);
    }
    if decision.expired {
        return close(room, state, report, elapsed, true, clock).map(Some);
    }
    Ok(None)
}

#[derive(Default)]
struct KeptPosts {
    entries: Vec<Entry>,
    fresh: BTreeMap<String, voice_panels::Block>,
    flagged: Vec<String>,
    failed: Vec<String>,
}
fn keep_posts(
    room: &Room,
    state: &DeliberationState,
    seats: &[Seat],
    posts: Vec<crate::Turn>,
    round: u32,
    clock: &dyn Clock,
) -> Result<KeptPosts, String> {
    let mut kept = KeptPosts::default();
    let filters: Vec<_> = seats.iter().map(|s| &s.filters).collect();
    for (seat, turn) in seats.iter().zip(posts) {
        let name = &seat.member.slug;
        let text = turn.post.unwrap_or_default();
        let block = voice_panels::parse_block(
            &text,
            state.kind,
            state.n,
            round,
            name,
            &state.crew,
            &state.drafts.keys().cloned().collect(),
        );
        let findings = crate::guard::post_findings_refs(&text, block.as_ref(), &filters);
        if turn.error.is_some() {
            kept.failed.push(name.clone());
        }
        if !findings.is_empty() {
            kept.flagged.push(name.clone());
            room.write_json(
                &format!("withheld/r{round}-{name}.json"),
                &serde_json::json!({"round":round,"seat":name,"text":text}),
            )?;
        } else if let Some(block) = &block {
            kept.fresh.insert(name.clone(), block.clone());
        }
        kept.entries.push(Entry::Post {
            round,
            seat: name.clone(),
            text: if findings.is_empty() {
                text
            } else {
                String::new()
            },
            malformed: findings.is_empty() && block.is_none(),
            block: if findings.is_empty() {
                block.map(Box::new)
            } else {
                None
            },
            error: turn.error,
            withheld: findings.iter().map(|s| (*s).into()).collect(),
            usage: turn.usage,
            at: clock.now_iso(),
        });
    }
    Ok(kept)
}

fn round_briefs(
    entries: &[Entry],
    state: &DeliberationState,
    convene: &Convened,
    seats: &[Seat],
    reports: &[TimedRound],
) -> Vec<(String, String)> {
    let mut previous = BTreeMap::new();
    let mut withheld = BTreeMap::new();
    let mut steers = Vec::new();
    for entry in entries {
        match entry {
            Entry::Post {
                round,
                seat,
                text,
                withheld: findings,
                ..
            } if *round == state.round_no => {
                previous.insert(seat.clone(), text.clone());
                if !findings.is_empty() {
                    withheld.insert(seat.clone(), findings.clone());
                }
            }
            Entry::Steer { round: r, text, .. } if state.round_no.checked_add(1) == Some(*r) => {
                steers.push(text.clone())
            }
            _ => {}
        }
    }
    let old_reports: Vec<_> = reports.iter().map(|r| r.report.clone()).collect();
    seats
        .iter()
        .map(|s| {
            crate::brief::make_brief(
                &s.member.slug,
                &s.member,
                &s.decisions,
                state,
                convene,
                &previous,
                &steers,
                &old_reports,
                &withheld,
            )
        })
        .collect()
}

fn parallel_turns(
    seats: &[Seat],
    briefs: &[(String, String)],
    turns: &dyn Turns,
) -> Vec<crate::Turn> {
    let mut posts = Vec::new();
    for (group, prompts) in seats.chunks(6).zip(briefs.chunks(6)) {
        let batch = std::thread::scope(|scope| {
            let handles: Vec<_> = group
                .iter()
                .zip(prompts)
                .map(|(seat, (system, prompt))| {
                    scope.spawn(move || turns.turn(&seat.member.slug, system, prompt))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join().unwrap_or_else(|_| crate::Turn {
                        error: Some(crate::text::FAILED.into()),
                        ..crate::Turn::default()
                    })
                })
                .collect::<Vec<_>>()
        });
        posts.extend(batch);
    }
    posts
}

fn close(
    room: &Room,
    state: &DeliberationState,
    report: &RoundReport,
    elapsed: f64,
    expired: bool,
    clock: &dyn Clock,
) -> Result<Ran, String> {
    let mut result =
        voice_panels::build_result(state, report).map_err(|_| crate::text::READ_STATE)?;
    if expired {
        result.outcome = "time_box".into();
    }
    let markdown = voice_panels::render_markdown(&result).map_err(|_| crate::text::READ_STATE)?;
    let mut usage: Usage = state
        .crew
        .iter()
        .map(|n| (n.clone(), TurnUsage::default()))
        .collect();
    for entry in room.lines::<Entry>("transcript.jsonl")? {
        if let Entry::Post {
            seat,
            usage: Some(part),
            ..
        } = entry
            && let Some(total) = usage.get_mut(&seat)
        {
            total.input_tokens = total.input_tokens.saturating_add(part.input_tokens);
            total.output_tokens = total.output_tokens.saturating_add(part.output_tokens);
            total.cache_creation_input_tokens = total
                .cache_creation_input_tokens
                .saturating_add(part.cache_creation_input_tokens);
            total.cache_read_input_tokens = total
                .cache_read_input_tokens
                .saturating_add(part.cache_read_input_tokens);
            total.cost_usd += part.cost_usd;
        }
    }
    let saved = SavedResult {
        id: room.id.clone(),
        label: result.label,
        outcome: result.outcome.clone(),
        kind: result.kind,
        answer: result.answer,
        groups: result.groups,
        dissent: result.dissent,
        trajectory: SavedTrajectory {
            d: result.trajectory.d,
            rounds: result.trajectory.rounds,
            elapsed_s: elapsed,
            usage,
        },
    };
    room.write_json("result.json", &saved)?;
    room.write("result.md", markdown.as_bytes())?;
    save_status(room, state, "closed", Some(result.outcome), clock)
}

/// Read and validate status without taking the run lock.
pub fn status(home: &Path, id: &str) -> Result<Status, String> {
    let room = Room::open(home, id, false)?;
    let metadata = room.read_json("convene.json")?;
    room.load_state(&metadata)?;
    read_status(&room)
}
/// Read valid panels newest first, skipping damaged folders.
pub fn list(home: &Path) -> Result<Vec<Summary>, String> {
    let root = home.join(".claude/voice/panels");
    let paths = match std::fs::read_dir(root) {
        Ok(paths) => paths,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(crate::text::READ_STATE.into()),
    };
    let mut summaries = Vec::new();
    for path in paths.flatten() {
        if let Some(id) = path.file_name().to_str()
            && let Ok(status) = status(home, id)
        {
            let room = Room::open(home, id, false)?;
            if let Ok(metadata) = room.read_json::<Convened>("convene.json") {
                summaries.push(Summary {
                    id: id.into(),
                    question: metadata.input.question,
                    status,
                });
            }
        }
    }
    summaries.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(summaries)
}
/// Read rounds and results without taking the run lock.
pub fn detail(home: &Path, id: &str) -> Result<Detail, String> {
    let room = Room::open(home, id, false)?;
    let saved = status(home, id)?;
    let metadata: Convened = room.read_json("convene.json")?;
    let entries: Vec<Entry> = room.lines("transcript.jsonl")?;
    let rounds: Vec<TimedRound> = room.lines("rounds.jsonl")?;
    let mut details = Vec::new();
    for timed in rounds {
        let mut withheld = BTreeMap::new();
        let mut failed = Vec::new();
        for entry in &entries {
            if let Entry::Post {
                round,
                seat,
                withheld: findings,
                error,
                ..
            } = entry
                && *round == timed.report.round
            {
                if !findings.is_empty() {
                    withheld.insert(seat.clone(), findings.clone());
                }
                if error.is_some() {
                    failed.push(seat.clone());
                }
            }
        }
        details.push(RoundDetail {
            round: timed.report.round,
            d: timed.report.d,
            withheld,
            failed,
        });
    }
    let (result, markdown) = if room.exists("result.json")? {
        let result: SavedResult = room.read_json("result.json")?;
        (
            Some(serde_json::to_value(result).map_err(|_| crate::text::READ_STATE)?),
            Some(String::from_utf8(room.read("result.md")?).map_err(|_| crate::text::READ_STATE)?),
        )
    } else {
        (None, None)
    };
    Ok(Detail {
        summary: Summary {
            id: id.into(),
            question: metadata.input.question.clone(),
            status: saved,
        },
        convene: metadata,
        rounds: details,
        asked: asked(&room)? && answer(&room)?.is_none(),
        result,
        markdown,
    })
}
