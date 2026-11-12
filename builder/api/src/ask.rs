//! One private question to one assistant, with only the sources chosen for this call.
use crate::dto::*;
use crate::{
    assistants::{claude_message, home},
    building,
    history::{self, failure},
};
use radio_panels::{clean, fence};
use serde_json::{Value, json};
use std::time::SystemTime;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
use voice_agent::{
    claude,
    grants::Capability,
    guard::{self, Guard},
    ledger::local_day,
    provider::Brief,
    self_hosted::SelfHosted,
};
use voice_core::{
    assistants,
    ceilings::{Ceilings, Source},
    error::Error,
};
use voice_ingest::filters::{Filters, Kind};

pub mod text;
type Result<T> = std::result::Result<T, Failure>;
pub const STEP: &str = "ask";
pub const MAX_QUESTION: usize = 2000;
pub const MAX_ANSWER: usize = 4000;
pub const MAX_EVENTS_SENT: usize = 500;

pub fn ask_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    request: AskRequest,
    read_calendar: &dyn Fn(&str) -> Result<CalendarRead>,
) -> Result<AskAnswer> {
    let question = clean(&request.question, usize::MAX);
    if question.is_empty() || question.chars().count() > MAX_QUESTION {
        return Err(failure(text::BAD_QUESTION));
    }
    let selected = request
        .capabilities
        .iter()
        .map(|id| Capability::from_id(id).ok_or_else(|| failure(crate::permissions::UNKNOWN)))
        .collect::<Result<Vec<_>>>()?;
    let capabilities: Vec<_> = Capability::ALL
        .into_iter()
        .filter(|c| selected.contains(c))
        .collect();
    let slug = &request.slug;
    let member = history::readable(home, slug)?;
    let filters = Filters::compile(&member.filters).map_err(|_| failure(building::SETTINGS))?;
    if filters.kind(&question) == Some(Kind::Secret) {
        return Err(failure(text::SECRET));
    }
    let folder = assistants::assistant_dir(home, slug).map_err(|_| failure(history::FIND))?;
    let decisions = voice_build::draft::read_text(&folder.join("decisions.md"), 256 * 1024)
        .map_err(|_| failure(text::NOT_BUILT))?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| failure(text::NOT_BUILT))?;
    let mut sources = vec![Source::CrewFolder, Source::Typed];
    sources.extend(capabilities.iter().map(|capability| capability.source()));
    let claude = claude::find_in(home, path, fallbacks).map_err(|e| failure(&claude_message(e)))?;
    let server = SelfHosted::load(home, slug).map_err(|_| failure(building::SETTINGS))?;
    let guard = Guard::new(
        guard::providers(&claude, server.as_ref()),
        Ceilings::for_member(&member).map_err(|_| failure(building::SETTINGS))?,
    );
    let provider = guard.route(&sources).map_err(|e| match e {
        Error::Invalid(message) => failure(&message),
        _ => failure(text::FAILED),
    })?;
    if provider.id() == claude::AGENT_ID {
        claude
            .check(home)
            .map_err(|e| failure(&claude_message(e)))?;
    }
    // Read before opening the session, so nothing is spent until every read has worked.
    let mut calendar = None;
    for capability in capabilities {
        match capability {
            Capability::CalendarRead => calendar = Some(read_calendar(slug)?),
        }
    }
    let session = building::open(home, slug)?;
    if !crate::read_consent(home, slug).agreed {
        return Err(failure(history::PERMISSION));
    }
    let prompt = prompt(
        &member.name,
        &member.principal,
        &local_day(SystemTime::now()),
        &decisions,
        &question,
        calendar.as_ref(),
    );
    let reply = guard
        .call(
            &session,
            &Brief {
                sources: &sources,
                step: STEP,
                system: text::SYSTEM,
                prompt: &prompt,
                model: claude::DEFAULT_MODEL,
                schema: &schema(),
            },
        )
        .map_err(|e| building::call_failure(e, &session, text::FAILED))?;
    let answer = answer(&reply.output)?;
    Ok(AskAnswer {
        answer,
        said: text::said(
            provider.id(),
            &voice_agent::text::typed_name(&member.name),
            calendar.as_ref(),
        ),
    })
}
fn answer(output: &Value) -> Result<String> {
    output["answer"]
        .as_str()
        .map(|s| clean(s, MAX_ANSWER + 1))
        .map(|s| {
            s.chars()
                .map(|c| {
                    if voice_ingest::text::hidden(c) && c != '\n' && c != '\t' {
                        ' '
                    } else {
                        c
                    }
                })
                .collect::<String>()
        })
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && s.chars().count() <= MAX_ANSWER)
        .ok_or_else(|| failure(text::UNREADABLE))
}
pub fn ask(request: AskRequest) -> Result<AskAnswer> {
    let home = home(text::FAILED)?;
    ask_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        request,
        &|slug| crate::calendar::read_calendar(SlugRequest { slug: slug.into() }, STEP),
    )
}
fn schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["answer"],"properties":{"answer":{"type":"string","minLength":1,"maxLength":4000}}})
}
fn section(label: &str, value: &str) -> String {
    fence(label, value).expect("a fixed fence label")
}
fn prompt(
    name: &str,
    principal: &str,
    today: &str,
    decisions: &str,
    question: &str,
    calendar: Option<&CalendarRead>,
) -> String {
    let name = clean(name, 64);
    let principal = clean(principal, 64);
    let mut parts = vec![
        text::introduction(&name, &principal, today),
        section("question", question),
        section("how they decide", decisions),
    ];
    if let Some(read) = calendar {
        parts.push(text::calendar_heading(&principal, read));
        let events = read
            .events
            .iter()
            .take(MAX_EVENTS_SENT)
            .map(text::event_line)
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(section(
            "calendar",
            if events.is_empty() {
                "No events."
            } else {
                &events
            },
        ));
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests;
