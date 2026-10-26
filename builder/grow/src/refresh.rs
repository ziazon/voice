//! Draft a refresh in private state; only the person's answer is recorded here.
use crate::{
    diff,
    edit::{Edit, EditKind, apply_edit},
    record::*,
    rules,
    store::{Prefix, Store},
    text,
    version::{DECISIONS_MOST, version},
};
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_agent::{claude::Claude, ledger::Session};
use persona_builder_build::{
    build::{Asker, Misfit, SHINGLE},
    draft::read_text,
};
use persona_builder_ingest::filters::Filters;
use persona_builder_steps::{book::book, input::RecordList, shingle::overlaps, text_check::hidden};
use serde_json::{Value, json};
use std::{collections::HashSet, path::Path};

pub const MOST_CASES: usize = 40;
pub const MOST_EDITS: usize = 10;
pub const MOST_LINE: usize = 400;
pub const MOST_REJECTED: usize = 5;
/// Fixed instructions only: this string is passed in argv.
pub const REFRESH_SYSTEM: &str = include_str!("prompts/refresh.md");
#[derive(Debug)]
pub enum Proposed {
    Nothing,
    Drafted(Proposal),
}

pub fn refresh_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["summary","edits"],
    "properties":{"summary":{"type":"string"},"edits":{"type":"array","items":{
        "type":"object","additionalProperties":false,
        "required":["area","kind","old","text","why","cases"],"properties":{
            "area":{"type":"string"},"kind":{"type":"string","enum":["add","replace"]},
            "old":{"type":["string","null"]},"text":{"type":"string"},
            "why":{"type":"string"},"cases":{"type":"array","items":{"type":"string"}}
        }}}}})
}

