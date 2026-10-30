//! Panels at the app boundary, with progress and a way to stop.
use crate::dto::*;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

type Result<T> = std::result::Result<T, Failure>;
pub use voice_panel_runner::text;
pub use voice_panel_runner::text::BAD_KIND;
use voice_panel_runner::{self as runner, ClaudeTurns, SystemClock};

/// Tracks panels running in this app.
#[derive(Default)]
pub struct Panels {
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}
impl Panels {
    /// Start an empty registry of running panels.
    pub fn new() -> Self {
        Self::default()
    }
    fn register(&self, id: &str) -> Result<Running<'_>> {
        let mut running = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if running.contains_key(id) {
            return Err(failure(text::BUSY));
        }
        let stop = Arc::new(AtomicBool::new(false));
        running.insert(id.into(), Arc::clone(&stop));
        Ok(Running {
            panels: self,
            id: id.into(),
            stop,
        })
    }
}
struct Running<'a> {
    panels: &'a Panels,
    id: String,
    stop: Arc<AtomicBool>,
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.panels
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.id);
    }
}
/// Return the app's registry of running panels.
pub fn panels() -> &'static Panels {
    static PANELS: OnceLock<Panels> = OnceLock::new();
    PANELS.get_or_init(Panels::new)
}
fn failure(message: impl Into<String>) -> Failure {
    Failure {
        message: message.into(),
    }
}
fn consented(home: &Path, slug: &str) -> bool {
    crate::read_consent(home, slug).agreed
}
fn turns(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    seats: &[String],
    running: &Running<'_>,
) -> Result<ClaudeTurns> {
    drop(
        runner::seats::load(home, seats, &consented).map_err(|message| {
            if message.ends_with(text::PERMISSION) {
                failure(text::PERMISSION)
            } else {
                failure(message)
            }
        })?,
    );
    let turns = ClaudeTurns::new(home, path, fallbacks, seats)
        .map_err(failure)?
        .with_stop(Arc::clone(&running.stop));
    if seats.iter().any(|slug| !consented(home, slug)) {
        return Err(failure(text::PERMISSION));
    }
    Ok(turns)
}
fn send(id: &str, progress: runner::Progress, callback: &mut dyn FnMut(PanelProgress)) {
    let round = match &progress {
        runner::Progress::RoundStarted { round } | runner::Progress::RoundDone { round, .. } => {
            *round
        }
    };
    callback(PanelProgress {
        id: id.into(),
        round,
        line: text::progress(&progress),
    });
}
fn ran(ran: runner::Ran) -> PanelRan {
    PanelRan {
        id: ran.id,
        status: ran.status,
        outcome: ran.outcome,
    }
}
/// Start a panel with the given home and Claude Code locations.
pub fn convene_panel_in(
    panels: &Panels,
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    request: ConvenePanelRequest,
    progress: &mut dyn FnMut(PanelProgress),
) -> Result<PanelRan> {
    let id = runner::room::new_id().map_err(failure)?;
    let running = panels.register(&id)?;
    let request = runner::Convene {
        question: request.question,
        background: request.background,
        kind: request.kind,
        seats: request.seats,
        time_box: request.time_box,
    };
    runner::run::validate(&request).map_err(failure)?;
    let turns = turns(home, path, fallbacks, &request.seats, &running)?;
    runner::convene_named(
        home,
        &id,
        &request,
        &consented,
        &turns,
        &SystemClock::new(),
        &running.stop,
        &mut |p| send(&id, p, progress),
    )
    .map(ran)
    .map_err(failure)
}
/// Carry on a panel with the given home and Claude Code locations.
pub fn resume_panel_in(
    panels: &Panels,
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    request: ResumePanelRequest,
    progress: &mut dyn FnMut(PanelProgress),
) -> Result<PanelRan> {
    let detail = runner::detail(home, &request.id).map_err(failure)?;
    let running = panels.register(&request.id)?;
    let turns = turns(home, path, fallbacks, &detail.convene.input.seats, &running)?;
    let request = runner::Resume {
        id: request.id,
        steer: request.steer,
        time_box: request.time_box,
    };
    runner::resume(
        home,
        &request,
        &consented,
        &turns,
        &SystemClock::new(),
        &running.stop,
        &mut |p| send(&request.id, p, progress),
    )
    .map(ran)
    .map_err(failure)
}
/// Ask a panel in the given registry to stop after its current turns.
pub fn stop_panel_in(panels: &Panels, request: PanelIdRequest) -> Result<()> {
    runner::room::validate_id(&request.id).map_err(failure)?;
    if let Some(stop) = panels
        .running
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&request.id)
    {
        stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}
