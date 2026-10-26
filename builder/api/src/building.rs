//! Building, spending and the interview at the app boundary. Raw errors never cross it.
use crate::{
    assistants::{claude_message, home},
    dto::*,
    history::{BUSY, FIND, PERMISSION, count, failure, locked, readable},
};
use persona_builder::{assistants, error::Error, member::Member};
use persona_builder_agent::{
    claude,
    ledger::{Cap, Session, local_day},
    text as agent_text,
};
use persona_builder_build::{build, interview, report, run, text};
use persona_builder_ingest::{corpus::read_records, filters::Filters, mine::to_jsonl, private};
use persona_builder_steps::{book::book, input::pretty, round};
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

type Result<T> = std::result::Result<T, Failure>;
const STOPPED: &str = "Stopped. Your assistant is as it was before this build. What was already finished is kept, so building again carries on from there.";
const NOTHING_YET: &str = "There's nothing to build from yet. Read your history and put it together first, in Your history.";
const SETTINGS: &str = "Something in this assistant's settings stops the build. Use the persona-builder command to see what.";
const BUILD_FAILED: &str = "The build stopped before it finished, so your assistant is as it was. What it already finished is kept: build again to carry on.";
const PRIVATE: &str = "The new version had something in it that looked personal or secret, so nothing was saved and your assistant is as it was. Build again to try once more.";
const LEDGER: &str = "Persona Builder's record of what this assistant has spent can't be read, so it won't spend anything more. Use the persona-builder command to see what's wrong.";
const BUILD_SAVE: &str = "Persona Builder couldn't save your assistant. Check there's space on this computer, then try again.";
const ENOUGH: &str = "Your history already holds enough answers to build from. The interview is for a thinner history.";
const ALREADY_DRAFTED: &str =
    "Your questions are already drafted. Answer them, or draft different ones.";
const DRAFT_FAILED: &str =
    "Claude drafted questions that didn't fit, twice, so none were saved. Try again.";
const NO_QUESTIONS: &str = "Draft the questions first.";
const ANSWER_BAD: &str = "Persona Builder couldn't keep that answer. Close it and open it again.";
const NO_ANSWERS: &str = "Answer at least one question first.";
const ALL_LEFT_OUT: &str = "Every answer looked personal or secret, so none was kept. Answer without names or private details, then try again.";
const ANSWERS_SAVE: &str = "Persona Builder couldn't save your answers. Check there's space on this computer, then try again.";
const CORPUS: &str = "decisions.jsonl";
const LETTERS: [&str; 4] = ["A", "B", "C", "D"];
fn cap_reached(spent: f64, cap: f64) -> String {
    format!(
        "Today's spending limit for this assistant is reached: about ${spent:.2} of ${cap:.2}, as Claude Code estimates it. Build again tomorrow to carry on."
    )
}

