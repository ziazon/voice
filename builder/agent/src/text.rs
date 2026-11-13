//! Every sentence the agent crate shows a person: plain words, no tool words.
use std::path::Path;

pub const NOT_INSTALLED: &str = "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Voice signs it in with an Anthropic Console account, which is billed for what it uses (API billing).";
pub const DOES_NOT_RUN: &str = "Claude Code is on this computer but didn't start. Open it once on its own to check it works, then try again.";
pub const SIGNED_OUT: &str = "Claude Code isn't signed in for Voice. Sign in with your Anthropic Console account on Connect Claude, or with `voice claude sign-in`, then try again.";
pub const SUBSCRIPTION_REFUSED: &str = "Claude Code is signed in for Voice with a Claude subscription, which this copy of Voice can't use. Sign in again with an Anthropic Console account on Connect Claude, or with `voice claude sign-in`.";
pub const SUBSCRIPTION_NOT_OFFERED: &str =
    "This copy of Voice signs in with an Anthropic Console account only.";

pub const PLAN_STOPPED: &str = "Your Claude plan is nearly used up, so Voice has stopped for today and left the rest for you. Run the build again tomorrow to carry on.";
pub const FAILED: &str = "Claude Code couldn't finish this step. Open Claude Code and check it answers you, then try again.";

pub fn ready(version: &str) -> String {
    format!("Claude Code {version} is installed and signed in.")
}
pub fn ledger_damaged(path: &Path) -> String {
    format!(
        "Voice's record of what it has spent for this assistant can't be read, so it won't spend anything more. The record is {}.",
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
pub fn usage(used: crate::ledger::Used) -> String {
    if used.calls == 0 {
        return "Today, no calls to Claude yet, on your Claude subscription.".into();
    }
    let calls = used.calls;
    let call = if calls == 1 { "call" } else { "calls" };
    let tokens = match used.tokens {
        1 => "1 token".into(),
        n @ 0..=999 => format!("{n} tokens"),
        n @ 1_000..=999_499 => format!("about {} thousand tokens", (n + 500) / 1_000),
        n => {
            // Round half up in tenths of a million, without overflowing at u64::MAX.
            let tenths = n / 100_000 + u64::from(n % 100_000 >= 50_000);
            let whole = tenths / 10;
            let decimal = tenths % 10;
            if decimal == 0 {
                format!("about {whole} million tokens")
            } else {
                format!("about {whole}.{decimal} million tokens")
            }
        }
    };
    format!("Today, {calls} {call} to Claude and {tokens}, on your Claude subscription.")
}

pub fn cut_off(minutes: u64) -> String {
    let unit = if minutes == 1 { "minute" } else { "minutes" };
    format!(
        "Claude Code was still working on this step after {minutes} {unit}, so Voice stopped it. Try again; if it happens again, try a smaller step."
    )
}

pub const STOPPED: &str = "Voice stopped Claude Code, as you asked.";

pub fn limit_damaged(path: &Path) -> String {
    format!(
        "The daily limit saved at {} can't be read, so Voice won't spend anything. Set it again with `voice limit set`, or on Connect Claude in the app.",
        path.display()
    )
}
pub const LIMIT_SUBSCRIPTION: &str = "Signed in with your Claude subscription, Voice has no daily limit: it stops for the day when Claude Code says your plan is nearly used up, and leaves the rest for you. The daily limit is for an Anthropic Console sign-in only.";
pub const LIMIT_SAVED_SUBSCRIPTION: &str = "Saved for an Anthropic Console sign-in. Signed in with your Claude subscription, Voice has no daily limit: it stops for the day when Claude Code says your plan is nearly used up, and leaves the rest for you.";

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
        (None, None) => "No limit: Voice doesn't stop for the day or for one step.".into(),
    }
}

pub const NO_STATE_FOR_LIMIT: &str = "This assistant has no private folder yet, so there's nowhere to keep its limit. Run `voice consent agree` first.";

pub const SIGN_OUT_FILE_LEFT: &str = "Claude Code signed out, but its sign-in file is still in Voice's folder. Try signing out again.";
pub const SIGN_OUT_SUCCESS: &str = "Signed out of Claude Code in Voice.";
pub const SIGN_OUT_DURING_SIGN_IN: &str =
    "Wait for the sign-in to finish or cancel it, then sign out.";
