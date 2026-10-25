pub fn shape(kind: &str, line: usize, column: usize) -> String {
    format!("the {kind} isn't in the growth loop's {kind} shape (line {line}, column {column})")
}

pub fn field(kind: &str, field: &str, rule: &str) -> String {
    format!("the {kind}'s {field} {rule}")
}

pub fn rule(id: &str, rule: &str) -> String {
    format!("{id} {rule}")
}
pub const NOT_APPROVED: &str = "must have an approved answer from the right role for this proposal";
pub const NOT_NEW: &str = "must exist and be new before it can be measured for this proposal";
pub const WRONG_MEASUREMENT: &str =
    "must name this proposal, its approval and its base, and give the outcome its results require";
pub const MISSING_REFERENCE: &str = "must refer to records kept for this persona";
pub const ALREADY_APPROVED: &str = "already has an approval";
pub const REJECTED: &str = "was rejected, so it cannot have a measurement";
pub fn base_moved(id: &str) -> String {
    format!(
        "{id} was drafted for a different version. Draft a new change with `persona-builder refresh propose`."
    )
}
pub fn case_changed(id: &str) -> String {
    format!("case {id} was appended again with a change other than its status")
}
pub fn status_move(id: &str, before: &str, after: &str) -> String {
    format!("case {id} cannot move from {before} to {after}")
}
pub const VERSION: &str = "must name version 1";
pub const FORMAT: &str = "must follow its growth loop format";
pub const NONEMPTY: &str = "must not be empty";
pub const VERDICT: &str = "must be present exactly when kind is verdict";
pub const ONE_ENTRY: &str = "must hold at least one entry";
pub const UNIQUE: &str = "must not repeat an entry";
pub const IDS: &str =
    "must hold ids with their fixed prefix followed by lowercase letters, digits and dashes";
pub const TWO_CHOICES: &str = "must hold at least two choices";
pub const NONEMPTY_CHOICE: &str = "must not hold an empty choice";
pub const CHOICE_OPTIONS: &str = "must be present for a choice";
pub const EXPECTED_CHOICE: &str = "must be one of the options";
pub const CASE_ONCE: &str = "must name each case once";

pub const SLUG_SHAPE: &str =
    "The assistant's slug must use lowercase letters, digits and single dashes between words.";
pub fn damaged(file: &str, line: usize) -> String {
    format!("The kept file {file} is damaged at line {line}. Nothing was added.")
}
pub fn already_kept(id: &str) -> String {
    format!("{id} is already kept and cannot be replaced.")
}
pub const FILLED_IN: &str = "Persona Builder fills in the answer's schema, id, persona and time. Leave those out and try again.";
pub const TOO_BIG: &str = "That answer is too big to keep. Keep it under 64 KiB and try again.";
pub const INPUT_SHAPE: &str =
    "That answer isn't in the expected shape. Check its fields and try again.";
