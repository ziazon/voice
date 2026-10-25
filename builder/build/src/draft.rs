//! The build's output (`bootstrap/PROMPT.md` steps 9 to 11): `decisions.md` in the template's own
//! shape, the dated `measurements.md` entry, the check that neither holds anything private, and the
//! write that changes the member's folder only once both are ready.
use crate::{
    build::{Built, live},
    text,
};
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_ingest::filters::Filters;
use persona_builder_steps::{
    book::Book,
    labels::tally_markdown,
    score::{Cell, Score},
    share::{percent, share},
};
use std::{
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub const TEMPLATE: &str = include_str!("../../../crew/_template/decisions.md");
pub const DECISIONS: &str = "decisions.md";
pub const MEASUREMENTS: &str = "measurements.md";
/// The template's example entries, removed once a real one is written.
const PLACEHOLDERS: [&str; 2] = [
    "## YYYY-MM-DD: build backtest",
    "## YYYY-MM-DD: calibration interview",
];
const REPORTS: &str = "## Reports";
/// The most the member's codebook brief, and its measurements file, may hold.
pub const CODEBOOK_MOST: u64 = 64 * 1024;
pub const MEASUREMENTS_MOST: u64 = 1024 * 1024;
const STAGED: &str = ".persona-builder-";
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn pronouns(member: &Member) -> &str {
    member.pronouns.as_deref().unwrap_or("they/them")
}

/// The template with the member's names in place, as `install.sh` fills it.
pub fn filled_template(member: &Member) -> String {
    TEMPLATE
        .replace("{{NAME}}", &member.name)
        .replace("{{PRONOUNS}}", pronouns(member))
        .replace("{{PRINCIPAL}}", &member.principal)
        .replace("{{SLUG}}", &member.slug)
}

fn cell(cell: Option<&Cell>) -> String {
    cell.map_or_else(|| "—".to_owned(), |c| format!("{}/{}", c.hits, c.n))
}

/// The live categories, in the codebook's order and then any others.
pub fn live_categories(book: &Book, score: &Score) -> Vec<String> {
    ordered(book, score)
        .into_iter()
        .filter(|name| live(score.categories[name][0].as_ref()))
        .collect()
}

fn ordered(book: &Book, score: &Score) -> Vec<String> {
    let mut names: Vec<String> = book
        .categories
        .iter()
        .filter(|c| score.categories.contains_key(*c))
        .cloned()
        .collect();
    for name in score.categories.keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names
}

fn sources(built: &Built) -> String {
    let c = &built.counts;
    let mut line = format!(
        "{} answered questions and {} typed messages in Claude Code",
        c.questions, c.messages
    );
    if c.claudeai > 0 {
        line.push_str(&format!(
            ", plus {} messages from claude.ai conversations marked mine",
            c.claudeai
        ));
    }
    line.push_str(&format!(", {} → {}", c.first, c.last));
    if c.interview > 0 {
        line.push_str(&format!("; {} interview answers", c.interview));
    }
    if c.rounds > 0 {
        line.push_str(&format!("; {} answers from question rounds", c.rounds));
    }
    line
}

fn calibration(member: &Member, book: &Book, built: &Built) -> (String, Vec<String>) {
    let Some(test) = &built.backtest else {
        let text = format!(
            "There were too few held-out questions to backtest ({}, fewer than 30), so no category is live. {} decides nothing alone until a later build can measure it; High is treated as Medium everywhere.",
            built.held, member.name
        );
        let rows = built
            .tally
            .iter()
            .map(|(name, _)| format!("| {name} | — | — | — | no |"))
            .collect();
        return (text, rows);
    };
    let s = &test.score;
    let live = live_categories(book, s);
    let text = format!(
        "Measured on {} held-out questions ({} → {}), the newest share of answered questions, never seen while this model was written. Overall {} of {} ({}); always taking the recommendation would have scored {}. A category is **live** at 5 or more High picks with at least 90% right: {}. {} decides alone only in live categories that autonomy.md opens; elsewhere High is treated as Medium.",
        s.overall.n,
        test.first,
        test.last,
        s.overall.hits,
        s.overall.n,
        percent(s.overall.accuracy()),
        percent(share(s.baseline, s.overall.n)),
        if live.is_empty() {
            "none yet".to_owned()
        } else {
            live.join(", ")
        },
        member.name
    );
    let rows = ordered(book, s)
        .iter()
        .map(|name| {
            let tiers = &s.categories[name];
            format!(
                "| {name} | {} | {} | {} | {} |",
                cell(tiers[0].as_ref()),
                cell(tiers[1].as_ref()),
                cell(tiers[2].as_ref()),
                if live.contains(name) { "yes" } else { "no" }
            )
        })
        .collect();
    (text, rows)
}

/// `decisions.md`: the template's lines, its placeholders filled from the build.
pub fn decisions(member: &Member, book: &Book, built: &Built) -> String {
    let (calibrated, table) = calibration(member, book, built);
    let tally = tally_markdown(&built.tally);
    let mut out = Vec::new();
    let mut heading = "";
    let mut in_placeholder = false;
    let template = filled_template(member);
    for line in template.lines() {
        if in_placeholder {
            in_placeholder = !line.ends_with('>');
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            heading = rest;
        }
        if line.starts_with('<') {
            in_placeholder = !line.ends_with('>');
            out.push(if heading.starts_with("Calibration") {
                calibrated.clone()
            } else if heading.starts_with("How often") {
                tally.trim_end().to_owned()
            } else {
                built.sections.trim_end().to_owned()
            });
            continue;
        }
        if line.starts_with("mined_through: ") {
            out.push(format!("mined_through: {}", built.counts.mined_through));
        } else if line.starts_with("sources: ") {
            out.push(format!("sources: {}", sources(built)));
        } else if line.starts_with("generated_by: ") {
            out.push("generated_by: persona-builder build (counts) and Claude Code distillation over labelled records (paraphrase only)".to_owned());
        } else {
            out.push(line.to_owned());
            if line.starts_with("|---") && heading.starts_with("Calibration") {
                out.extend(table.iter().cloned());
            }
        }
    }
    out.join("\n") + "\n"
}

fn tiers(s: &Score) -> String {
    ["High", "Medium", "Low"]
        .iter()
        .zip(&s.tiers)
        .map(|(name, c)| {
            format!(
                "{name} {}",
                c.map_or_else(|| "none".to_owned(), |c| format!("{}/{}", c.hits, c.n))
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The dated entries for `measurements.md`: the backtest (or why there was none), and the interview
/// when the history holds one.
pub fn entries(book: &Book, built: &Built, model: &str, today: &str) -> String {
    let mut out = match &built.backtest {
        Some(test) => {
            let s = &test.score;
            let live = live_categories(book, s);
            format!(
                "## {today}: build backtest\n\n- **Set:** {} held-out questions, {} → {}: the newest share of answered questions, never seen by the model.\n- **Method:** `persona-builder build` predicted each one blind through the person's own Claude Code (model {model}), from decisions.md and the closest past decisions in the training split only; scored with `persona-builder score`.\n- **Result:** {} of {} ({}); always-take-the-recommendation baseline {}. {}.\n- **Read:** {}\n",
                s.overall.n,
                test.first,
                test.last,
                s.overall.hits,
                s.overall.n,
                percent(s.overall.accuracy()),
                percent(share(s.baseline, s.overall.n)),
                tiers(s),
                if live.is_empty() {
                    "no category is live yet.".to_owned()
                } else {
                    format!("live: {}.", live.join(", "))
                }
            )
        }
        None => format!(
            "## {today}: build (no backtest)\n\n- **Set:** {} held-out questions, fewer than 30, so there was too little to measure.\n- **Read:** no category is live.\n",
            built.held
        ),
    };
    if built.counts.interview > 0 {
        out.push_str(&format!(
            "\n## {today}: calibration interview\n\n- {} interview answers, labelled and distilled like any other record (training data, never test data).\n",
            built.counts.interview
        ));
    }
    out
}

/// `measurements.md` with the new entries: the template's example entries removed, the new ones
/// before `## Reports` (else at the end). A missing file starts with the template's heading.
pub fn measurements(existing: Option<&str>, member: &Member, entries: &str) -> String {
    let fresh = format!(
        "# {} measurements\n\nEvery test of {}, newest last. Paraphrase only: never paste one of {}'s answers here.\n",
        member.name, member.name, member.principal
    );
    let text = existing.unwrap_or(&fresh);
    let mut kept: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        if line.starts_with("## ") {
            skipping = PLACEHOLDERS.contains(&line);
        }
        if !skipping {
            kept.push(line);
        }
    }
    let at = kept.iter().position(|line| *line == REPORTS);
    let (before, after) = kept.split_at(at.unwrap_or(kept.len()));
    let before = before.join("\n");
    let mut out = format!("{}\n\n{}", before.trim_end(), entries.trim_end());
    if after.is_empty() {
        out.push('\n');
    } else {
        out.push_str("\n\n");
        out.push_str(&after.join("\n"));
        out.push('\n');
    }
    out
}

/// Refuse a draft or an entry that a filter marks, naming places only.
pub fn check_private(filters: &Filters, decisions: &str, entries: &str) -> Result<()> {
    let mut places = Vec::new();
    for (name, text) in [
        (DECISIONS, decisions),
        ("the new measurements entry", entries),
    ] {
        for (line, kind) in filters.lines(text) {
            places.push(format!("{name}:{line} ({})", kind.as_str()));
        }
    }
    if places.is_empty() {
        Ok(())
    } else {
        Err(Error::Invalid(text::private_hits(&places)))
    }
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// The existing file's mode, or 0644 for a new one; a link or anything but a file is refused.
fn mode_of(path: &Path) -> Result<u32> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0o644),
        Err(source) => Err(io(path, source)),
        Ok(meta) if meta.file_type().is_symlink() => Err(Error::Invalid(format!(
            "{} is a link: Persona Builder writes only real files, never links. Move it away and try again",
            path.display()
        ))),
        Ok(meta) if !meta.is_file() => Err(Error::Invalid(format!(
            "{} is not a file: move it away and try again",
            path.display()
        ))),
        Ok(meta) => Ok(meta.permissions().mode() & 0o777),
    }
}

/// A file written beside its target, removed unless renamed into place.
struct Staged {
    path: PathBuf,
    target: PathBuf,
    armed: bool,
}
impl Staged {
    fn new(dir: &Path, name: &str, text: &str) -> Result<Staged> {
        let target = dir.join(name);
        let mode = mode_of(&target)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = dir.join(format!(
            ".{name}{STAGED}{}-{nanos}-{count}",
            std::process::id()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|e| io(&path, e))?;
        let staged = Staged {
            path,
            target,
            armed: true,
        };
        file.write_all(text.as_bytes())
            .map_err(|e| io(&staged.path, e))?;
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|e| io(&staged.path, e))?;
        file.sync_all().map_err(|e| io(&staged.path, e))?;
        Ok(staged)
    }
    fn rename(mut self) -> Result<()> {
        mode_of(&self.target)?;
        fs::rename(&self.path, &self.target).map_err(|e| io(&self.target, e))?;
        self.armed = false;
        Ok(())
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Write both files into the member's folder: staged beside them first, then renamed into place,
/// `decisions.md` first. Any failure before the renames leaves the folder as it was.
pub fn write(member_dir: &Path, decisions: &str, measurements: &str) -> Result<()> {
    sweep(member_dir)?;
    let decisions = Staged::new(member_dir, DECISIONS, decisions)?;
    let measurements = Staged::new(member_dir, MEASUREMENTS, measurements)?;
    decisions.rename()?;
    measurements.rename()
}

/// Remove the staged files an interrupted run left (regular files only, never a link).
fn sweep(member_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(member_dir).map_err(|e| io(member_dir, e))? {
        let entry = entry.map_err(|e| io(member_dir, e))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let left = [DECISIONS, MEASUREMENTS]
            .iter()
            .any(|target| name.starts_with(&format!(".{target}{STAGED}")));
        if left
            && entry
                .file_type()
                .map_err(|e| io(&entry.path(), e))?
                .is_file()
        {
            fs::remove_file(entry.path()).map_err(|e| io(&entry.path(), e))?;
        }
    }
    Ok(())
}

/// Before anything is spent: both files can be replaced (each a real file, or none yet), and the
/// measurements file can be read. It is read again just before the write.
pub fn preflight(member_dir: &Path) -> Result<()> {
    mode_of(&member_dir.join(DECISIONS))?;
    mode_of(&member_dir.join(MEASUREMENTS))?;
    read_text(&member_dir.join(MEASUREMENTS), MEASUREMENTS_MOST).map(|_| ())
}

/// The member's own `briefs/codebook.md`: neither the folder nor the file a link.
pub fn read_codebook(member_dir: &Path) -> Result<String> {
    let briefs = member_dir.join("briefs");
    match fs::symlink_metadata(&briefs) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(Error::Invalid(format!(
                "{} is a link: Persona Builder reads only real folders, never links. Move it away and try again",
                briefs.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::Invalid(text::NO_CODEBOOK.into()));
        }
        Err(source) => return Err(io(&briefs, source)),
    }
    read_text(&briefs.join("codebook.md"), CODEBOOK_MOST)?
        .ok_or_else(|| Error::Invalid(text::NO_CODEBOOK.into()))
}

/// A text file in the member's folder, or `None` when it is not there: opened without following a
/// link, a regular file of at most `most` bytes, UTF-8.
pub fn read_text(path: &Path, most: u64) -> Result<Option<String>> {
    use std::io::Read;
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
            return Err(Error::Invalid(format!(
                "{} is a link: Persona Builder reads only real files, never links. Move it away and try again",
                path.display()
            )));
        }
        Err(source) => return Err(io(path, source)),
    };
    let meta = file.metadata().map_err(|e| io(path, e))?;
    if !meta.is_file() {
        return Err(Error::Invalid(format!(
            "{} is not a file: move it away and try again",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.take(most + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io(path, e))?;
    if bytes.len() as u64 > most {
        return Err(Error::Invalid(format!(
            "{} is larger than Persona Builder reads ({} KiB at most)",
            path.display(),
            most / 1024
        )));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| Error::Invalid(format!("{} is not UTF-8 text", path.display())))
}