pub fn refresh_prompt(
    categories: &[String],
    decisions: &str,
    cases: &[Case],
    signals: &[Signal],
    rejected: &[Proposal],
) -> String {
    let mut out = format!(
        "Draft changes to how the person decides, learning from the answers below. Return the summary and edits in the required shape.\n\nAreas (use exactly these names):\n{}\n\nCurrent decisions.md:\n{decisions}\n\nTests from the person's answers, one JSON object per line:\n",
        serde_json::to_string(categories).expect("strings serialize")
    );
    for case in cases {
        let mut row = json!({"id":case.id,"area":case.area,"situation":case.situation,"wanted":case.expected});
        if let Some(options) = &case.options {
            row["options"] = json!(options);
        }
        let sources: Vec<_> = signals
            .iter()
            .filter(|s| case.from_signals.contains(&s.id))
            .collect();
        // Keep all sources when more than one signal contributed, with the single-source fields
        // at the top level as well for the common one-answer test.
        if let Some(signal) = sources.first() {
            row["kind"] = json!(signal.kind);
            if let Some(said) = &signal.persona_said {
                row["persona_said"] = json!(said);
            }
        }
        if sources.len() > 1 {
            row["signals"] = json!(
                sources
                    .iter()
                    .map(|s| {
                        let mut value = json!({"kind":s.kind});
                        if let Some(said) = &s.persona_said {
                            value["persona_said"] = json!(said);
                        }
                        value
                    })
                    .collect::<Vec<_>>()
            );
        }
        out.push_str(&row.to_string());
        out.push('\n');
    }
    let mut earlier: Vec<_> = rejected
        .iter()
        .filter(|p| p.cases.iter().any(|id| cases.iter().any(|c| &c.id == id)))
        .collect();
    earlier.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
    if !earlier.is_empty() {
        out.push_str("\nThe person said no to these earlier drafts, or they were tested and undone; propose something else\n");
        for p in earlier.into_iter().take(MOST_REJECTED) {
            out.push_str(&p.summary);
            out.push('\n');
            for change in &p.changes {
                for line in diff::added_lines(&change.diff) {
                    out.push('+');
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
    }
    out
}

fn plain_line(value: &Value, place: &str) -> std::result::Result<String, Misfit> {
    let words = value
        .as_str()
        .ok_or_else(|| Misfit::plain(text::reply_line(place)))?;
    if words.trim().is_empty()
        || words.trim() != words
        || words.chars().count() > MOST_LINE
        || words.chars().any(hidden)
    {
        return Err(Misfit::plain(text::reply_line(place)));
    }
    Ok(words.into())
}

pub fn check_reply(
    categories: &[String],
    decisions: &str,
    cases: &[Case],
    signals: &[Signal],
    filters: &Filters,
    reply: &Value,
) -> std::result::Result<Vec<Edit>, Misfit> {
    let shape = || Misfit::plain(text::REPLY_SHAPE);
    let obj = reply.as_object().ok_or_else(shape)?;
    if obj.len() != 2 || !obj.contains_key("summary") || !obj.contains_key("edits") {
        return Err(shape());
    }
    let summary = plain_line(&reply["summary"], "summary")?;
    let items = reply["edits"].as_array().ok_or_else(shape)?;
    if items.is_empty() || items.len() > MOST_EDITS {
        return Err(Misfit::plain(text::EDIT_COUNT));
    }
    let mut edits = Vec::new();
    let mut current = decisions.to_owned();
    let mut words = vec![summary];
    for item in items {
        let object = item.as_object().ok_or_else(shape)?;
        if object.len() != 6
            || ["area", "kind", "old", "text", "why", "cases"]
                .iter()
                .any(|key| !object.contains_key(*key))
        {
            return Err(shape());
        }
        let area = item["area"]
            .as_str()
            .filter(|a| categories.iter().any(|c| c == a))
            .ok_or_else(|| Misfit::plain(text::REPLY_AREA))?;
        let kind = match item["kind"].as_str() {
            Some("add") => EditKind::Add,
            Some("replace") => EditKind::Replace,
            _ => return Err(shape()),
        };
        let old = match kind {
            EditKind::Add if item["old"].is_null() => None,
            EditKind::Replace if item["old"].is_string() => {
                Some(plain_line(&item["old"], "earlier point")?)
            }
            _ => return Err(Misfit::plain(text::REPLY_OLD)),
        };
        let point = plain_line(&item["text"], "point")?;
        let why = plain_line(&item["why"], "reason")?;
        let ids = item["cases"]
            .as_array()
            .ok_or_else(|| Misfit::plain(text::REPLY_CASES))?;
        let mut seen = HashSet::new();
        let mut named = Vec::new();
        for id in ids {
            let id = id
                .as_str()
                .filter(|id| cases.iter().any(|c| c.id == *id) && seen.insert(*id))
                .ok_or_else(|| Misfit::plain(text::REPLY_CASES))?;
            named.push(id.to_owned());
        }
        if named.is_empty() {
            return Err(Misfit::plain(text::REPLY_CASES));
        }
        if let Some(old) = &old {
            // Replacing a replacement would target the same item a second time too.
            if edits
                .iter()
                .any(|e: &Edit| e.kind == EditKind::Replace && e.area == area && &e.text == old)
            {
                return Err(Misfit::plain(text::REPLY_REPLACE));
            }
        }
        let edit = Edit {
            area: area.into(),
            kind,
            old,
            text: point.clone(),
            why: why.clone(),
            cases: named,
        };
        let next = apply_edit(&current, &edit).map_err(|_| Misfit::plain(text::REPLY_APPLY))?;
        if next == current {
            return Err(Misfit::plain(text::REPLY_APPLY));
        }
        current = next;
        words.extend([point, why]);
        edits.push(edit);
    }
    check_words(&words, cases, signals, filters)?;
    Ok(edits)
}

pub(crate) fn check_words(
    words: &[String],
    cases: &[Case],
    signals: &[Signal],
    filters: &Filters,
) -> std::result::Result<(), Misfit> {
    for (at, value) in words.iter().enumerate() {
        if filters.kind(value).is_some() {
            return Err(Misfit::new(
                format!("{} (place {})", text::REPLY_PRIVATE, at + 1),
                format!("{} (place {})", text::REPLY_PRIVATE, at + 1),
            ));
        }
    }
    let mut records: RecordList = Vec::new();
    for case in cases {
        let map = json!({"id":case.id,"question":case.situation,"text":case.expected})
            .as_object()
            .expect("object")
            .clone();
        records.push((records.len() + 1, map));
        for signal in signals.iter().filter(|s| case.from_signals.contains(&s.id)) {
            let map = json!({"id":signal.id,"question":signal.situation,"text":signal.wanted,"context":signal.persona_said.as_deref().unwrap_or("")}).as_object().expect("object").clone();
            records.push((records.len() + 1, map));
        }
    }
    let hits = overlaps(&words.join("\n"), &[("answers", &records)], SHINGLE)
        .map_err(|_| Misfit::plain(text::REPLY_RECORDS))?;
    if !hits.is_empty() {
        return Err(Misfit::new(
            format!("Rewrite these runs in your own words: {}", hits.join("; ")),
            text::quoted(hits.len()),
        ));
    }
    Ok(())
}

pub(crate) fn decisions(member_dir: &Path) -> Result<String> {
    read_text(&member_dir.join("decisions.md"), DECISIONS_MOST)?
        .ok_or_else(|| Error::Invalid(text::NOT_BUILT.into()))
}

fn pending(store: &Store, base: &str) -> Result<Option<(Proposal, bool)>> {
    let approvals = store.approvals()?;
    let measurements = store.measurements()?;
    let mut proposals = store.proposals()?;
    proposals.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
    for proposal in proposals.into_iter().filter(|p| p.base == base) {
        let approval = approvals.iter().find(|a| a.proposal == proposal.id);
        match approval {
            None => return Ok(Some((proposal, false))),
            Some(a)
                if a.decision == Decision::Approved
                    && !measurements.iter().any(|m| m.proposal == proposal.id) =>
            {
                return Ok(Some((proposal, true)));
            }
            _ => {}
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
pub fn propose(
    claude: &Claude,
    session: &Session,
    home: &Path,
    store: &Store,
    member: &Member,
    member_dir: &Path,
    filters: &Filters,
    model: &str,
    now: &str,
) -> Result<Proposed> {
    // Session already holds the state lock for its entire life. Never lock or call locked
    // save methods here: all writes below share that lock through the unlocked store method.
    let before = decisions(member_dir)?;
    let base = version(before.as_bytes());
    if let Some((p, approved)) = pending(store, &base)? {
        return Err(Error::Invalid(if approved {
            text::open_approved(&p.id)
        } else {
            text::open_waiting(&p.id)
        }));
    }
    let mut cases: Vec<_> = store
        .cases()?
        .iter()
        .filter(|c| c.status == Status::New)
        .cloned()
        .collect();
    cases.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
    cases.truncate(MOST_CASES);
    if cases.is_empty() {
        return Ok(Proposed::Nothing);
    }
    let signals = store.signals()?;
    let approvals = store.approvals()?;
    let measurements = store.measurements()?;
    let rejected: Vec<_> = store
        .proposals()?
        .into_iter()
        .filter(|p| {
            approvals
                .iter()
                .any(|a| a.proposal == p.id && a.decision == Decision::Rejected)
                || measurements
                    .iter()
                    .any(|m| m.proposal == p.id && m.outcome == KeepOrRevert::Revert)
        })
        .collect();
    let categories = book(member)?.categories;
    let prompt = refresh_prompt(&categories, &before, &cases, &signals, &rejected);
    let mut asker =
        Asker::new(claude, session, home, &member.slug, model).with_misfit(text::refresh_misfit);
    let ((summary, edits), _) = asker.ask(
        "refresh",
        REFRESH_SYSTEM,
        &refresh_schema(),
        &prompt,
        text::DRAFTING,
        |reply| {
            let edits = check_reply(&categories, &before, &cases, &signals, filters, reply)?;
            Ok((
                reply["summary"]
                    .as_str()
                    .expect("checked summary")
                    .to_owned(),
                edits,
            ))
        },
    )?;
    let mut current = before.clone();
    let mut applied = before;
    let mut changes = Vec::new();
    let mut named = Vec::new();
    for edit in edits {
        let next = apply_edit(&current, &edit)?;
        let written = diff::unified("decisions.md", &current, &next)?;
        applied = diff::apply(&applied, &written, "decisions.md")?;
        if applied != next {
            return Err(Error::Invalid(text::DIFF_BUG.into()));
        }
        changes.push(Change {
            file: "decisions.md".into(),
            diff: written,
            why: edit.why,
        });
        for id in edit.cases {
            if !named.contains(&id) {
                named.push(id);
            }
        }
        current = next;
    }
    if version(decisions(member_dir)?.as_bytes()) != base {
        return Err(Error::Invalid(text::DECISIONS_CHANGED.into()));
    }
    let day = now
        .get(..10)
        .ok_or_else(|| Error::Invalid(text::REPLY_SHAPE.into()))?;
    let proposal = Proposal {
        schema: PROPOSAL_V1.into(),
        id: store.next_id(Prefix::Proposal, day)?,
        persona: member.slug.clone(),
        persona_kind: PersonaKind::Person,
        created: now.into(),
        base,
        summary,
        changes,
        cases: named,
    };
    check_stored(&proposal).map_err(|_| Error::Invalid(text::DECISIONS_HIDDEN.into()))?;
    store.save_proposal_unlocked(&proposal)?;
    Ok(Proposed::Drafted(proposal))
}

pub(crate) fn check_stored(proposal: &Proposal) -> Result<()> {
    if proposal.summary.chars().any(hidden)
        || proposal.changes.iter().any(|c| {
            c.why.chars().any(hidden)
                || c.diff
                    .chars()
                    .any(|c| !matches!(c, '\n' | '\r' | '<') && hidden(c))
        })
    {
        return Err(Error::Invalid(text::DAMAGED_PROPOSAL.into()));
    }
    Ok(())
}
pub fn waiting(store: &Store, member_dir: &Path) -> Result<Option<(Proposal, bool)>> {
    let base = version(decisions(member_dir)?.as_bytes());
    let found = pending(store, &base)?;
    if let Some((p, _)) = &found {
        check_stored(p)?;
    }
    Ok(found)
}
pub fn open(store: &Store, member_dir: &Path) -> Result<Option<Proposal>> {
    Ok(waiting(store, member_dir)?.map(|(p, _)| p))
}

pub fn show(store: &Store, member_dir: &Path, id: Option<&str>) -> Result<Option<Proposal>> {
    let found = match id {
        Some(id) => Some(
            store
                .proposal(id)?
                .ok_or_else(|| Error::Invalid(text::no_proposal(id)))?,
        ),
        None => open(store, member_dir)?,
    };
    if let Some(p) = &found {
        check_stored(p)?;
    }
    Ok(found)
}
pub fn render(proposal: &Proposal, member: &Path) -> Vec<String> {
    // Public callers cannot accidentally print untrusted control characters either.
    if check_stored(proposal).is_err() {
        return vec![text::DAMAGED_PROPOSAL.into()];
    }
    let mut out = vec![
        text::refresh_heading(&proposal.created, &proposal.id),
        proposal.summary.clone(),
    ];
    for (at, change) in proposal.changes.iter().enumerate() {
        out.push(format!("{}. {}", at + 1, change.why));
        out.extend(change.diff.lines().map(str::to_owned));
    }
    out.push(text::learned(proposal.cases.len()));
    out.extend(text::refresh_commands(member, &proposal.id));
    out
}
pub fn answer(
    store: &Store,
    member_dir: &Path,
    id: &str,
    decision: Decision,
    now: &str,
) -> Result<Approval> {
    let _lock = store.lock()?;
    answer_locked(store, member_dir, id, decision, now)
}

/// The caller holds the assistant's state lock.
pub fn answer_locked(
    store: &Store,
    member_dir: &Path,
    id: &str,
    decision: Decision,
    now: &str,
) -> Result<Approval> {
    let proposal = store
        .proposal(id)?
        .ok_or_else(|| Error::Invalid(text::no_proposal(id)))?;
    if decision == Decision::Approved {
        rules::base_is_current(&proposal, &version(decisions(member_dir)?.as_bytes()))?;
        rules::to_measure(&proposal, &store.cases()?)?;
    }
    let day = now
        .get(..10)
        .ok_or_else(|| Error::Invalid(text::REPLY_SHAPE.into()))?;
    let approval = Approval {
        schema: APPROVAL_V1.into(),
        id: store.next_id(Prefix::Approval, day)?,
        proposal: id.into(),
        at: now.into(),
        role: GivenBy::Subject,
        decision,
        note: None,
    };
    store.save_approval_unlocked(&approval)?;
    Ok(approval)
}
