use super::*;
use crate::{
    crew::{NewMember, new_member},
    error::Error,
    test_support::{Scratch, repo},
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

const DECISIONS: &str = "# Decisions\n\n## Calibration (the blind backtest)\n\n| Kind | High |\n|---|---|\n| scope | 6/6 |\n| sequencing | 5/6 |\n\n## Other\n| ignored |\n\n## By kind of question\n\n### scope\n\nKeep the first draft small.\nAdd detail after feedback.\n\n### sequencing\n\nStart with the unknown.\nSave polish for later.\n\n### writing-voice\n\nUse everyday words.\nLead with the choice.\n\n## Outside\n### ignored\nDo not export this.\n";

fn fixture(scratch: &Scratch) -> PathBuf {
    let member = scratch.path().join("ari");
    new_member(
        &repo().join("crew/_template"),
        &member,
        &NewMember {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
            repo_shown: "~/voice",
            date: "2026-10-24",
        },
    )
    .unwrap();
    fs::write(member.join("decisions.md"), DECISIONS).unwrap();
    member
}

fn make(member: &Path, out: &Path) -> Vec<PathBuf> {
    let result = pack(member, out);
    assert!(result.is_ok(), "{result:?}");
    let paths = result.unwrap();
    assert!(
        out.join("instructions.md").is_file(),
        "instructions.md was not written"
    );
    paths
}

#[test]
fn exact_pack_text_and_private_modes() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    let out = scratch.path().join("pack");
    let paths = make(&member, &out);
    assert_eq!(
        paths,
        [
            "instructions.md",
            "decisions-scope.md",
            "decisions-sequencing.md",
            "decisions-writing-voice.md"
        ]
        .map(|name| out.join(name))
    );
    assert_eq!(
        fs::read_to_string(out.join("instructions.md")).unwrap(),
        INSTRUCTIONS
    );
    assert_eq!(
        fs::read_to_string(out.join("decisions-scope.md")).unwrap(),
        "# How Robin decides: scope\n\nKeep the first draft small.\nAdd detail after feedback.\n"
    );
    assert_eq!(
        fs::metadata(&out).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for path in paths {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn same_member_gives_identical_bytes_and_normalizes_crlf() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    let first = make(&member, &scratch.path().join("first"));
    let second = make(&member, &scratch.path().join("second"));
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap());
    }
    for name in ["decisions.md", "IDENTITY.md", "autonomy.md"] {
        let path = member.join(name);
        let text = fs::read_to_string(&path).unwrap();
        fs::write(path, text.replace('\n', "\r\n")).unwrap();
    }
    let crlf = make(&member, &scratch.path().join("crlf"));
    for (a, b) in first.iter().zip(crlf.iter()) {
        assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap());
    }
}

#[test]
fn deleted_and_blank_areas_are_absent() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    let text = DECISIONS.replace(
        "### sequencing\n\nStart with the unknown.\nSave polish for later.\n\n",
        "### blank\n \t\n\n",
    );
    fs::write(member.join("decisions.md"), text).unwrap();
    let out = scratch.path().join("pack");
    let paths = make(&member, &out);
    assert_eq!(paths.len(), 3);
    let instructions = fs::read_to_string(out.join("instructions.md")).unwrap();
    for name in ["sequencing", "blank"] {
        assert!(!out.join(format!("decisions-{name}.md")).exists());
        assert!(!instructions.contains(&format!("decisions-{name}.md")));
    }
}

#[test]
fn no_areas_refused_without_creating_out() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    for text in [
        "## By kind of question\n",
        "## By kind of question\n### scope\n \t\n",
    ] {
        fs::write(member.join("decisions.md"), text).unwrap();
        let out = scratch.path().join("pack");
        let result = pack(&member, &out);
        assert!(
            matches!(result, Err(Error::Invalid(ref message)) if message == "Ari has no kinds of question yet. Build Ari first, then make the pack"),
            "{result:?}"
        );
        assert!(!out.exists());
    }
}

#[test]
fn header_only_calibration_is_not_measured() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    fs::write(
        member.join("decisions.md"),
        DECISIONS.replace("| scope | 6/6 |\n| sequencing | 5/6 |\n", ""),
    )
    .unwrap();
    let out = scratch.path().join("pack");
    make(&member, &out);
    let text = fs::read_to_string(out.join("instructions.md")).unwrap();
    assert!(text.contains("## How often Ari has been right\n\nNot measured yet. Treat every answer as Medium at most.\n"));
    assert!(!text.contains("| Kind |"));
}

#[test]
fn invalid_and_repeated_categories_refused_without_writes() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    for (heading, message) in [
        (
            "Not A Kind",
            "decisions.md: \"Not A Kind\" is not a kind of question (lower-case letters, digits and -)",
        ),
        ("scope", "decisions.md: scope appears twice"),
        (
            "",
            "decisions.md: \"\" is not a kind of question (lower-case letters, digits and -)",
        ),
    ] {
        fs::write(
            member.join("decisions.md"),
            DECISIONS.replace("### sequencing", &format!("### {heading}")),
        )
        .unwrap();
        let out = scratch.path().join("pack");
        let result = pack(&member, &out);
        assert!(
            matches!(result, Err(Error::Invalid(ref text)) if text == message),
            "{result:?}"
        );
        assert!(!out.exists());
    }
}

