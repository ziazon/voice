//! Every sentence the build shows a person, in plain words.

pub const ALREADY_DRAFTED: &str = "Your interview questions are already drafted and waiting for your answers. Answer them, or add --replace to draft new ones.";
pub const NO_ANSWERS: &str =
    "None of the interview questions has an answer yet, so nothing was saved.";
pub const ALREADY_ANSWERED: &str =
    "Your interview answers are already kept. Add --replace to keep these answers in their place.";
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
        "Swap any question that doesn't fit your work, then answer them. Save your answers in the same private folder, then: persona-builder interview answer {folder} --answers <the answers file's name>"
    )
}

pub fn merge_next(folder: &str) -> String {
    format!("Next, add them to your history: persona-builder merge {folder}")
}

pub const NOTHING_TO_BUILD: &str = "There's no history to build from yet. Read your history in first (persona-builder mine, then persona-builder merge), then try again.";
pub const NO_CODEBOOK: &str = "Your assistant's folder has no briefs/codebook.md, which says what each kind of decision means. Put it back from crew/_template/briefs/codebook.md, then try again.";
pub const CHECKING: &str = "Checking the draft for anything private before it's saved.";

pub fn reading(answered: usize, held: usize) -> String {
    format!(
        "Reading your history: {answered} answered questions, {held} of them kept back to test your assistant."
    )
}

pub fn labelling(at: usize, of: usize) -> String {
    format!("Sorting your decisions by kind, part {at} of {of}")
}

pub fn distilling(at: usize, of: usize) -> String {
    format!("Learning how you decide, group {at} of {of}")
}

pub fn testing(at: usize, of: usize, held: usize) -> String {
    format!("Testing your assistant on {held} questions it hasn't seen, batch {at} of {of}")
}

pub fn step_line(what: &str, reused: bool) -> String {
    if reused {
        format!("{what}: already done.")
    } else {
        format!("{what}: done.")
    }
}

pub fn too_few(held: usize) -> String {
    format!(
        "Only {held} questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Your assistant decides nothing on its own until it can be tested."
    )
}

pub fn misfit(what: &str, problem: &str) -> String {
    format!(
        "{what}: Claude's answer didn't fit twice, so the build stopped (the last problem: {problem}). What it already finished is kept: run the build again to carry on."
    )
}

pub fn private_hits(places: &[String]) -> String {
    format!(
        "The draft has something that looks personal or secret ({}), so nothing was saved. Run the build again; if it keeps happening, check your filters.",
        places.join(", ")
    )
}

pub fn thin_history(answered: usize) -> String {
    format!(
        "Your history holds {answered} answered questions. Fewer than 150 is thin: persona-builder interview draft adds twenty questions to fill it out."
    )
}

pub fn written(folder: &str) -> String {
    format!(
        "Saved how you decide in {folder}/decisions.md, and the test's result in {folder}/measurements.md."
    )
}

pub fn live(categories: &[String]) -> String {
    if categories.is_empty() {
        "No kind of decision passed the test yet (5 or more sure picks, at least 90% right), so your assistant decides nothing on its own yet.".to_owned()
    } else {
        format!(
            "Your assistant passed the test on: {}. It can decide those on its own where you allow it.",
            categories.join(", ")
        )
    }
}

pub fn no_group(category: &str) -> String {
    format!(
        "Your assistant's settings put the kind of decision {category} in no group (codebook.groups), so the build couldn't write about it. Add it to a group, then try again."
    )
}

pub fn too_big(what: &str, rows: usize) -> String {
    format!(
        "{what}: its {rows} records are more than one step can read at once, so the build stopped before spending on it. Splitting a large group is not built yet."
    )
}