pub const SIGN_IN_DURING_SIGN_OUT: &str = "Wait for the sign-out to finish, then sign in.";
pub const SIGN_OUT_BUSY: &str = "Voice is already signing out.";
pub const SIGN_OUT_FAILED: &str = "Claude Code didn't finish signing out. Try again.";
#[cfg(not(feature = "claude-subscription"))]
pub const CONSOLE_NOT_AVAILABLE: &str =
    "Signing in with a Console account doesn't work on a Mac yet. A later update adds it.";
#[cfg(feature = "claude-subscription")]
pub const CONSOLE_NOT_AVAILABLE: &str = "Signing in with a Console account doesn't work on a Mac yet. A later update adds it. Use Sign in with a Claude subscription instead.";
pub const SIGN_IN_CLI: &str = "Run voice claude sign-in to sign in.";
pub const SIGN_IN_WAITING: &str =
    "Finish signing in on the page that opened in your browser. Voice carries on when you're done.";
pub const SIGN_IN_FAILED: &str = "Claude Code didn't finish signing in. Try again. If no sign-in page opened in your browser, check that this computer has a default browser.";
pub const SIGN_IN_TIMED_OUT: &str =
    "Signing in took more than 10 minutes, so Voice stopped waiting. Try again.";
pub const SIGN_IN_STOPPED: &str =
    "You stopped signing in, so Claude Code still isn't signed in for Voice.";
pub const PROFILE_FAILED: &str = "Voice couldn't set up its own Claude Code folder. Check there's space on this computer, then try again.";
pub fn signed_out_at(version: &str, program: &str) -> String {
    format!(
        "Claude Code {version} at {program} isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with an Anthropic Console account."
    )
}
pub fn shown_program(program: &Path, home: &Path) -> String {
    match program.strip_prefix(home) {
        Ok(rest) => Path::new("~").join(rest).display().to_string(),
        Err(_) => program.display().to_string(),
    }
}

/// Where a provider sends the data it receives, in words for people.
pub fn reach_words(reach: crate::provider::Reach) -> &'static str {
    match reach {
        crate::provider::Reach::OwnMachines => "your own machines",
        crate::provider::Reach::Anthropic => "Anthropic",
        crate::provider::Reach::ThirdParty => "another company",
    }
}
/// Explain why a source could not be sent to a provider.
pub fn kept_home(
    source: voice_core::ceilings::Source,
    ceiling: voice_core::ceilings::Ceiling,
    provider: &str,
    reach: crate::provider::Reach,
) -> String {
    if reach == crate::provider::Reach::ThirdParty
        && ceiling == voice_core::ceilings::Ceiling::AnyConsented
    {
        return format!(
            "Nothing was sent. This step uses {}, and the provider for this step, {provider}, sends what it's given to another company, which you haven't agreed to.",
            source.name()
        );
    }
    format!(
        "Nothing was sent. This step uses {}, which you keep {}, and the provider for this step, {provider}, sends what it's given to {}. To let it go there, run: voice ceilings set <assistant folder> {} {}",
        source.name(),
        ceiling.words(),
        reach_words(reach),
        source.id(),
        crate::guard::needed(reach).id()
    )
}

