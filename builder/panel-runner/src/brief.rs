//! Fence every untrusted part of an assistant's turn as data.
use crate::{
    guard::{forwarded, scrub},
    run::Convened,
    text,
};
use persona_builder::member::Member;
use std::collections::BTreeMap;
use voice_panels::{DeliberationState, RoundReport, clean, fence};

fn section(parts: &mut Vec<String>, label: &str, value: &str) {
    let fenced = fence(label, value).or_else(|_| fence(text::POST, &format!("{label}\n{value}")));
    if let Ok(fenced) = fenced {
        parts.push(fenced);
    }
}
/// The system instructions and prompt for one turn.
#[allow(clippy::too_many_arguments)]
pub fn make_brief(
    seat: &str,
    member: &Member,
    decisions: &str,
    state: &DeliberationState,
    convene: &Convened,
    previous: &BTreeMap<String, String>,
    steers: &[String],
    reports: &[RoundReport],
    withheld: &BTreeMap<String, Vec<String>>,
) -> (String, String) {
    let system = text::brief_system(
        &clean(&member.name, 64),
        &clean(&member.principal, 64),
        state,
    );
    let mut parts = vec![text::round_started(state.round_no.saturating_add(1))];
    section(&mut parts, text::QUESTION, &convene.input.question);
    if !convene.input.background.is_empty() {
        section(&mut parts, text::BACKGROUND, &convene.input.background);
    }
    section(&mut parts, text::DECISIONS, decisions);
    if let Some(previous) = &convene.where_left_off {
        section(&mut parts, text::WHERE_LEFT_OFF, &scrub(previous));
    }
    if let Some(categories) = withheld
        .get(seat)
        .filter(|categories| !categories.is_empty())
    {
        parts.push(text::withheld(state.round_no, categories));
    }
    if state.round_no > 0 {
        for name in &state.crew {
            let post = previous
                .get(name)
                .filter(|post| !post.is_empty())
                .map_or(text::NO_POST, String::as_str);
            section(
                &mut parts,
                &text::post_label(state.round_no, name),
                &forwarded(post),
            );
        }
    }
    for steer in steers {
        section(&mut parts, text::STEER, steer);
    }
    for report in reports.iter().skip(reports.len().saturating_sub(2)) {
        parts.push(text::distances(
            report.round,
            report.d,
            report.own.get(seat).copied().unwrap_or_default(),
        ));
    }
    if state.holdout.get(seat).copied().unwrap_or_default() {
        parts.push(text::holdout(
            state.weight.get(seat).copied().unwrap_or_default(),
        ));
    }
    if let Some(Some(block)) = state.last_block.get(seat)
        && let Ok(block) = serde_json::to_string(block)
    {
        section(&mut parts, text::LAST_BLOCK, &scrub(&block));
    }
    (system, parts.join("\n\n") + "\n")
}
