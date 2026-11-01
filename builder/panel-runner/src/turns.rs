//! Panel model calls through the person's Claude Code or per-seat model servers.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};
#[cfg(test)]
use voice_agent::provider::Provider;
use voice_agent::{
    claude::{self, Claude},
    guard::{self, Guard},
    ledger::{Cap, Session},
    provider::Brief,
    self_hosted::SelfHosted,
};
use voice_core::{
    assistants::assistant_dir,
    ceilings::{Ceilings, Source},
    error::Error,
    member::read_member,
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
/// Claude Code, per-seat model servers and sessions, and the whole panel's ceilings.
pub struct ClaudeTurns {
    claude: Claude,
    sessions: BTreeMap<String, Session>,
    servers: BTreeMap<String, Option<SelfHosted>>,
    ceilings: Ceilings,
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
        let mut ceilings = Vec::new();
        let mut servers = BTreeMap::new();
        for seat in seats {
            let settings = || -> voice_core::error::Result<Ceilings> {
                let folder = assistant_dir(home, seat)?;
                Ceilings::for_member(&read_member(&folder.join("member.json"))?)
            };
            ceilings
                .push(settings().map_err(|_| {
                    crate::text::assistant_problem(seat, crate::text::BAD_SETTINGS)
                })?);
            let server = SelfHosted::load(home, seat)
                .map_err(|_| crate::text::assistant_problem(seat, crate::text::BAD_SERVER))?
                .map(|server| server.with_timeout(Duration::from_secs(600)));
            servers.insert(seat.clone(), server);
            let cap = Cap::load(home, seat)
                .map_err(|_| crate::text::assistant_problem(seat, crate::text::BAD_LIMIT))?;
            let session = Session::open_existing(home, seat, cap).map_err(|error| {
                let reason = if matches!(&error, Error::Invalid(value) if value == voice_ingest::private::BUSY) {
                    crate::text::ASSISTANT_BUSY
                } else { crate::text::NO_ASSISTANT };
                crate::text::assistant_problem(seat, reason)
            })?;
            sessions.insert(seat.clone(), session);
        }
        Ok(Self {
            claude: claude.with_timeout(Duration::from_secs(600)),
            sessions,
            servers,
            ceilings: panel_ceilings(&ceilings),
        })
    }
    /// Attach the app's stop flag to model calls.
    pub fn with_stop(mut self, stop: Arc<AtomicBool>) -> Self {
        self.claude = self.claude.with_stop(Arc::clone(&stop));
        self.servers = self
            .servers
            .into_iter()
            .map(|(seat, server)| {
                (
                    seat,
                    server.map(|server| server.with_stop(Arc::clone(&stop))),
                )
            })
            .collect();
        self
    }
}
fn panel_ceilings(ceilings: &[Ceilings]) -> Ceilings {
    ceilings
        .iter()
        .cloned()
        .reduce(|all, seat| all.narrowest(&seat))
        .unwrap_or_else(Ceilings::defaults)
}
fn message(error: Error) -> String {
    match error {
        Error::Invalid(message) => message,
        _ => voice_agent::text::FAILED.into(),
    }
}
fn turn_error(error: Error) -> String {
    use voice_agent::text as agent_text;
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
                || agent_text::SERVER_MESSAGES.contains(&value.as_str())
                || value.starts_with(agent_text::SERVER_CUT_OFF_START)
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
        let guard = Guard::new(
            guard::providers(&self.claude, self.servers[seat].as_ref()),
            self.ceilings.clone(),
        );
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
                Some(post) if post.chars().count() <= 12000 => Turn {
                    post: Some(post.into()),
                    error: None,
                    usage,
                },
                _ => Turn {
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
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use voice_agent::{
        guard::{Guard, Sealed},
        provider::{Reach, Reply, Usage},
    };
    use voice_core::ceilings::{Ceilings, Source};

    #[test]
    fn panel_ceiling_fold_keeps_every_seats_narrowest_and_explicit_settings() {
        use voice_core::ceilings::Ceiling;
        let seat = |settings| {
            let mut member: voice_core::member::Member =
                serde_json::from_str(include_str!("../../../crew/_template/member.json")).unwrap();
            member.extra.insert("ceilings".into(), settings);
            Ceilings::for_member(&member).unwrap()
        };
        let first = seat(serde_json::json!({"mail":"anthropic", "crew-folder":"local-only"}));
        let second = seat(serde_json::json!({"mail":"anthropic", "typed":"local-only"}));
        let third = seat(serde_json::json!({"mail":"anthropic", "calendar":"local-only"}));
        let all = panel_ceilings(&[first.clone(), second.clone(), third.clone()]);
        assert_eq!(all.get(Source::Typed), Ceiling::LocalOnly);
        assert_eq!(all.get(Source::CrewFolder), Ceiling::LocalOnly);
        assert_eq!(all.get(Source::Mail), Ceiling::Anthropic);
        assert!(all.is_set(Source::Calendar));
        assert_eq!(panel_ceilings(&[third, second, first]), all);
    }

    #[test]
    fn server_turn_errors_are_shown_verbatim() {
        use voice_agent::text;
        for message in [text::SERVER_FAILED.to_owned(), text::server_cut_off(3)] {
            assert_eq!(turn_error(Error::Invalid(message.clone())), message);
        }
    }

    struct Fake(Mutex<Vec<Vec<Source>>>, String);
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
        fn call(&self, _session: &Session, sealed: &Sealed) -> voice_core::error::Result<Reply> {
            sealed.check(self.id(), self.reach())?;
            self.0.lock().unwrap().push(sealed.brief().sources.to_vec());
            Ok(Reply {
                output: serde_json::json!({"post": self.1}),
                usage: Usage {
                    input_tokens: 7,
                    output_tokens: 11,
                    cache_creation_input_tokens: 2,
                    cache_read_input_tokens: 3,
                },
                cost_usd: 0.25,
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
        let fake = Fake(Mutex::new(Vec::new()), "Try a small step.".into());
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
    #[test]
    fn panel_post_length_is_enforced_in_characters_and_keeps_usage() {
        let home = Scratch(std::env::temp_dir().join(format!(
            "panel-post-length-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&home.0).unwrap();
        let session = Session::open(&home.0, "pebble", Cap::default()).unwrap();
        for length in [12001, 12000] {
            let post = "界".repeat(length);
            let fake = Fake(Mutex::new(Vec::new()), post.clone());
            let guard = Guard::new(vec![&fake], Ceilings::defaults());
            let turn = ask(
                &guard,
                &session,
                "Answer as Pebble.",
                "Choose a small step.",
            );
            if length > 12000 {
                assert!(turn.post.is_none(), "accepted a 12,001-character post");
                assert_eq!(turn.error.as_deref(), Some(crate::text::UNREADABLE));
            } else {
                assert_eq!(turn.error, None);
                assert_eq!(turn.post.as_deref(), Some(post.as_str()));
            }
            assert_eq!(
                turn.usage,
                Some(TurnUsage {
                    input_tokens: 7,
                    output_tokens: 11,
                    cache_creation_input_tokens: 2,
                    cache_read_input_tokens: 3,
                    cost_usd: 0.25,
                })
            );
        }
    }
}