#[test]
fn nonempty_and_symlink_outputs_are_refused_untouched() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    let out = scratch.path().join("pack");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("keep"), "keep these bytes").unwrap();
    let result = pack(&member, &out);
    assert!(
        matches!(result, Err(Error::Invalid(ref text)) if text.contains(&out.display().to_string())),
        "{result:?}"
    );
    assert_eq!(fs::read_dir(&out).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(out.join("keep")).unwrap(),
        "keep these bytes"
    );
    let link = scratch.path().join("link");
    symlink(&out, &link).unwrap();
    let result = pack(&member, &link);
    assert!(
        matches!(result, Err(Error::Invalid(ref text)) if text == &format!("{}: symlink refused", link.display())),
        "{result:?}"
    );
    assert!(fs::symlink_metadata(link).unwrap().is_symlink());
    assert_eq!(fs::read_dir(&out).unwrap().count(), 1);
}

#[test]
fn private_config_is_not_exported_and_empty_output_is_accepted() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    fs::write(
        member.join("private.json"),
        r#"{"filters":{"personal":"FICTIONAL_PRIVATE_MARKER"}}"#,
    )
    .unwrap();
    let out = scratch.path().join("pack");
    fs::create_dir(&out).unwrap();
    let paths = make(&member, &out);
    for path in paths {
        assert!(
            !fs::read_to_string(path)
                .unwrap()
                .contains("FICTIONAL_PRIVATE_MARKER")
        );
    }
}

#[test]
fn empty_dial_lists_and_middle_where_sections() {
    let scratch = Scratch::new();
    let member = fixture(&scratch);
    fs::write(member.join("autonomy.md"), "# No dials\n").unwrap();
    fs::write(
        member.join("IDENTITY.md"),
        "# Ari\n\nHello.\n\n## Where Ari lives\n\nOmit this.\n\n## Keep\n\nKeep this.\n",
    )
    .unwrap();
    let out = scratch.path().join("pack");
    make(&member, &out);
    let text = fs::read_to_string(out.join("instructions.md")).unwrap();
    assert!(text.starts_with("# Ari\n\nHello.\n\n### Keep\n\nKeep this.\n\n"));
    assert!(text.contains("choice to Robin:\n\n- none\n\nYou may decide these yourself, but only when you are at High:\n\n- none\n"));
}

const INSTRUCTIONS: &str = r#"# Ari

Ari (they/them) is Robin's assistant for decisions. When a question comes up in
Robin's work, Ari answers it the way Robin would, says how sure that answer
is, and hands the question back whenever it is Robin's to decide.

Ari learned this from Robin's own answers and messages to Claude, starting 2026-10-24.

### How Ari works

- **Says how sure the answer is.** Every answer comes with how confident Ari is and how often
  Ari has been right on questions like it. When Ari is not sure, Ari says so plainly.
- **Hands back what is Robin's.** Some decisions always stay with Robin; the list
  is in the list under "What Robin decides". On those, Ari says what Robin would likely pick and
  why, then leaves the choice to Robin.
- **Talks like a colleague.** Plain, warm words, in Robin's own way of speaking and the
  words of Robin's field. A teacher's assistant sounds like someone who works in a school,
  not like an engineer.
- **Grows with Robin.** Every answer and every correction Robin gives teaches
  Ari something, and Robin can read what Ari has learned in the files beside this
  one.

### What Ari never does

- Send, post, submit, sign or agree to anything **as Robin**. Ari drafts and
  suggests; Robin sends.
- Decide something the list under "What Robin decides" keeps for Robin.
- Look into, learn from or write about Robin's private life (health, family, legal matters,
  household money), anyone else's private conversations, or details that could identify another
  person.
- Pretend to be sure.

## How to answer

1. Find the kind of question. Each kind has a file in this Project's knowledge that says how
   Robin usually leans, what changes the answer, and past examples.
2. Pick what Robin would pick. A rule Robin has stated comes first, then the past
   examples, then the usual lean.
3. Say how sure you are, in one word:
   - **High:** a stated rule settles it, or the examples agree with the usual lean, and the table
     below shows a good record on that kind of question.
   - **Medium:** the examples mostly agree, or there is no good record on that kind yet.
   - **Low:** few or no examples, or they disagree.
   Never more than Medium when Robin would likely answer in their own words rather than
   pick one of the options.
4. Hand the question back when the list below says Robin decides that kind, or when you
   are not at High. Say what Robin would likely pick and why, and leave the choice to
   Robin.

Answer in this shape:

Ari · <kind of question> · <High, Medium or Low>
Pick: <the option, or what Robin would write instead>
Why: <one or two lines>
Yours to decide: no | yes, because <the reason>

When Robin talks to you directly, answer in the first person, short and plain.

## What Robin decides

Robin always decides these. Say what Robin would likely pick and why, then leave the choice to Robin:

- `send-external`: any email or message to another person: colleagues, clients, partners, anyone
- `submit`: sending in or finalising an application, a document, a manuscript or a form
- `commitment`: agreeing to a deadline, a role, a review, a collaboration or a request for help
- `irreversible`: deleting files or history, overwriting a shared document, anything that can't be undone
- `money`: budgets, spending, payments, rates
- `privacy`: other people's information, logins and keys, what a file or export reveals
- `reverses-ruling`: the answer would go against a rule Robin has set, or two of those rules disagree
- `no-precedent`: a kind of question Robin has not answered before
- `below-high`: Ari is not very sure, or has not yet shown a good record on this kind of question

You may decide these yourself, but only when you are at High:

- `scope`: how much to include, which items of a list, what to cut
- `sequencing`: what to do first, now or later
- `writing-voice`: wording, tone and structure in Robin's own drafts
- `tooling`: which app, tool or file layout to use for Robin's own work

## How often Ari has been right

| Kind | High |
|---|---|
| scope | 6/6 |
| sequencing | 5/6 |

## Kinds of question

Each kind has its own file in this Project's knowledge:

- `decisions-scope.md`
- `decisions-sequencing.md`
- `decisions-writing-voice.md`
"#;
