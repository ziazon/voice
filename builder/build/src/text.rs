//! Every sentence the build shows a person, in plain words.

pub const ALREADY_DRAFTED: &str = "Your interview questions are already drafted and waiting for your answers. Answer them, or add --replace to draft new ones.";
pub const NO_ANSWERS: &str =
    "None of the interview questions has an answer yet, so nothing was saved.";
pub const ALL_LEFT_OUT: &str =
    "Every answer looked personal or secret, so none was kept and nothing was saved.";
pub const NOTHING_TO_ASK: &str = "Your assistant's settings name no kinds of decision to ask about (codebook.categories holds only other).";

pub fn enough(answered: usize) -> String {
    format!(
        "Your history already holds {answered} answered questions, enough to build from. The interview is for a thinner history."
    )
}

pub fn draft_failed(problem: &str) -> String {
    format!(
        "Claude drafted interview questions that didn't fit the form twice, so nothing was saved (the last problem: {problem}). Try again; if it keeps happening, try another model with --model."
    )
}

pub fn drafted(count: usize, weighted: bool) -> String {
    if weighted {
        format!(
            "Drafted {count} interview questions from your areas of work, more of them on the kinds of decision your history holds least of."
        )
    } else {
        format!(
            "Drafted {count} interview questions from your areas of work, at least one on each kind of decision."
        )
    }
}

pub fn swap(folder: &str) -> String {
    format!(
        "Swap any question that doesn't fit your work, then answer them: persona-builder interview answer {folder} --answers <your answers file>"
    )
}

pub fn merge_next(folder: &str) -> String {
    format!("Next, add them to your history: persona-builder merge {folder}")
}
