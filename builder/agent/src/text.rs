//! Every sentence the agent crate shows a person: plain words, no tool words.
use std::path::Path;

pub const NOT_INSTALLED: &str = "Persona Builder works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), sign in with your own Claude account, then try again. Claude Code needs a paid Claude plan; the free plan doesn't include it.";
pub const DOES_NOT_RUN: &str = "Claude Code is on this computer but didn't start. Open it once on its own to check it works, then try again.";
pub const SIGNED_OUT: &str = "Claude Code isn't signed in for Persona Builder. Sign in on Connect Claude, or with `persona-builder claude sign-in`, then try again.";
pub const PLAN_STOPPED: &str = "Your Claude plan is nearly used up, so Persona Builder has stopped for today and left the rest for you. Run the build again tomorrow to carry on.";
pub const FAILED: &str = "Claude Code couldn't finish this step. Open Claude Code and check it answers you, then try again.";

pub const API_KEY_SET: &str = "ANTHROPIC_API_KEY is set on this computer, so Claude Code may use that API key, billed separately, instead of your Claude plan.";

pub fn ready(version: &str) -> String {
    format!("Claude Code {version} is installed and signed in.")
}
pub fn work_folder_not_empty(path: &Path) -> String {
    format!(
        "Persona Builder's working folder for this assistant ({}) has files in it that it didn't put there, so it stopped. Empty that folder, then try again.",
        path.display()
    )
}
pub fn ledger_damaged(path: &Path) -> String {
    format!(
        "Persona Builder's record of what it has spent for this assistant can't be read, so it won't spend anything more. The record is {}.",
        path.display()
    )
}
pub fn cap_reached(spent: f64, cap: f64) -> String {
    format!(
        "Today's spending limit for this assistant is reached: about ${spent:.2} of ${cap:.2}, as Claude Code estimates it. The build can carry on tomorrow, or you can raise the limit."
    )
}
pub fn spend(spent: f64, cap: f64) -> String {
    if cap == 0.0 {
        format!("Today, about ${spent:.2}, with no daily limit, as Claude Code estimates it.")
    } else {
        format!(
            "Today, about ${spent:.2} of the ${cap:.2} daily limit, as Claude Code estimates it."
        )
    }
}
pub fn cut_off(minutes: u64) -> String {
    let unit = if minutes == 1 { "minute" } else { "minutes" };
    format!(
        "Claude Code was still working on this step after {minutes} {unit}, so Persona Builder stopped it. Try again; if it happens again, try a smaller step."
    )
}

pub const STOPPED: &str = "Persona Builder stopped Claude Code, as you asked.";

pub fn limit_damaged(path: &Path) -> String {
    format!(
        "The daily limit saved at {} can't be read, so Persona Builder won't spend anything. Set it again with `persona-builder limit set`, or on Connect Claude in the app.",
        path.display()
    )
}
pub fn limit(cap: crate::ledger::Cap) -> String {
    match (cap.daily_limit(), cap.call_limit()) {
        (Some(daily), Some(call)) => format!(
            "Daily limit: about ${daily:.2} a day, and at most ${call:.2} for one step, as Claude Code estimates it."
        ),
        (None, Some(call)) => format!(
            "No daily limit, and at most ${call:.2} for one step, as Claude Code estimates it."
        ),
        (Some(daily), None) => format!(
            "Daily limit: about ${daily:.2} a day, and no separate limit for one step, as Claude Code estimates it."
        ),
        (None, None) => {
            "No limit: Persona Builder doesn't stop for the day or for one step.".into()
        }
    }
}

pub const NO_STATE_FOR_LIMIT: &str = "This assistant has no private folder yet, so there's nowhere to keep its limit. Run `persona-builder consent agree` first.";

pub const SIGN_IN_CLI: &str = "Run persona-builder claude sign-in to sign in.";
pub const SIGN_IN_WAITING: &str = "Finish signing in on the Claude page that opened in your browser. Persona Builder carries on when you're done.";
pub const SIGN_IN_FAILED: &str = "Claude Code didn't finish signing in. Try again. If no Claude page opened in your browser, check that this computer has a default browser.";
pub const SIGN_IN_TIMED_OUT: &str =
    "Signing in took more than 10 minutes, so Persona Builder stopped waiting. Try again.";
pub const SIGN_IN_STOPPED: &str =
    "You stopped signing in, so Claude Code still isn't signed in for Persona Builder.";
pub const PROFILE_FAILED: &str = "Persona Builder couldn't set up its own Claude Code folder. Check there's space on this computer, then try again.";
pub fn signed_out_at(version: &str, program: &str) -> String {
    format!(
        "Claude Code {version} at {program} isn't signed in for Persona Builder yet. Persona Builder keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with your Claude account."
    )
}
pub fn shown_program(program: &Path, home: &Path) -> String {
    match program.strip_prefix(home) {
        Ok(rest) => Path::new("~").join(rest).display().to_string(),
        Err(_) => program.display().to_string(),
    }
}
