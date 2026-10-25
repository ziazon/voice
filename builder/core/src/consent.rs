pub const VERSION: u32 = 1;

/// The private permission record, shared by the app and CLI so both reject damaged records.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub version: u32,
    pub agreed_on: String,
}

pub struct Section {
    pub heading: &'static str,
    pub lines: &'static [&'static str],
}
pub const TITLE: &str = "Before Persona Builder reads anything";
pub const SECTIONS: &[Section] = &[
    Section {
        heading: "What it reads",
        lines: &[
            "The messages you typed to Claude Code on this computer, including your answers to its questions.",
            "Your claude.ai chats, only if you download them and add them yourself.",
        ],
    },
    Section {
        heading: "What stays on this computer",
        lines: &[
            "Your history, and everything Persona Builder makes from it, stays in your Claude folder on this computer.",
            "It leaves out messages that look private, such as health, family, legal or money matters and account numbers. It can miss some.",
        ],
    },
    Section {
        heading: "What goes to Claude",
        lines: &[
            "To build your assistant, Persona Builder sends parts of your history to Claude through your own Claude Code, signed in to your own Claude account. The terms of your Claude account apply, as they do to anything else you send through Claude Code.",
            "This uses your own Claude plan's usage, and a build can use a lot of it. Persona Builder stops for the day when it reaches a daily limit, and you can stop it at any time.",
        ],
    },
    Section {
        heading: "Nothing else",
        lines: &[
            "Persona Builder sends nothing anywhere else. A copy goes to GitHub only if you choose that later.",
            "You can change your mind: Forget deletes the history it read and everything it made from it.",
        ],
    },
];
/// SHA-1 (hex) of `plain_text()`. When changing the wording, bump VERSION and update this.
pub const WORDING_SHA1: &str = "aae7004e8e1404bcad07ed30f6d26659b0c493cc";
pub fn plain_text() -> String {
    let mut text = format!("{TITLE}\n\n");
    for (index, section) in SECTIONS.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(section.heading);
        text.push('\n');
        for line in section.lines {
            text.push_str(line);
            text.push('\n');
        }
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wording_matches_pinned_sha1_and_contains_every_line() {
        let text = plain_text();
        assert!(
            text.starts_with(&format!("{TITLE}\n\n")),
            "permission text must start with its title and a blank line"
        );
        assert!(text.ends_with('\n'));
        for section in SECTIONS {
            assert!(text.contains(section.heading));
            for line in section.lines {
                assert!(text.contains(line), "missing permission line: {line}");
            }
        }
        assert_eq!(
            sha1_smol::Sha1::from(text.as_bytes()).digest().to_string(),
            WORDING_SHA1,
            "permission wording changed without updating its version and SHA-1"
        );
    }
}