#[derive(Default)]
pub struct Builds {
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}
impl Builds {
    pub fn new() -> Self {
        Self::default()
    }
    fn register(&self, slug: &str) -> Result<Running<'_>> {
        let mut running = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if running.contains_key(slug) {
            return Err(failure(BUSY));
        }
        let stop = Arc::new(AtomicBool::new(false));
        running.insert(slug.into(), Arc::clone(&stop));
        Ok(Running {
            builds: self,
            slug: slug.into(),
            stop,
        })
    }
}
pub fn builds() -> &'static Builds {
    static BUILDS: OnceLock<Builds> = OnceLock::new();
    BUILDS.get_or_init(Builds::new)
}
struct Running<'a> {
    builds: &'a Builds,
    slug: String,
    stop: Arc<AtomicBool>,
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.builds
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.slug);
    }
}
pub(crate) fn open(home: &Path, slug: &str) -> Result<Session> {
    Session::open(home, slug, Cap::default()).map_err(|e| match e {
        Error::Invalid(message) if message == private::BUSY => failure(BUSY),
        _ => failure(BUILD_SAVE),
    })
}
pub(crate) fn spend(session: &Session) -> Result<Spend> {
    Ok(Spend {
        spent_usd: session.spent_today().map_err(|_| failure(LEDGER))?,
        daily_cap_usd: session.cap().daily_usd(),
    })
}
pub(crate) fn call_failure(error: Error, session: &Session, fallback: &str) -> Failure {
    match session.spent_today() {
        Ok(spent) if spent + session.cap().call_usd() > session.cap().daily_usd() + 1e-9 => {
            failure(&cap_reached(spent, session.cap().daily_usd()))
        }
        Err(_) => failure(LEDGER),
        _ => {
            let message = claude_message(error);
            if message == agent_text::FAILED {
                failure(fallback)
            } else {
                failure(&message)
            }
        }
    }
}
pub fn build_assistant_in(
    builds: &Builds,
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    request: SlugRequest,
    progress: &mut dyn FnMut(BuildProgress),
) -> Result<BuildDone> {
    let dir = assistants::assistant_dir(home, &request.slug).map_err(|_| failure(FIND))?;
    let running = builds.register(&request.slug)?;
    let member = readable(home, &request.slug)?;
    if private::read_private_optional(home, &request.slug, CORPUS)
        .map_err(|_| failure(SETTINGS))?
        .is_none()
    {
        return Err(failure(NOTHING_YET));
    }
    let prepared = run::prepare(&member, &dir, home, CORPUS).map_err(|_| failure(SETTINGS))?;
    let claude = claude::find_in(path, fallbacks)
        .map_err(|e| failure(&claude_message(e)))?
        .with_stop(Arc::clone(&running.stop));
    claude
        .check(home)
        .map_err(|e| failure(&claude_message(e)))?;
    let session = open(home, &request.slug)?;
    if !crate::read_consent(home, &request.slug).agreed {
        return Err(failure(PERMISSION));
    }
    let mut last_spend = Spend {
        spent_usd: 0.0,
        daily_cap_usd: Cap::default().daily_usd(),
    };
    let mut send = |line: &str| {
        if let Ok(now) = spend(&session) {
            last_spend = now;
        }
        progress(BuildProgress {
            line: line.into(),
            spend: last_spend.clone(),
        });
    };
    let built = build::build(
        &claude,
        &session,
        home,
        &prepared.inputs(&member, CORPUS, claude::DEFAULT_MODEL),
        &mut send,
    )
    .map_err(|error| {
        if running.stop.load(Ordering::SeqCst) {
            failure(STOPPED)
        } else if matches!(&error, Error::Invalid(message) if message == text::NOTHING_TO_BUILD) {
            failure(NOTHING_YET)
        } else {
            call_failure(error, &session, BUILD_FAILED)
        }
    })?;
    send(text::CHECKING);
    let checked = run::check(&member, &prepared, &built, claude::DEFAULT_MODEL)
        .map_err(|_| failure(PRIVATE))?;
    if running.stop.load(Ordering::SeqCst) {
        return Err(failure(STOPPED));
    }
    run::save(&member, &dir, home, checked).map_err(|_| failure(BUILD_SAVE))?;
    Ok(BuildDone {
        answered: count(prepared.answered),
        held_out: count(built.held),
        tested: built.backtest.is_some(),
        spend: spend(&session).unwrap_or(last_spend),
    })
}
pub fn stop_build_in(builds: &Builds, request: SlugRequest) -> Result<()> {
    if let Some(stop) = builds
        .running
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&request.slug)
    {
        stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}