fn summary(summary: runner::Summary) -> PanelSummary {
    PanelSummary {
        id: summary.id,
        question: summary.question,
        status: summary.status.status,
        round: summary.status.round,
        outcome: summary.status.outcome,
        updated_at: summary.status.updated_at,
    }
}
/// Read saved panels in the given home, newest first.
pub fn list_panels_in(home: &Path) -> Result<Vec<PanelSummary>> {
    runner::list(home)
        .map(|rows| rows.into_iter().map(summary).collect())
        .map_err(failure)
}
fn answer(value: &serde_json::Value) -> (Option<String>, Vec<String>) {
    (
        value.as_str().map(str::to_owned),
        value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    )
}
fn result(value: serde_json::Value, markdown: String) -> Result<PanelResultView> {
    let outcome = value
        .get("outcome")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| failure(text::READ_FAILED))?
        .into();
    let (answer_text, answer_items) = answer(&value["answer"]);
    let groups = value
        .get("groups")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| failure(text::READ_FAILED))?
        .iter()
        .map(|group| {
            let (answer_text, answer_items) = answer(&group["answer"]);
            Ok(PanelGroup {
                members: serde_json::from_value(group["members"].clone())
                    .map_err(|_| failure(text::READ_FAILED))?,
                weight: group["weight"]
                    .as_f64()
                    .ok_or_else(|| failure(text::READ_FAILED))?,
                answer_text,
                answer_items,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let dissent = value
        .get("dissent")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| failure(text::READ_FAILED))?
        .iter()
        .map(|entry| {
            Ok(PanelDissent {
                seat: entry["crewmate"]
                    .as_str()
                    .ok_or_else(|| failure(text::READ_FAILED))?
                    .into(),
                holdout: entry["holdout"]
                    .as_bool()
                    .ok_or_else(|| failure(text::READ_FAILED))?,
                objections: entry["objections"]
                    .as_str()
                    .ok_or_else(|| failure(text::READ_FAILED))?
                    .into(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PanelResultView {
        outcome,
        answer_text,
        answer_items,
        groups,
        dissent,
        markdown,
    })
}
/// Read a saved panel, including its rounds and result, in the given home.
pub fn panel_in(home: &Path, request: PanelIdRequest) -> Result<PanelDetail> {
    let detail = runner::detail(home, &request.id).map_err(failure)?;
    Ok(PanelDetail {
        summary: summary(detail.summary),
        kind: detail.convene.input.kind,
        seats: detail.convene.input.seats,
        time_box: detail.convene.input.time_box,
        asked: detail.asked,
        rounds: detail
            .rounds
            .into_iter()
            .map(|round| PanelRound {
                round: round.round,
                d: round.d,
                withheld: round
                    .withheld
                    .into_iter()
                    .map(|(seat, findings)| PanelWithheld { seat, findings })
                    .collect(),
                failed: round.failed,
            })
            .collect(),
        result: detail
            .result
            .map(|value| result(value, detail.markdown.unwrap_or_default()))
            .transpose()?,
    })
}
/// Start a panel using this app's home and Claude Code.
pub fn convene_panel(
    request: ConvenePanelRequest,
    progress: &mut dyn FnMut(PanelProgress),
) -> Result<PanelRan> {
    let home = crate::assistants::home(text::READ_FAILED)?;
    convene_panel_in(
        panels(),
        &home,
        std::env::var_os("PATH").as_deref(),
        &persona_builder_agent::claude::fallbacks(&home),
        request,
        progress,
    )
}
/// Carry on a panel using this app's home and Claude Code.
pub fn resume_panel(
    request: ResumePanelRequest,
    progress: &mut dyn FnMut(PanelProgress),
) -> Result<PanelRan> {
    let home = crate::assistants::home(text::READ_FAILED)?;
    resume_panel_in(
        panels(),
        &home,
        std::env::var_os("PATH").as_deref(),
        &persona_builder_agent::claude::fallbacks(&home),
        request,
        progress,
    )
}
/// Ask a panel to stop after its current turns.
pub fn stop_panel(request: PanelIdRequest) -> Result<()> {
    stop_panel_in(panels(), request)
}
/// Read saved panels in this app's home, newest first.
pub fn list_panels() -> Result<Vec<PanelSummary>> {
    list_panels_in(&crate::assistants::home(text::READ_FAILED)?)
}
/// Read a saved panel with its rounds and result in this app's home.
pub fn panel(request: PanelIdRequest) -> Result<PanelDetail> {
    panel_in(&crate::assistants::home(text::READ_FAILED)?, request)
}
