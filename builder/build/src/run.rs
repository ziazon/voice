//! The checked build path shared by the command and the app.
use crate::{
    build::{Built, Inputs},
    draft, interview,
    report::{self, Report},
};
use persona_builder::{error::Result, member::Member};
use persona_builder_agent::ledger::local_day;
use persona_builder_ingest::{corpus::read_records, filters::Filters, private};
use persona_builder_steps::book::{Book, book};
use std::{path::Path, time::SystemTime};

pub struct Prepared {
    pub book: Book,
    pub codebook: String,
    pub corpus: Vec<u8>,
    pub answered: usize,
    pub filters: Filters,
}

pub fn prepare(
    member: &Member,
    member_dir: &Path,
    home: &Path,
    corpus_name: &str,
) -> Result<Prepared> {
    let book = book(member)?;
    report::check_names(book.categories.iter().map(String::as_str))?;
    let codebook = draft::read_codebook(member_dir)?;
    draft::preflight(member_dir)?;
    let (folder, name) = private::place(corpus_name)?;
    private::validate_name(name)?;
    let corpus = private::read_private_at(home, &member.slug, folder, name)?;
    let answered = interview::answered(&read_records(corpus_name, &corpus)?);
    let filters = Filters::compile(&member.filters)?;
    Ok(Prepared {
        book,
        codebook,
        corpus,
        answered,
        filters,
    })
}

impl Prepared {
    pub fn inputs<'a>(
        &'a self,
        member: &'a Member,
        corpus_name: &'a str,
        model: &'a str,
    ) -> Inputs<'a> {
        Inputs {
            member,
            book: &self.book,
            codebook: &self.codebook,
            corpus_name,
            corpus: &self.corpus,
            filters: &self.filters,
            model,
        }
    }
}

pub struct Checked {
    decisions: String,
    entries: String,
    report: Report,
}

pub fn check(member: &Member, prepared: &Prepared, built: &Built, model: &str) -> Result<Checked> {
    let decisions = draft::decisions(member, &prepared.book, built);
    let day = local_day(SystemTime::now());
    let entries = draft::entries(&prepared.book, built, model, &day);
    draft::check_private(&prepared.filters, &decisions, &entries)?;
    let report = report::report(&prepared.book, built, prepared.answered, &day);
    report::check(&report)?;
    Ok(Checked {
        decisions,
        entries,
        report,
    })
}

pub fn save(member: &Member, member_dir: &Path, home: &Path, checked: Checked) -> Result<Report> {
    let existing = draft::read_text(
        &member_dir.join(draft::MEASUREMENTS),
        draft::MEASUREMENTS_MOST,
    )?;
    let measurements = draft::measurements(existing.as_deref(), member, &checked.entries);
    draft::write(member_dir, &checked.decisions, &measurements)?;
    report::write(home, &member.slug, &checked.report)?;
    Ok(checked.report)
}
