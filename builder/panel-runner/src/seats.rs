//! Load the assistants and check permission before a run.
use crate::{text, turns::Turns};
use persona_builder::{
    assistants::assistant_dir,
    error::Error,
    member::{Member, read_member},
};
use persona_builder_agent::ledger::{Cap, Session};
use persona_builder_ingest::{filters::Filters, private::BUSY};
use std::{collections::BTreeSet, path::Path};

/// An assistant's checked identity, decision model and privacy filters.
pub struct Seat {
    pub member: Member,
    pub decisions: String,
    pub filters: Filters,
    _session: Option<Session>,
}
/// Load all assistants before creating any panel files.
pub fn load(
    home: &Path,
    names: &[String],
    consented: &dyn Fn(&Path, &str) -> bool,
) -> Result<Vec<Seat>, String> {
    load_checked(home, names, consented, &|_| false)
}
/// Check assistants using spending sessions already held by the model caller.
pub fn load_for(
    home: &Path,
    names: &[String],
    consented: &dyn Fn(&Path, &str) -> bool,
    turns: &dyn Turns,
) -> Result<Vec<Seat>, String> {
    load_checked(home, names, consented, &|name| turns.holds_session(name))
}
fn load_checked(
    home: &Path,
    names: &[String],
    consented: &dyn Fn(&Path, &str) -> bool,
    held: &dyn Fn(&str) -> bool,
) -> Result<Vec<Seat>, String> {
    if !(2..=24).contains(&names.len())
        || names.iter().collect::<BTreeSet<_>>().len() != names.len()
    {
        return Err(text::BAD_SEATS.into());
    }
    let mut seats = Vec::new();
    for name in names {
        let problem = |reason| text::assistant_problem(name, reason);
        let folder = assistant_dir(home, name).map_err(|_| problem(text::NO_ASSISTANT))?;
        let member =
            read_member(&folder.join("member.json")).map_err(|_| problem(text::NO_ASSISTANT))?;
        let problem = |reason| text::assistant_problem(&member.name, reason);
        if member.slug != *name {
            return Err(problem(text::NO_ASSISTANT));
        }
        if !consented(home, name) {
            return Err(problem(text::PERMISSION));
        }
        let decisions =
            persona_builder_build::draft::read_text(&folder.join("decisions.md"), 256 * 1024)
                .map_err(|_| problem(text::NOT_BUILT_ASSISTANT))?
                .filter(|text| !text.trim().is_empty())
                .ok_or_else(|| problem(text::NOT_BUILT_ASSISTANT))?;
        let filters = Filters::compile(&member.filters).map_err(|_| problem(text::BAD_FILTERS))?;
        let cap = Cap::load(home, name).map_err(|_| problem(text::BAD_LIMIT))?;
        let session = if held(name) {
            None
        } else {
            Some(Session::open_existing(home, name, cap).map_err(|error| {
                problem(
                    if matches!(error, Error::Invalid(ref message) if message == BUSY) {
                        text::ASSISTANT_BUSY
                    } else {
                        text::NO_ASSISTANT
                    },
                )
            })?)
        };
        seats.push(Seat {
            member,
            decisions,
            filters,
            _session: session,
        });
    }
    Ok(seats)
}
