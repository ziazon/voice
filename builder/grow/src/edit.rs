//! Structured edits to the decision model; this never writes the member's files.
use crate::text;
use persona_builder::error::{Error, Result};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKind {
    Add,
    Replace,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub area: String,
    pub kind: EditKind,
    pub old: Option<String>,
    pub text: String,
    pub why: String,
    pub cases: Vec<String>,
}
fn plain(line: &str) -> &str {
    line.strip_suffix('\n').unwrap_or(line)
}
fn invalid() -> Error {
    Error::Invalid(text::EDIT_DOES_NOT_FIT.into())
}
/// Return the changed text without touching the member's folder.
pub fn apply_edit(decisions: &str, edit: &Edit) -> Result<String> {
    let mut lines: Vec<String> = decisions.split_inclusive('\n').map(str::to_owned).collect();

    let by_kind = lines
        .iter()
        .position(|line| plain(line) == "## By kind of question")
        .ok_or_else(|| Error::Invalid(text::NOT_BUILT.into()))?;
    let by_end = (by_kind + 1..lines.len())
        .find(|&n| plain(&lines[n]).starts_with("## "))
        .unwrap_or(lines.len());
    let heading = format!("### {}", edit.area);
    let section = (by_kind + 1..by_end).find(|&n| plain(&lines[n]) == heading);
    let Some(start) = section else {
        if edit.kind != EditKind::Add {
            return Err(invalid());
        }
        let prefix = if by_end > 0 && !lines[by_end - 1].ends_with('\n') {
            "\n\n"
        } else if by_end > 0 && !plain(&lines[by_end - 1]).is_empty() {
            "\n"
        } else {
            ""
        };
        lines.insert(
            by_end,
            format!(
                "{prefix}### {}\n\n**Standing rules:**\n- {}\n\n",
                edit.area, edit.text
            ),
        );
        return Ok(lines.concat());
    };
    let end = (start + 1..by_end)
        .find(|&n| plain(&lines[n]).starts_with("### "))
        .unwrap_or(by_end);
    match edit.kind {
        EditKind::Add => {
            if lines[start + 1..end]
                .iter()
                .any(|line| plain(line).strip_prefix("- ") == Some(edit.text.as_str()))
            {
                return Err(invalid());
            }
            if let Some(rules) =
                (start + 1..end).find(|&n| plain(&lines[n]) == "**Standing rules:**")
            {
                let mut after = rules + 1;
                while after < end && plain(&lines[after]).starts_with("- ") {
                    after += 1;
                }
                if after == rules + 2 && plain(&lines[rules + 1]) == "- None yet." {
                    lines[rules + 1] = format!("- {}\n", edit.text);
                } else {
                    if after > rules + 1 && !lines[after - 1].ends_with('\n') {
                        lines[after - 1].push('\n');
                    }
                    lines.insert(after, format!("- {}\n", edit.text));
                }
            } else {
                let mut at = end;
                while at > start + 1 && plain(&lines[at - 1]).is_empty() {
                    at -= 1;
                }
                let prefix = if at > 0 && !lines[at - 1].ends_with('\n') {
                    "\n\n"
                } else {
                    "\n"
                };
                lines.insert(
                    at,
                    format!("{prefix}**Standing rules:**\n- {}\n", edit.text),
                );
            }
        }
        EditKind::Replace => {
            let old = edit.old.as_deref().ok_or_else(invalid)?.trim();
            if old == "None yet." {
                return Err(invalid());
            }
            let mut precedents = false;
            let mut found = Vec::new();
            for (index, line) in lines.iter().enumerate().take(end).skip(start + 1) {
                let line = plain(line);
                if line.starts_with("**") && line.ends_with(":**") {
                    precedents = line == "**Precedents:**";
                }
                if !precedents
                    && line
                        .strip_prefix("- ")
                        .is_some_and(|point| point.trim() == old)
                {
                    found.push(index);
                }
            }
            if found.len() != 1 {
                return Err(invalid());
            }
            let at = found[0];
            let newline = if lines[at].ends_with('\n') { "\n" } else { "" };
            lines[at] = format!("- {}{newline}", edit.text);
        }
    }
    Ok(lines.concat())
}
