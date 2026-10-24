//! Files for a claude.ai Project, rendered locally from a member's files.
use crate::{
    crew::{MemberDir, fill},
    error::{Error, Result},
    install_fs::{Journal, exists, io},
    member::refuse_symlink,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

/// Write the instructions first, then each non-empty kind of question in file order.
/// Failed writes roll back only the files and directory created by this call.
pub fn pack(member_dir: &Path, out: &Path) -> Result<Vec<PathBuf>> {
    refuse_symlink(out)?;
    if exists(out)? && (!out.is_dir() || io(out, fs::read_dir(out))?.next().is_some()) {
        return Err(Error::Invalid(format!(
            "{} is not an empty folder: choose a new or empty folder for the pack",
            out.display()
        )));
    }
    let member = MemberDir::open(member_dir)?;
    for name in ["IDENTITY.md", "autonomy.md", "decisions.md"] {
        refuse_symlink(&member_dir.join(name))?;
    }
    let decisions = read(&member_dir.join("decisions.md"))?;
    let areas = areas(&decisions)?;
    if areas.is_empty() {
        return Err(Error::Invalid(format!(
            "{0} has no kinds of question yet. Build {0} first, then make the pack",
            member.member.name
        )));
    }
    let instructions = instructions(&member, &decisions, &areas)?;
    let mut journal = Journal::default();
    journal.directory(out, false)?;
    let mut paths = vec![out.join("instructions.md")];
    journal.new_file(&paths[0], &instructions)?;
    for (category, body) in areas {
        let path = out.join(format!("decisions-{category}.md"));
        let text = format!(
            "# How {} decides: {category}\n\n{body}\n",
            member.member.principal
        );
        journal.new_file(&path, &text)?;
        paths.push(path);
    }
    journal.commit();
    Ok(paths)
}

fn read(path: &Path) -> Result<String> {
    Ok(io(path, fs::read_to_string(path))?.replace("\r\n", "\n"))
}

fn section<'a>(text: &'a str, heading: &str) -> Vec<&'a str> {
    let mut lines = text.lines().skip_while(|line| *line != heading);
    lines.next();
    lines.take_while(|line| !line.starts_with("## ")).collect()
}

fn trim_blank(lines: &[&str]) -> String {
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());
    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].join("\n"),
        _ => String::new(),
    }
}

fn areas(text: &str) -> Result<Vec<(String, String)>> {
    let lines = section(text, "## By kind of question");
    let mut areas = Vec::new();
    let mut seen = HashSet::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(category) = lines[index].strip_prefix("### ") else {
            index += 1;
            continue;
        };
        if category.is_empty()
            || category.len() > 64
            || !category
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(Error::Invalid(format!(
                "decisions.md: \"{category}\" is not a kind of question (lower-case letters, digits and -)"
            )));
        }
        if !seen.insert(category) {
            return Err(Error::Invalid(format!(
                "decisions.md: {category} appears twice"
            )));
        }
        index += 1;
        let start = index;
        while index < lines.len() && !lines[index].starts_with("### ") {
            index += 1;
        }
        let body = trim_blank(&lines[start..index]);
        if !body.is_empty() {
            areas.push((category.to_owned(), body));
        }
    }
    Ok(areas)
}

fn identity(text: &str, principal: &str) -> String {
    let mut lines = text.lines().skip_while(|line| !line.starts_with("# "));
    lines.next();
    let mut kept = Vec::new();
    let mut dropping = false;
    for line in lines {
        if line.starts_with("## ") {
            dropping = line.starts_with("## Where ");
        }
        if !dropping {
            kept.push(if line.starts_with("## ") {
                format!("#{line}")
            } else {
                line.to_owned()
            });
        }
    }
    trim_blank(&kept.iter().map(String::as_str).collect::<Vec<_>>()).replace(
        "[autonomy.md](autonomy.md)",
        &format!("the list under \"What {principal} decides\""),
    )
}

fn instructions(member: &MemberDir, decisions: &str, areas: &[(String, String)]) -> Result<String> {
    let name = &member.member.name;
    let principal = &member.member.principal;
    let identity = identity(&read(&member.dir.join("IDENTITY.md"))?, principal);
    let how = fill(HOW_TO_ANSWER, &[("NAME", name), ("PRINCIPAL", principal)]);
    let dials = member.autonomy()?.dials();
    let list = |checked| {
        let lines: Vec<_> = dials
            .iter()
            .filter(|dial| dial.principal_decides == checked)
            .map(|dial| format!("- `{}`: {}", dial.key, dial.text))
            .collect();
        if lines.is_empty() {
            "- none".into()
        } else {
            lines.join("\n")
        }
    };
    let autonomy = format!(
        "## What {principal} decides\n\n{principal} always decides these. Say what {principal} would likely pick and why, then leave the choice to {principal}:\n\n{}\n\nYou may decide these yourself, but only when you are at High:\n\n{}",
        list(true),
        list(false)
    );
    let table: Vec<_> = section(decisions, "## Calibration (the blind backtest)")
        .into_iter()
        .filter(|line| line.starts_with('|'))
        .collect();
    let calibration = if table.len() < 3 {
        "Not measured yet. Treat every answer as Medium at most.".into()
    } else {
        table.join("\n")
    };
    let files = areas
        .iter()
        .map(|(category, _)| format!("- `decisions-{category}.md`"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!(
        "# {name}\n\n{identity}\n\n{how}\n\n{autonomy}\n\n## How often {name} has been right\n\n{calibration}\n\n## Kinds of question\n\nEach kind has its own file in this Project's knowledge:\n\n{files}\n"
    ))
}

const HOW_TO_ANSWER: &str = r#"## How to answer

1. Find the kind of question. Each kind has a file in this Project's knowledge that says how
   {{PRINCIPAL}} usually leans, what changes the answer, and past examples.
2. Pick what {{PRINCIPAL}} would pick. A rule {{PRINCIPAL}} has stated comes first, then the past
   examples, then the usual lean.
3. Say how sure you are, in one word:
   - **High:** a stated rule settles it, or the examples agree with the usual lean, and the table
     below shows a good record on that kind of question.
   - **Medium:** the examples mostly agree, or there is no good record on that kind yet.
   - **Low:** few or no examples, or they disagree.
   Never more than Medium when {{PRINCIPAL}} would likely answer in their own words rather than
   pick one of the options.
4. Hand the question back when the list below says {{PRINCIPAL}} decides that kind, or when you
   are not at High. Say what {{PRINCIPAL}} would likely pick and why, and leave the choice to
   {{PRINCIPAL}}.

Answer in this shape:

{{NAME}} · <kind of question> · <High, Medium or Low>
Pick: <the option, or what {{PRINCIPAL}} would write instead>
Why: <one or two lines>
Yours to decide: no | yes, because <the reason>

When {{PRINCIPAL}} talks to you directly, answer in the first person, short and plain."#;

#[cfg(test)]
#[path = "pack_tests.rs"]
mod tests;