pub const SERVER_UNREACHABLE: &str = "Voice couldn't reach your model server. Check it's running and that its address is right (`voice self-hosted show`), then try again.";
pub const SERVER_NOT_THIS_COMPUTER: &str = "Nothing was sent. Your model server's address isn't this computer, and you haven't said to use your network. To use another computer on your own network, set it up again with --network.";
pub const SERVER_NOT_OWN: &str = "Nothing was sent. Your model server's address leads outside this computer and your own network, so Voice won't use it. Give it the address of a computer on your own network.";
pub const SERVER_FAILED: &str = "Your model server couldn't finish this step. Check it answers you and has the model loaded, then try again.";
pub const SERVER_STOPPED: &str = "Voice stopped before asking your model server, as you asked.";
pub const BAD_SERVER_URL: &str = "A model server's address looks like http://192.168.1.20:11434 or http://localhost:1234: http or https, a computer's name or address, and a port, with nothing after it.";
pub const BAD_SERVER_MODEL: &str = "A model's name is letters and digits, and may have . _ - : / or @ in it, up to 128 in all, like qwen3:8b.";
pub const NO_STATE_FOR_SERVER: &str = "This assistant has no private folder yet, so there's nowhere to keep its model server. Run `voice consent agree` first.";
pub const NO_SERVER: &str = "No model server is set up for this assistant, so a step with data you keep on your own machines doesn't run.";
pub const SELF_HOSTED_HINT: &str = "To run it without the data leaving your machines, set up a model server with `voice self-hosted set`.";
/// Every fixed sentence a model-server call can fail with, for callers that pass only known
/// sentences on to a person.
pub const SERVER_MESSAGES: [&str; 5] = [
    SERVER_UNREACHABLE,
    SERVER_NOT_OWN,
    SERVER_NOT_THIS_COMPUTER,
    SERVER_FAILED,
    SERVER_STOPPED,
];
pub const SERVER_CUT_OFF_START: &str = "Your model server was still working on this step after ";
pub fn server_cut_off(minutes: u64) -> String {
    let unit = if minutes == 1 { "minute" } else { "minutes" };
    format!(
        "Your model server was still working on this step after {minutes} {unit}, so Voice stopped waiting. Try again; if it happens again, try a smaller model or a smaller step."
    )
}
pub fn server_damaged(path: &Path) -> String {
    format!(
        "The model server settings saved at {} can't be read, so Voice won't use them. Set them again with `voice self-hosted set`, or remove them with `voice self-hosted off`.",
        path.display()
    )
}
pub fn no_model(model: &str) -> String {
    format!(
        "Your model server answers, but it doesn't list the model {model}. Load it on the server, or give the name the server lists."
    )
}
pub fn server(server: &crate::self_hosted::Server) -> String {
    if server.network() {
        format!(
            "Model server: {}, model {}, on your network. Steps with data you keep on your own machines go to it. Before each step Voice checks the address is on a private network, but it can't tell your network from someone else's: on other Wi-Fi, a stranger's computer could answer at the same address. Before you use other Wi-Fi, turn it off with voice self-hosted off.",
            server.url(),
            server.model()
        )
    } else {
        format!(
            "Model server: {}, model {}, on this computer. Steps with data you keep on your own machines go to it, and before each step Voice checks the address still leads to this computer.",
            server.url(),
            server.model()
        )
    }
}

pub const WINDOW_RANGE: &str =
    "The calendar window goes from 0 to 31 days back and from 1 to 92 days ahead.";
pub const AUDIT_LAST: &str = "Show between 1 and 1,000 entries.";
pub const AUDIT_STEP: &str = "A step name starts with a lowercase letter and uses only lowercase letters, digits and dashes, from 1 to 32 characters.";
pub const NO_STATE_FOR_GRANTS: &str =
    "This assistant has no private state yet. Build it once, then give permission.";
