//! The model-call seam and the person's own Claude Code.
use persona_builder::{
    assistants::assistant_dir,
    ceilings::{Ceilings, Source},
    error::Error,
    member::read_member,
};
#[cfg(test)]
use persona_builder_agent::provider::Provider;
use persona_builder_agent::{
    claude::{self, Claude},
    guard::Guard,
    ledger::{Cap, Session},
    provider::Brief,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

/// Reported usage for one assistant's turn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cost_usd: f64,
}
/// One assistant's answer or a plain failure sentence.
#[derive(Clone, Debug, Default)]
pub struct Turn {
    pub post: Option<String>,
    pub error: Option<String>,
    pub usage: Option<TurnUsage>,
}
/// Calls may run concurrently, with at most six assistants at once.
pub trait Turns: Sync {
    /// Ask one assistant to answer the supplied instructions and fenced prompt.
    fn turn(&self, seat: &str, system: &str, prompt: &str) -> Turn;
    /// Whether this caller already holds the assistant’s spending session.
    fn holds_session(&self, _seat: &str) -> bool {
        false
    }
}
/// Claude Code and the sessions held for each assistant.
pub struct ClaudeTurns {
    claude: Claude,
    sessions: BTreeMap<String, Session>,
    ceilings: BTreeMap<String, Ceilings>,
}
impl ClaudeTurns {
    /// Find and check Claude Code and lock each assistant's spending session.
    pub fn new(
        home: &Path,
        path: Option<&OsStr>,
        fallbacks: &[PathBuf],
        seats: &[String],
    ) -> Result<Self, String> {
        let claude = claude::find_in(home, path, fallbacks).map_err(message)?;
        claude.check(home).map_err(message)?;
        let mut sessions = BTreeMap::new();
        let mut ceilings = BTreeMap::new();
        for seat in seats {
            let settings = || -> persona_builder::error::Result<Ceilings> {
                let folder = assistant_dir(home, seat)?;
                Ceilings::for_member(&read_member(&folder.join("member.json"))?)
            };
            ceilings.insert(
                seat.clone(),
                settings()
                    .map_err(|_| crate::text::assistant_problem(seat, crate::text::BAD_SETTINGS))?,
            );
            let cap = Cap::load(home, seat)
                .map_err(|_| crate::text::assistant_problem(seat, crate::text::BAD_LIMIT))?;
            let session = Session::open_existing(home, seat, cap).map_err(|error| {
                let reason = if matches!(&error, Error::Invalid(value) if value == persona_builder_ingest::private::BUSY) {
                    crate::text::ASSISTANT_BUSY
                } else { crate::text::NO_ASSISTANT };
                crate::text::assistant_problem(seat, reason)
            })?;
            sessions.insert(seat.clone(), session);
        }
        Ok(Self {
            claude: claude.with_timeout(Duration::from_secs(600)),
            sessions,
            ceilings,
        })
    }
    /// Attach the app's stop flag to model calls.
    pub fn with_stop(self, stop: Arc<AtomicBool>) -> Self {
        Self {
            claude: self.claude.with_stop(stop),
            ..self
        }
    }
}
fn message(error: Error) -> String {
    match error {
        Error::Invalid(message) => message,
        _ => persona_builder_agent::text::FAILED.into(),
    }
}
fn turn_error(error: Error) -> String {
    use persona_builder_agent::text as agent_text;
    match error {
        Error::Invalid(value)
            if [
                agent_text::PLAN_STOPPED,
                agent_text::SIGNED_OUT,
                agent_text::FAILED,
                agent_text::STOPPED,
                agent_text::DOES_NOT_RUN,
            ]
            .contains(&value.as_str())
                || value.starts_with("Nothing was sent.")
                || value.starts_with("Today's spending limit for this assistant is reached:")
                || value.starts_with("Claude Code was still working on this step after ") =>
        {
            value
        }
        _ => agent_text::FAILED.into(),
    }
}
impl Turns for ClaudeTurns {
    fn holds_session(&self, seat: &str) -> bool {
        self.sessions.contains_key(seat)
    }
    fn turn(&self, seat: &str, system: &str, prompt: &str) -> Turn {
        let Some(session) = self.sessions.get(seat) else {
            return Turn {
                error: Some(crate::text::NO_ASSISTANT.into()),
                ..Turn::default()
            };
        };
        let guard = Guard::new(vec![&self.claude], self.ceilings[seat].clone());
        ask(&guard, session, system, prompt)
    }
}

pub const SOURCES: &[Source] = &[Source::CrewFolder, Source::Typed];

fn ask(guard: &Guard, session: &Session, system: &str, prompt: &str) -> Turn {
    let schema = serde_json::json!({"type":"object","properties":{"post":{"type":"string","minLength":1,"maxLength":12000}},"required":["post"],"additionalProperties":false});
    match guard.call(
        session,
        &Brief {
            sources: SOURCES,
            step: "panel-turn",
            system,
            prompt,
            model: claude::DEFAULT_MODEL,
            schema: &schema,
        },
    ) {
        Ok(reply) => {
            let usage = Some(TurnUsage {
                input_tokens: reply.usage.input_tokens,
                output_tokens: reply.usage.output_tokens,
                cache_creation_input_tokens: reply.usage.cache_creation_input_tokens,
                cache_read_input_tokens: reply.usage.cache_read_input_tokens,
                cost_usd: reply.cost_usd,
            });
            match reply.output.get("post").and_then(serde_json::Value::as_str) {
                Some(post) => Turn {
                    post: Some(post.into()),
                    error: None,
                    usage,
                },
                None => Turn {
                    error: Some(crate::text::UNREADABLE.into()),
                    usage,
                    ..Turn::default()
                },
            }
        }
        Err(error) => Turn {
            error: Some(turn_error(error)),
            ..Turn::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use persona_builder::ceilings::{Ceilings, Source};
    use persona_builder_agent::{
        guard::{Guard, Sealed},
        provider::{Reach, Reply, Usage},
    };
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    struct Fake(Mutex<Vec<Vec<Source>>>);
    impl Provider for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn name(&self) -> &'static str {
            "Fake"
        }
        fn reach(&self) -> Reach {
            Reach::OwnMachines
        }
        fn call(
            &self,
            _session: &Session,
            sealed: &Sealed,
        ) -> persona_builder::error::Result<Reply> {
            sealed.check(self.id(), self.reach())?;
            self.0.lock().unwrap().push(sealed.brief().sources.to_vec());
            Ok(Reply {
                output: serde_json::json!({"post": "Try a small step."}),
                usage: Usage::default(),
                cost_usd: 0.0,
            })
        }
    }
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn panel_turn_briefs_name_crew_folder_and_typed() {
        let home = Scratch(std::env::temp_dir().join(format!(
            "panel-sources-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&home.0).unwrap();
        let session = Session::open(&home.0, "pebble", Cap::default()).unwrap();
        let fake = Fake(Mutex::new(Vec::new()));
        let guard = Guard::new(vec![&fake], Ceilings::defaults());
        let turn = ask(
            &guard,
            &session,
            "Answer as Pebble.",
            "Choose a small step.",
        );
        assert_eq!(turn.error, None);
        assert_eq!(turn.post.as_deref(), Some("Try a small step."));
        assert_eq!(
            *fake.0.lock().unwrap(),
            vec![vec![Source::CrewFolder, Source::Typed]]
        );
    }
}