pub fn unknown_key(key: &str) -> String {
    if key.is_empty() || key.len() > 40 || !key.bytes().all(|c| c.is_ascii_lowercase() || c == b'_')
    {
        return "The answer has a field Persona Builder doesn't know. Leave it out and try again."
            .into();
    }
    format!("The answer has an unknown field: {key}. Leave it out and try again.")
}
pub fn unknown_area(categories: &[String]) -> String {
    format!(
        "Choose one of your assistant's areas: {}.",
        categories.join(", ")
    )
}
pub fn invalid_text(field: &str, limit: usize) -> String {
    let limit = limit.to_string();
    let mut grouped = String::new();
    for (index, digit) in limit.chars().enumerate() {
        if index > 0 && (limit.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let label = match field {
        "area" => "The area",
        "situation" => "What was being decided",
        "wanted" => "The answer you wanted",
        "persona_said" => "The assistant's answer",
        _ => "Where the answer came from",
    };
    format!("{label} must be 1 to {grouped} characters long, with no hidden characters.")
}
fn field_name(field: &str) -> &str {
    match field {
        "area" => "area",
        "situation" => "what was being decided",
        "wanted" => "answer you wanted",
        "persona_said" => "assistant's answer",
        _ => "source",
    }
}
pub fn left_out(field: &str, kind: &str) -> String {
    format!(
        "That answer looks {kind} (in {}), so it wasn't kept. Leave out anything private and try again.",
        field_name(field)
    )
}
pub const NO_CASES: &str = "You haven't given your assistant any answers to learn from yet.";
pub fn cases_summary(book: &crate::rules::CaseBook, categories: &[String]) -> String {
    use crate::record::Status;
    let counts = |area: Option<&str>| {
        let mut n = [0usize; 3];
        for c in book.iter().filter(|c| area.is_none_or(|a| c.area == a)) {
            n[match c.status {
                Status::New => 0,
                Status::Active => 1,
                Status::Retired => 2,
            }] += 1;
        }
        n
    };
    let line = |n: [usize; 3]| {
        format!(
            "{} waiting for a refresh, {} in use, {} retired.",
            n[0], n[1], n[2]
        )
    };
    let total = counts(None);
    if total == [0; 3] {
        return NO_CASES.into();
    }
    let mut text = format!("Tests made from your answers: {}", line(total));
    for area in categories {
        let n = counts(Some(area));
        if n != [0; 3] {
            text.push_str(&format!("\n{area}: {}", line(n)));
        }
    }
    text
}

pub const WRONG_ROLE: &str = "must come from the approver role named by its proposal";

pub const TOO_MANY_IDS: &str =
    "There are too many kept records for this day. Try again on another day.";

pub const KEPT: &str =
    "Kept your answer. It's now one of the tests your assistant is checked on at its next refresh.";

pub const NOT_BUILT: &str = "Build your assistant first: there's nothing to refresh yet.";
pub const DIFF_DOES_NOT_FIT: &str =
    "That saved change doesn't fit your assistant's current decisions.";
pub const DIFF_TOO_BIG: &str = "That change is too large to check. Try a smaller change.";
pub const EDIT_DOES_NOT_FIT: &str = "That point doesn't fit your assistant's current decisions.";
pub const DRAFTING: &str = "Drafting a change from your answers";
pub const NOTHING_TO_REFRESH: &str =
    "There's nothing new to learn from yet. Keep answering your assistant, then try again.";
pub const NO_OPEN: &str = "No change is waiting for your answer.";
pub const DAMAGED_PROPOSAL: &str = "That saved change looks damaged, so it wasn't shown. Draft a new one with `persona-builder refresh propose`.";
pub const DIFF_BUG: &str = "The drafted change couldn't be checked, so nothing was saved.";
pub const REPLY_SHAPE: &str = "The drafted change didn't have the expected shape.";
pub const EDIT_COUNT: &str = "The drafted change needs between 1 and 10 points.";
pub const REPLY_AREA: &str = "A point names an unknown area.";
pub const REPLY_OLD: &str = "A point doesn't name the earlier point it replaces correctly.";
pub const REPLY_CASES: &str = "A point must name your answers once each, from the ones given.";
pub const REPLY_REPLACE: &str = "Two points replace the same earlier point.";
pub const REPLY_APPLY: &str = "A point doesn't fit the current decisions.";
pub const REPLY_PRIVATE: &str = "A point looks personal or secret; leave that detail out.";
pub const REPLY_RECORDS: &str = "The saved answers couldn't be checked for repeated words.";
pub fn reply_line(place: &str) -> String {
    format!("The {place} must be a short, plain line of at most 400 characters.")
}
pub fn quoted(count: usize) -> String {
    format!("{count} runs of 6 or more of your own words")
}
pub fn open_waiting(id: &str) -> String {
    format!(
        "A change is already waiting for your answer ({id}). Look at it with `persona-builder refresh show`, then approve or reject it."
    )
}
pub fn open_approved(id: &str) -> String {
    format!("You approved a change ({id}) that hasn't been tested and applied yet.")
}
pub fn approved(_id: &str) -> String {
    "Approved. Your assistant changes only after the change is tested on all your answers and nothing gets worse.".into()
}
pub fn rejected(_id: &str) -> String {
    "Rejected. Nothing changed. Your answers stay saved for the next refresh.".into()
}
pub fn no_proposal(id: &str) -> String {
    format!("No saved change has that id ({id}).")
}
pub fn refresh_heading(date: &str, id: &str) -> String {
    let date = date.get(..10).unwrap_or(date);
    format!("Change drafted {date} ({id})")
}
pub fn learned(count: usize) -> String {
    format!("Learned from {count} of your answers.")
}
pub fn refresh_commands(member: &std::path::Path, id: &str) -> [String; 2] {
    [
        format!("persona-builder refresh approve {} {id}", member.display()),
        format!("persona-builder refresh reject {} {id}", member.display()),
    ]
}

pub const DECISIONS_HIDDEN: &str = "Your assistant's decisions.md has a hidden character near the change, so nothing was saved. Remove it, then try again.";

pub fn refresh_misfit(what: &str, problem: &str) -> String {
    format!(
        "{what}: Claude's draft didn't fit twice, so nothing was saved ({problem}). Run `persona-builder refresh propose` again."
    )
}

pub const DECISIONS_CHANGED: &str = "Your assistant's decisions changed while the change was being drafted, so nothing was saved. Try again.";