pub fn spend_today_in(home: &Path, request: SlugRequest) -> Result<Spend> {
    readable(home, &request.slug)?;
    spend(&open(home, &request.slug)?)
}
fn object(home: &Path, slug: &str, name: &str) -> Option<Map<String, Value>> {
    let bytes = private::read_private_optional(home, slug, name).ok()??;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .as_object()
        .cloned()
}
fn questions(home: &Path, slug: &str) -> Option<Map<String, Value>> {
    object(home, slug, interview::QUESTIONS_FILE).filter(|q| round::check_questions(q).is_ok())
}
fn answers(home: &Path, slug: &str) -> Option<Map<String, Value>> {
    let file = object(home, slug, interview::ANSWERS_FILE)?;
    let rows = file.get("answers")?.as_object()?;
    for (n, row) in rows {
        n.parse::<u32>().ok()?;
        row.as_object()?;
        if let Some(pick) = row.get("pick")
            && !pick.is_string()
            && !pick
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string))
        {
            return None;
        }
        if row.get("own_words").is_some_and(|w| !w.is_string()) {
            return None;
        }
    }
    Some(file)
}
fn answered(home: &Path, slug: &str) -> Result<usize> {
    private::read_private_optional(home, slug, CORPUS)
        .map_err(|_| failure(SETTINGS))?
        .map(|bytes| {
            read_records(CORPUS, &bytes)
                .map(|r| interview::answered(&r))
                .map_err(|_| failure(SETTINGS))
        })
        .unwrap_or(Ok(0))
}
fn status(home: &Path, member: &Member) -> Result<InterviewStatus> {
    let slug = &member.slug;
    let questions = questions(home, slug)
        .map(|file| {
            file["questions"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|q| InterviewQuestion {
                    n: count(
                        q["n"]
                            .as_u64()
                            .unwrap_or(0)
                            .try_into()
                            .unwrap_or(usize::MAX),
                    ),
                    kind: q["header"].as_str().unwrap_or("").into(),
                    question: q["question"].as_str().unwrap_or("").into(),
                    multi_select: q["multiSelect"].as_bool().unwrap_or(false),
                    recommended: q["recommended"].as_str().map(str::to_owned),
                    options: q["options"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .zip(LETTERS)
                        .map(|(o, letter)| InterviewOption {
                            letter: letter.into(),
                            label: o["label"].as_str().unwrap_or("").into(),
                            description: o["description"].as_str().unwrap_or("").into(),
                        })
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut answers: Vec<_> = answers(home, slug)
        .map(|file| {
            file["answers"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(n, a)| InterviewAnswer {
                    n: n.parse().unwrap_or(0),
                    picks: match &a["pick"] {
                        Value::String(s) => vec![s.clone()],
                        Value::Array(a) => a
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect(),
                        _ => vec![],
                    },
                    own_words: a["own_words"].as_str().unwrap_or("").into(),
                })
                .collect()
        })
        .unwrap_or_default();
    answers.sort_by_key(|a| a.n);
    Ok(InterviewStatus {
        answered: count(answered(home, slug)?),
        thin_below: count(interview::THIN_BELOW),
        questions,
        answers,
        kept: private::read_private_optional(home, slug, interview::OUT)
            .map_err(|_| failure(SETTINGS))?
            .is_some(),
    })
}
pub fn interview_status_in(home: &Path, request: SlugRequest) -> Result<InterviewStatus> {
    status(home, &readable(home, &request.slug)?)
}
fn can_draft(home: &Path, slug: &str, replace: bool) -> Result<()> {
    if answered(home, slug)? >= interview::THIN_BELOW {
        return Err(failure(ENOUGH));
    }
    if !replace && questions(home, slug).is_some() {
        return Err(failure(ALREADY_DRAFTED));
    }
    Ok(())
}
pub fn draft_interview_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    request: DraftInterviewRequest,
) -> Result<InterviewStatus> {
    let member = readable(home, &request.slug)?;
    can_draft(home, &request.slug, request.replace)?;
    let book = book(&member).map_err(|_| failure(SETTINGS))?;
    let report = report::read(home, &request.slug).ok().flatten();
    let weights =
        interview::weights_from_report(&book, report.as_ref()).map_err(|_| failure(SETTINGS))?;
    let claude = claude::find_in(path, fallbacks).map_err(|e| failure(&claude_message(e)))?;
    claude
        .check(home)
        .map_err(|e| failure(&claude_message(e)))?;
    let session = open(home, &request.slug)?;
    if !crate::read_consent(home, &request.slug).agreed {
        return Err(failure(PERMISSION));
    }
    can_draft(home, &request.slug, request.replace)?;
    let drafted = interview::draft(&claude, &session, claude::DEFAULT_MODEL, &book, &weights)
        .map_err(|e| call_failure(e, &session, DRAFT_FAILED))?;
    if request.replace {
        private::remove_private(home, &request.slug, interview::ANSWERS_FILE)
            .map_err(|_| failure(ANSWERS_SAVE))?;
    }
    private::write_private(
        home,
        &request.slug,
        interview::QUESTIONS_FILE,
        pretty(&Value::Object(drafted)).as_bytes(),
    )
    .map_err(|_| failure(ANSWERS_SAVE))?;
    status(home, &member)
}
pub fn save_interview_answers_in(
    home: &Path,
    request: InterviewAnswersRequest,
) -> Result<InterviewStatus> {
    if request.answers.len() > interview::COUNT
        || request.answers.iter().any(|answer| answer.picks.len() > 4)
    {
        return Err(failure(ANSWER_BAD));
    }
    let (member, _lock) = locked(home, &request.slug)?;
    let questions = questions(home, &request.slug).ok_or_else(|| failure(NO_QUESTIONS))?;
    let mut seen = HashSet::new();
    let mut kept = Map::new();
    for answer in request.answers {
        let q = questions["questions"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|q| q["n"].as_u64() == Some(u64::from(answer.n)))
            .ok_or_else(|| failure(ANSWER_BAD))?;
        let options = q["options"].as_array().map_or(0, Vec::len);
        let multi = q["multiSelect"].as_bool().unwrap_or(false);
        if !seen.insert(answer.n)
            || answer.own_words.chars().count() > 2000
            || answer.picks.len() > options
            || answer.picks.iter().collect::<HashSet<_>>().len() != answer.picks.len()
            || (!multi && answer.picks.len() > 1)
            || answer
                .picks
                .iter()
                .any(|p| !LETTERS[..options].contains(&p.as_str()))
        {
            return Err(failure(ANSWER_BAD));
        }
        if answer.picks.is_empty() && answer.own_words.trim().is_empty() {
            continue;
        }
        let pick = if multi {
            json!(answer.picks)
        } else {
            answer.picks.first().map_or(Value::Null, |p| json!(p))
        };
        let mut row = json!({"own_words": answer.own_words});
        if !pick.is_null() {
            row["pick"] = pick;
        }
        kept.insert(answer.n.to_string(), row);
    }
    let file = json!({"answered": local_day(SystemTime::now()), "answers": kept});
    let filters = Filters::compile(&member.filters).map_err(|_| failure(SETTINGS))?;
    round::round(
        &member,
        &filters,
        interview::STEP,
        &questions,
        file.as_object().expect("object"),
        false,
    )
    .map_err(|_| failure(ANSWER_BAD))?;
    private::write_private(
        home,
        &request.slug,
        interview::ANSWERS_FILE,
        pretty(&file).as_bytes(),
    )
    .map_err(|_| failure(ANSWERS_SAVE))?;
    status(home, &member)
}
pub fn keep_interview_answers_in(home: &Path, request: SlugRequest) -> Result<InterviewKept> {
    let (member, _lock) = locked(home, &request.slug)?;
    let questions = questions(home, &request.slug).ok_or_else(|| failure(NO_QUESTIONS))?;
    let answers = answers(home, &request.slug)
        .filter(|a| a["answers"].as_object().is_some_and(|a| !a.is_empty()))
        .ok_or_else(|| failure(NO_ANSWERS))?;
    let filters = Filters::compile(&member.filters).map_err(|_| failure(SETTINGS))?;
    let kept = interview::answer(&member, &filters, &questions, &answers).map_err(|e| match e {
        Error::Invalid(message) if message == text::NO_ANSWERS => failure(NO_ANSWERS),
        Error::Invalid(message) if message == text::ALL_LEFT_OUT => failure(ALL_LEFT_OUT),
        _ => failure(ANSWER_BAD),
    })?;
    private::write_private(
        home,
        &request.slug,
        interview::OUT,
        to_jsonl(&kept.records).as_bytes(),
    )
    .map_err(|_| failure(ANSWERS_SAVE))?;
    Ok(InterviewKept {
        kept: count(kept.records.len()),
        unanswered: count(kept.unanswered),
        private_left_out: count(kept.personal.saturating_add(kept.secret)),
    })
}
pub fn build_assistant(
    request: SlugRequest,
    progress: &mut dyn FnMut(BuildProgress),
) -> Result<BuildDone> {
    let home = home(FIND)?;
    build_assistant_in(
        builds(),
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        request,
        progress,
    )
}
pub fn stop_build(request: SlugRequest) -> Result<()> {
    stop_build_in(builds(), request)
}
pub fn spend_today(request: SlugRequest) -> Result<Spend> {
    spend_today_in(&home(FIND)?, request)
}
pub fn interview_status(request: SlugRequest) -> Result<InterviewStatus> {
    interview_status_in(&home(FIND)?, request)
}
pub fn draft_interview(request: DraftInterviewRequest) -> Result<InterviewStatus> {
    let home = home(FIND)?;
    draft_interview_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        request,
    )
}
pub fn save_interview_answers(request: InterviewAnswersRequest) -> Result<InterviewStatus> {
    save_interview_answers_in(&home(FIND)?, request)
}
pub fn keep_interview_answers(request: SlugRequest) -> Result<InterviewKept> {
    keep_interview_answers_in(&home(FIND)?, request)
}
