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

pub const REPORT_BAD_NAME: &str = "A kind of decision in your assistant's settings (codebook.categories) has a name that is empty, longer than 100 characters, or holds a character that can't be shown, so nothing was saved. Rename it, then try again.";

pub const REPORT_UNREADABLE: &str = "Your last report couldn't be read, so it isn't shown. Build your assistant again to make a new one.";

pub fn no_report(folder: &str) -> String {
    format!("There's no report yet. Build your assistant first: persona-builder build {folder}")
}
pub fn report_from(day: &str) -> String {
    format!("From the build on {day}:")
}

pub const REPORT_ENOUGH_HISTORY: &str =
    "There's enough history to learn from, but too little to test yet.";
pub const REPORT_INTERVIEW: &str =
    "The interview fills out a thin history: persona-builder interview draft";
pub const REPORT_AREAS: &str = "By kind of decision:";
pub const REPORT_NO_AREAS: &str = "By kind of decision: none yet.";
pub const REPORT_STRENGTHEN_INTERVIEW: &str =
    "answer the interview (persona-builder interview draft)";
pub const REPORT_STRENGTHEN_CLAUDEAI: &str = "add your claude.ai chats (persona-builder claudeai)";
pub const REPORT_STRENGTHEN_TIME: &str = "keep using Claude, since it learns from every answer";

pub fn report_level(level: crate::report::Level) -> &'static str {
    match level {
        crate::report::Level::Strong => "Strong",
        crate::report::Level::GettingThere => "Getting there",
        crate::report::Level::Thin => "Thin",
    }
}

pub fn report_heading(level: &str) -> String {
    format!("How well your assistant knows you: {level}.")
}

pub fn report_thin_history(answered: usize) -> String {
    format!(
        "Your history holds {answered} answered questions. It takes about 150 to know you well."
    )
}

pub fn report_tested(strong: usize, areas: usize, right: usize, of: usize) -> String {
    format!(
        "It passed the test on {strong} of {areas} kinds of decision, and got {right} of {of} test questions right."
    )
}

pub fn report_untested(held: usize) -> String {
    format!(
        "Only {held} questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Until it can be tested, it decides nothing on its own."
    )
}

pub fn report_area(area: &str, level: &str) -> String {
    format!("- {area}: {level}")
}

pub fn report_examples(examples: usize) -> String {
    let noun = if examples == 1 { "example" } else { "examples" };
    format!(", {examples} {noun} so far")
}

pub fn report_strengthen(ways: &[&str]) -> String {
    format!(" To strengthen it: {}.", ways.join("; "))
}
