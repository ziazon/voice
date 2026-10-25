//! The private, plain report from a completed build.
use crate::{
    build::{self, Built},
    draft, interview, text,
};
use persona_builder::error::{Error, Result};
use persona_builder_ingest::private;
use persona_builder_steps::book::Book;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const REPORT: &str = "report.json";
pub const SCHEMA: &str = "persona-builder/report/1";
pub const REPORT_MOST: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Strong,
    GettingThere,
    Thin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strengthen {
    Interview,
    Claudeai,
    Time,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub right: usize,
    pub of: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub area: String,
    pub level: Level,
    pub examples: usize,
    pub strengthen: Vec<Strengthen>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Always SCHEMA.
    pub schema: String,
    /// The local day of the build, YYYY-MM-DD.
    pub built: String,
    pub overall: Level,
    pub answered: usize,
    pub held: usize,
    /// None when untested; else the backtest's overall hits and n.
    pub test: Option<Test>,
    pub areas: Vec<Area>,
}

/// Counts and test results only: no record text enters the report.
pub fn report(book: &Book, built: &Built, answered: usize, day: &str) -> Report {
    let mut rows = Vec::new();
    for name in &book.categories {
        if let Some(row) = built.tally.iter().find(|(area, _)| area == name) {
            rows.push(row);
        }
    }
    for row in &built.tally {
        if !book.categories.contains(&row.0) {
            rows.push(row);
        }
    }
    let mut areas: Vec<Area> = rows
        .into_iter()
        .map(|(area, row)| {
            let examples = row.questions + row.messages;
            let strong = built.backtest.as_ref().is_some_and(|test| {
                build::live(
                    test.score
                        .categories
                        .get(area)
                        .and_then(|tiers| tiers[0].as_ref()),
                )
            });
            let level = if strong {
                Level::Strong
            } else if examples < build::THIN {
                Level::Thin
            } else {
                Level::GettingThere
            };
            let mut strengthen = Vec::new();
            if level != Level::Strong {
                if answered < interview::THIN_BELOW || examples < build::THIN {
                    strengthen.push(Strengthen::Interview);
                }
                if built.counts.claudeai == 0 {
                    strengthen.push(Strengthen::Claudeai);
                }
                strengthen.push(Strengthen::Time);
            }
            Area {
                area: area.clone(),
                level,
                examples,
                strengthen,
            }
        })
        .collect();
    if let Some(test) = &built.backtest {
        for area in draft::live_categories(book, &test.score) {
            if !areas.iter().any(|existing| existing.area == area) {
                areas.push(Area {
                    area,
                    level: Level::Strong,
                    examples: 0,
                    strengthen: Vec::new(),
                });
            }
        }
    }
    let strong = areas
        .iter()
        .filter(|area| area.level == Level::Strong)
        .count();
    let overall = if answered < interview::THIN_BELOW {
        Level::Thin
    } else if built.backtest.is_some() && !areas.is_empty() && strong * 2 >= areas.len() {
        Level::Strong
    } else {
        Level::GettingThere
    };
    Report {
        schema: SCHEMA.into(),
        built: day.into(),
        overall,
        answered,
        held: built.held,
        test: built.backtest.as_ref().map(|test| Test {
            right: test.score.overall.hits,
            of: test.score.overall.n,
        }),
        areas,
    }
}

/// The same words after a build and when showing its saved report.
pub fn render(report: &Report) -> Vec<String> {
    let mut lines = vec![text::report_heading(text::report_level(report.overall))];
    if report.overall == Level::Thin {
        lines.push(text::report_thin_history(report.answered));
    } else if let Some(test) = &report.test {
        let strong = report
            .areas
            .iter()
            .filter(|area| area.level == Level::Strong)
            .count();
        lines.push(text::report_tested(
            strong,
            report.areas.len(),
            test.right,
            test.of,
        ));
    } else {
        lines.push(text::REPORT_ENOUGH_HISTORY.into());
    }
    if report.test.is_none() {
        lines.push(text::report_untested(report.held));
        if report.answered < interview::THIN_BELOW {
            lines.push(text::REPORT_INTERVIEW.into());
        }
    }
    if report.areas.is_empty() {
        lines.push(text::REPORT_NO_AREAS.into());
        return lines;
    }
    lines.push(text::REPORT_AREAS.into());
    for area in &report.areas {
        let mut line = text::report_area(&area.area, text::report_level(area.level));
        if area.level == Level::Thin {
            line.push_str(&text::report_examples(area.examples));
        }
        line.push('.');
        if area.level != Level::Strong {
            let ways: Vec<_> = area
                .strengthen
                .iter()
                .map(|way| match way {
                    Strengthen::Interview => text::REPORT_STRENGTHEN_INTERVIEW,
                    Strengthen::Claudeai => text::REPORT_STRENGTHEN_CLAUDEAI,
                    Strengthen::Time => text::REPORT_STRENGTHEN_TIME,
                })
                .collect();
            line.push_str(&text::report_strengthen(&ways));
        }
        lines.push(line);
    }
    lines
}

/// Check area names before saving any member files.
pub fn check(report: &Report) -> Result<()> {
    check_names(report.areas.iter().map(|area| area.area.as_str()))
}

/// Check category names before making any model calls.
pub fn check_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<()> {
    if names.into_iter().any(|name| {
        name.is_empty()
            || name.chars().count() > 100
            || name.chars().any(persona_builder_steps::text_check::hidden)
    }) {
        return Err(Error::Invalid(text::REPORT_BAD_NAME.into()));
    }
    Ok(())
}

fn bad_names(report: &Report) -> bool {
    check(report).is_err()
}

/// Save the report privately in the member's state folder.
pub fn write(home: &Path, slug: &str, report: &Report) -> Result<PathBuf> {
    let mut bytes = serde_json::to_vec_pretty(report).expect("a report always serialises");
    bytes.push(b'\n');
    private::write_private(home, slug, REPORT, &bytes)
}

fn unreadable() -> Error {
    Error::Invalid(text::REPORT_UNREADABLE.into())
}

/// Read the last report, refusing an invalid or unsafe file.
pub fn read(home: &Path, slug: &str) -> Result<Option<Report>> {
    let Some(bytes) = private::read_private_optional(home, slug, REPORT)? else {
        return Ok(None);
    };
    if bytes.len() > REPORT_MOST {
        return Err(unreadable());
    }
    let report: Report = serde_json::from_slice(&bytes).map_err(|_| unreadable())?;
    if report.schema != SCHEMA || !build::is_date(&report.built) || bad_names(&report) {
        return Err(unreadable());
    }
    Ok(Some(report))
}
