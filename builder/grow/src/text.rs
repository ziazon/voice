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
        "{id} was drafted for a different version. Draft a new proposal against the current version."
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