pub const GRANT_NOT_LOGGED: &str = "Voice couldn't log this, so it took back every permission for this assistant. Give permission again once there's space on this computer.";
pub const AUDIT_OLDER: &str = "Older entries aren't shown.";
pub fn grants_damaged(path: &Path) -> String {
    format!(
        "{} is damaged, so Voice reads nothing it needs permission for. Delete it and give permission again.",
        path.display()
    )
}
pub fn no_grant(capability: crate::grants::Capability) -> String {
    format!(
        "Voice doesn't have your permission to {} for this assistant.",
        capability.words()
    )
}
pub fn typed_name(name: &str) -> String {
    let mut chars = name.chars();
    let mut shown: String = chars
        .by_ref()
        .take(64)
        .map(|c| {
            if voice_ingest::text::hidden(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    if chars.next().is_some() {
        shown.push('…');
    }
    shown
}
pub fn unknown_capability(id: &str) -> String {
    let id = typed_name(id);
    let ids = crate::grants::Capability::ALL
        .map(|cap| cap.id())
        .join(", ");
    format!("{id} isn't something Voice can be given permission for. It can be given: {ids}.")
}
pub fn grant_window(window: crate::grants::Window) -> String {
    let past = match window.past_days() {
        0 => "from today".into(),
        1 => "from 1 day back".into(),
        n => format!("from {n} days back"),
    };
    let next = window.next_days();
    let unit = if next == 1 { "day" } else { "days" };
    format!("{past} to {next} {unit} ahead")
}
pub fn grant_shown(
    capability: crate::grants::Capability,
    grant: Option<&crate::grants::Grant>,
) -> String {
    let id = capability.id();
    let words = capability.words();
    match grant {
        Some(grant) => format!(
            "{id}\ton\t{words}, {}, given {}",
            grant_window(grant.window),
            grant.given_on
        ),
        None => format!("{id}\toff\t{words}"),
    }
}
pub fn grant_given(name: &str, grant: &crate::grants::Grant) -> String {
    format!(
        "Voice may now {} for {name}, {}.",
        grant.capability.words(),
        grant_window(grant.window)
    )
}
pub fn grant_revoked(name: &str, capability: crate::grants::Capability, changed: bool) -> String {
    let words = capability.words();
    if changed {
        format!("Voice may no longer {words} for {name}.")
    } else {
        format!("Voice didn't have permission to {words} for {name}, so nothing changed.")
    }
}
pub fn audit_entry(entry: &crate::grants::Entry) -> String {
    use crate::grants::{Event, Reason};
    let what = match &entry.event {
        Event::Grant { capability, window } => {
            format!("given: {}, {}", capability.words(), grant_window(*window))
        }
        Event::Revoke { capability } => format!("taken back: {}", capability.words()),
        Event::Read {
            capability,
            step,
            items,
        } => {
            let unit = if *items == 1 { "item" } else { "items" };
            format!("read: {}, {items} {unit}, for {step}", capability.words())
        }
        Event::Refused {
            capability,
            step,
            reason,
        } => {
            let reason = match reason {
                Reason::NoGrant => "no permission",
                Reason::Damaged => "the permissions file is damaged",
            };
            format!("refused: {}, for {step}, {reason}", capability.words())
        }
    };
    format!("{} UTC  {what}", entry.at[..16].replace('T', " "))
}
pub fn audit_unreadable(count: usize) -> String {
    let unit = if count == 1 { "entry" } else { "entries" };
    format!("{count} {unit} couldn't be read.")
}
pub fn audit_empty(name: &str) -> String {
    format!("Nothing has been logged for {name} yet.")
}

pub const MAC_ONLY: &str =
    "Voice runs Claude Code inside the Mac's own sandbox, so for now it works on a Mac only.";
pub const NOT_NATIVE: &str = "This Claude Code is an older kind that runs as a script, which Voice can't run inside its sandbox. Install the current Claude Code (https://code.claude.com/docs/en/setup), then try again.";
pub const RUN_FOLDER_FAILED: &str = "Voice couldn't set up a folder for Claude Code to work in. Check there's space on this computer, then try again.";
pub const PROXY_FAILED: &str = "Voice couldn't open the private connection Claude Code uses to reach Claude. Try again; if it keeps happening, restart this computer.";

pub const READER_SOURCE: &str = "Voice can't read one of the places you chose. Check it's still there and isn't a password or key file, then try again.";
pub const READER_FOLDER: &str =
    "Voice couldn't open this assistant's own folders. Close Voice and open it again.";

#[cfg(test)]
mod calendar_text_tests {
    #[test]
    fn typed_names_replace_hidden_characters() {
        for c in [
            '\u{061c}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{2061}', '\u{2062}',
            '\u{2063}', '\u{2064}', '\u{206a}', '\u{206b}', '\u{206c}', '\u{206d}', '\u{206e}',
            '\u{206f}', '\u{feff}',
        ] {
            assert_eq!(
                super::typed_name(&format!("Team{c}planning")),
                "Team planning",
                "{c:?}"
            );
        }
    }
}

#[cfg(test)]
mod usage_tests {
    #[test]
    fn usage_reports_calls_and_rounded_tokens_without_dollars() {
        for (calls, tokens, expected) in [
            (
                0,
                0,
                "Today, no calls to Claude yet, on your Claude subscription.",
            ),
            (
                1,
                1,
                "Today, 1 call to Claude and 1 token, on your Claude subscription.",
            ),
            (
                2,
                850,
                "Today, 2 calls to Claude and 850 tokens, on your Claude subscription.",
            ),
            (
                3,
                1000,
                "Today, 3 calls to Claude and about 1 thousand tokens, on your Claude subscription.",
            ),
            (
                11,
                342_310,
                "Today, 11 calls to Claude and about 342 thousand tokens, on your Claude subscription.",
            ),
            (
                4,
                999_499,
                "Today, 4 calls to Claude and about 999 thousand tokens, on your Claude subscription.",
            ),
            (
                5,
                999_600,
                "Today, 5 calls to Claude and about 1 million tokens, on your Claude subscription.",
            ),
            (
                40,
                1_240_000,
                "Today, 40 calls to Claude and about 1.2 million tokens, on your Claude subscription.",
            ),
            (
                41,
                1_250_000,
                "Today, 41 calls to Claude and about 1.3 million tokens, on your Claude subscription.",
            ),
            (
                60,
                12_000_000,
                "Today, 60 calls to Claude and about 12 million tokens, on your Claude subscription.",
            ),
            (
                1,
                0,
                "Today, 1 call to Claude and 0 tokens, on your Claude subscription.",
            ),
            (
                1,
                999_500,
                "Today, 1 call to Claude and about 1 million tokens, on your Claude subscription.",
            ),
        ] {
            assert_eq!(
                super::usage(crate::ledger::Used { calls, tokens }),
                expected
            );
        }
    }
}
