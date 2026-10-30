//! The model-call seam and the person's own Claude Code.
use persona_builder::error::Error;
use persona_builder_agent::{
    claude::{self, Call, Claude},
    ledger::{Cap, Session},
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
        for seat in seats {
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
        let schema = serde_json::json!({"type":"object","properties":{"post":{"type":"string","minLength":1,"maxLength":12000}},"required":["post"],"additionalProperties":false});
        match self.claude.call(
            session,
            &Call {
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
}
