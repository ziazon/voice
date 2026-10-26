//! Every sentence the agent crate shows a person: plain words, no tool words.
use std::path::Path;

pub const NOT_INSTALLED: &str = "Persona Builder works through your own Claude Code, and it isn't on this Mac yet. Install it (https://code.claude.com/docs/en/setup), sign in with your own Claude account, then try again. Claude Code needs a paid Claude plan; the free plan doesn't include it.";
pub const DOES_NOT_RUN: &str = "Claude Code is on this Mac but didn't start. Open it once on its own to check it works, then try again.";
pub const SIGNED_OUT: &str = "Claude Code isn't signed in. Open Claude Code, sign in with your own Claude account, then try again.";
pub const FAILED: &str = "Claude Code couldn't finish this step. Open Claude Code and check it answers you, then try again.";

pub const API_KEY_SET: &str = "ANTHROPIC_API_KEY is set on this Mac, so Claude Code may use that API key, billed separately, instead of your Claude plan.";

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
    format!("Today, about ${spent:.2} of the ${cap:.2} daily limit, as Claude Code estimates it.")
}
pub fn cut_off(minutes: u64) -> String {
    let unit = if minutes == 1 { "minute" } else { "minutes" };
    format!(
        "Claude Code was still working on this step after {minutes} {unit}, so Persona Builder stopped it. Try again; if it happens again, try a smaller step."
    )
}

pub const STOPPED: &str = "Persona Builder stopped Claude Code, as you asked.";
