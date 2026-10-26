//! Full interviews fill a thin history; short interviews fill out kinds with few examples.
//! Both draft questions in the shape `round` reads and keep answers as training records.
use crate::text;
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_agent::{
    claude::{Call, Claude},
    ledger::Session,
};
use persona_builder_ingest::filters::Filters;
use persona_builder_steps::{
    book::Book,
    input::Records,
    labels::Row,
    round::{Round, check_questions, round},
};
use serde_json::{Map, Value, json};

/// Below this many answered questions the full interview is offered.
pub const THIN_BELOW: usize = 150;
pub const COUNT: usize = 20;
/// The drafted questions, in the state folder, for the person to swap and then answer.
pub const QUESTIONS_FILE: &str = "interview-questions.json";
/// The app's answers so far, in the round-answers shape.
pub const ANSWERS_FILE: &str = "interview-answers.json";
/// The answers as records; the merge keeps this file in training.
pub const OUT: &str = "decisions-interview.jsonl";
/// The step's name in the ledger, and the round name that gives ids `question:interview-NN`.
pub const STEP: &str = "interview";
/// The short interview covers at most five kinds, four questions on each.
pub const MOST_THIN_KINDS: usize = 5;
pub const PER_THIN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Full,
    Short,
}

impl Form {
    pub fn for_answered(answered: usize) -> Self {
        if answered < THIN_BELOW {
            Self::Full
        } else {
            Self::Short
        }
    }
    pub fn step(self) -> &'static str {
        match self {
            Self::Full => STEP,
            Self::Short => "interview-short",
        }
    }
    pub fn questions_file(self) -> &'static str {
        match self {
            Self::Full => QUESTIONS_FILE,
            Self::Short => "interview-short-questions.json",
        }
    }
    pub fn answers_file(self) -> &'static str {
        match self {
            Self::Full => ANSWERS_FILE,
            Self::Short => "interview-short-answers.json",
        }
    }
    pub fn out(self) -> &'static str {
        match self {
            Self::Full => OUT,
            Self::Short => "decisions-interview-short.jsonl",
        }
    }
    pub fn count(self, weights: &[(String, Option<usize>)]) -> usize {
        match self {
            Self::Full => COUNT,
            Self::Short => PER_THIN * weights.len(),
        }
    }
}

/// Counted kinds with fewer than five examples, retaining the thinnest-first order.
pub fn thin_kinds(weights: &[(String, Option<usize>)]) -> Vec<(String, Option<usize>)> {
    weights
        .iter()
        .filter(|(_, n)| n.is_some_and(|n| n < crate::build::THIN))
        .take(MOST_THIN_KINDS)
        .cloned()
        .collect()
}

const MAX_QUESTION: usize = 400;
const MAX_LABEL: usize = 100;
const MAX_DESCRIPTION: usize = 300;

/// Fixed text only: the system prompt goes in the argv, which other local users can read.
pub const SYSTEM: &str = "You draft calibration interview questions for Persona Builder, which builds a person's AI assistant from how they decide. You have no tools. Answer only with the JSON the schema asks for.";

/// The answers the call must give: the round-questions shape. Fixed, for the same reason.
pub fn schema(count: usize) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["questions"],
        "properties": {"questions": {
            "type": "array", "minItems": count, "maxItems": count,
            "items": {
                "type": "object",
                "additionalProperties": false,
                "required": ["n", "header", "question", "multiSelect", "recommended", "options"],
                "properties": {
                    "n": {"type": "integer", "minimum": 1, "maximum": count},
                    "header": {"type": "string"},
                    "question": {"type": "string", "minLength": 1, "maxLength": MAX_QUESTION},
                    "multiSelect": {"type": "boolean"},
                    "recommended": {"enum": ["A", "B", "C", "D", null]},
                    "options": {
                        "type": "array", "minItems": 2, "maxItems": 4,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["label", "description"],
                            "properties": {
                                "label": {"type": "string", "minLength": 1, "maxLength": MAX_LABEL},
                                "description": {"type": "string", "maxLength": MAX_DESCRIPTION}
                            }
                        }
                    }
                }
            }
        }}
    })
}

/// The answered questions in a corpus, as `install.sh` counts them: every question record.
pub fn answered(records: &Records) -> usize {
    records
        .iter()
        .filter(|(_, record)| record.get("kind").and_then(Value::as_str) == Some("question"))
        .count()
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

/// The kinds of decision to ask about (the codebook's categories but `other`), each with its
/// training records in an earlier build's tally, thinnest first; in the codebook's order, with
/// no counts, when there is no tally yet.
pub fn weights(
    book: &Book,
    tally: Option<&[(String, Row)]>,
) -> Result<Vec<(String, Option<usize>)>> {
    let counts = tally.map(|rows| {
        rows.iter()
            .map(|(name, row)| (name.clone(), row.questions + row.messages))
            .collect::<Vec<_>>()
    });
    weighted(book, counts.as_deref())
}

/// The last checked report's example counts, thinnest first.
pub fn weights_from_report(
    book: &Book,
    report: Option<&crate::report::Report>,
) -> Result<Vec<(String, Option<usize>)>> {
    let counts = report.map(|report| {
        report
            .areas
            .iter()
            .map(|area| (area.area.clone(), area.examples))
            .collect::<Vec<_>>()
    });
    weighted(book, counts.as_deref())
}

fn weighted(
    book: &Book,
    counts: Option<&[(String, usize)]>,
) -> Result<Vec<(String, Option<usize>)>> {
    let mut kinds: Vec<&str> = Vec::new();
    for kind in &book.categories {
        if kind != "other" && !kinds.contains(&kind.as_str()) {
            kinds.push(kind);
        }
    }
    if kinds.is_empty() {
        return Err(invalid(text::NOTHING_TO_ASK));
    }
    let Some(rows) = counts else {
        return Ok(kinds.iter().map(|kind| (kind.to_string(), None)).collect());
    };
    let mut counted: Vec<_> = kinds
        .iter()
        .map(|kind| {
            let records = rows
                .iter()
                .find(|(name, _)| name == kind)
                .map_or(0, |(_, count)| *count);
            (kind.to_string(), Some(records))
        })
        .collect();
    // Stable: kinds with as many records keep the codebook's order.
    counted.sort_by_key(|(_, records)| *records);
    Ok(counted)
}

/// The call's prompt: the person's areas of work and kinds of decision, never a record.
pub fn prompt(form: Form, book: &Book, weights: &[(String, Option<usize>)]) -> String {
    let count = form.count(weights);
    let areas: Vec<&str> = book
        .domains
        .iter()
        .map(String::as_str)
        .filter(|area| *area != "other")
        .collect();
    let areas = if areas.is_empty() {
        "not named yet; draw on everyday work".to_owned()
    } else {
        areas.join(", ")
    };
    let counted = weights.iter().any(|(_, records)| records.is_some());
    let kinds: String = weights
        .iter()
        .map(|(kind, records)| match records {
            Some(1) => format!("- {kind}: 1 past decision\n"),
            Some(n) => format!("- {kind}: {n} past decisions\n"),
            None => format!("- {kind}\n"),
        })
        .collect();
    let (listed, spread) = if form == Form::Short {
        (
            " (the kinds of decision their history holds the fewest examples of)",
            "Exactly 4 questions on each kind of decision listed above.",
        )
    } else if counted {
        (
            " (with how many past decisions of each their history holds, the thinnest first)",
            "Put at least one question on each kind of decision, and give the rest to the kinds listed first, which their history holds least of.",
        )
    } else {
        (
            "",
            "Put at least one question on each kind of decision, and spread the rest evenly.",
        )
    };
    format!(
        "Draft a calibration interview for one person: {count} questions about the kinds of decision they make, so that an AI assistant can learn how they decide.

Their areas of work: {areas}.
The kinds of decision to ask about{listed}:
{kinds}
Rules:
- Exactly {count} questions, with `n` numbering them 1 to {count} in order.
- Each `header` is the kind of decision the question is about, written exactly as listed above.
- {spread}
- Each question describes one concrete, everyday situation from one of their areas of work and asks what they would do. Spread the questions across their areas.
- Give 2 to 4 options, each a short `label` with a one-line `description`; no two labels in a question the same. Keep each question under {MAX_QUESTION} characters, each label under {MAX_LABEL} and each description under {MAX_DESCRIPTION}.
- `recommended` is the letter of the honest default (A for the first option, then B, C, D), or null where there is none. Mark one only where most people in their line of work would agree.
- `multiSelect` is true only where choosing several options makes sense.
- Plain, warm words, no jargon. Never name a real person or organisation, and never ask about health, family, money or any private matter.
"
    )
}

/// A drafted questions file checked whole: `round`'s rules, the form's count numbered in order, each
/// header one of the kinds of decision, and short enough to show. Answers the file rebuilt with
/// only the keys `round` reads. No error repeats the draft's own words.
pub fn check(
    form: Form,
    weights: &[(String, Option<usize>)],
    drafted: &Value,
) -> Result<Map<String, Value>> {
    let count = form.count(weights);
    let wrong_count = || invalid(format!("the interview needs exactly {count} questions"));
    let file = drafted.as_object().ok_or_else(wrong_count)?;
    let questions = file
        .get("questions")
        .and_then(Value::as_array)
        .filter(|questions| questions.len() == count)
        .ok_or_else(wrong_count)?;
    check_questions(file)?;
    let too_long = |n: usize, what: &str, most: usize| {
        invalid(format!(
            "question {n}: {what} is longer than {most} characters"
        ))
    };
    let longer = |value: &Value, most: usize| {
        value
            .as_str()
            .is_some_and(|text| text.chars().count() > most)
    };
    let mut clean = Vec::with_capacity(count);
    for (at, q) in questions.iter().enumerate() {
        let n = at + 1;
        // `round` has checked the shape: an object, a distinct positive `n`, string fields, 2 to
        // 4 options with distinct labels, and a recommended letter among them.
        if q["n"].as_u64() != Some(n as u64) {
            return Err(invalid(format!(
                "the questions must be numbered 1 to {count} in order"
            )));
        }
        let header = q["header"].as_str().unwrap_or("");
        if !weights.iter().any(|(kind, _)| kind == header) {
            return Err(invalid(format!(
                "question {n}: the header must be one of the kinds of decision"
            )));
        }
        if longer(&q["question"], MAX_QUESTION) {
            return Err(too_long(n, "the question", MAX_QUESTION));
        }
        let mut options = Vec::new();
        for option in q["options"].as_array().into_iter().flatten() {
            if longer(&option["label"], MAX_LABEL) {
                return Err(too_long(n, "an option's label", MAX_LABEL));
            }
            if longer(&option["description"], MAX_DESCRIPTION) {
                return Err(too_long(n, "an option's description", MAX_DESCRIPTION));
            }
            options.push(json!({
                "label": option["label"],
                "description": option.get("description").cloned().unwrap_or_else(|| json!("")),
            }));
        }
        clean.push(json!({
            "n": n,
            "header": header,
            "question": q["question"],
            "multiSelect": q.get("multiSelect").cloned().unwrap_or(Value::Bool(false)),
            "recommended": q.get("recommended").cloned().unwrap_or(Value::Null),
            "options": options,
        }));
    }
    if form == Form::Short
        && weights
            .iter()
            .any(|(kind, _)| !clean.iter().any(|q| q["header"] == *kind))
    {
        return Err(invalid("the kinds of decision listed each need a question"));
    }
    Ok(Map::from_iter([(
        "questions".to_owned(),
        Value::from(clean),
    )]))
}

/// One call drafts the questions; a draft that fails [`check`] is asked for once more, with the
/// problem named. A call that fails, or that the cap refuses, is never retried here.
pub fn draft(
    form: Form,
    claude: &Claude,
    session: &Session,
    model: &str,
    book: &Book,
    weights: &[(String, Option<usize>)],
) -> Result<Map<String, Value>> {
    let count = form.count(weights);
    let schema = schema(count);
    let first = prompt(form, book, weights);
    let mut asked = first.clone();
    let mut problem = String::new();
    for _ in 0..2 {
        let reply = claude.call(
            session,
            &Call {
                step: form.step(),
                system: SYSTEM,
                prompt: &asked,
                model,
                schema: &schema,
            },
        )?;
        match check(form, weights, &reply.output) {
            Ok(file) => return Ok(file),
            Err(error) => {
                problem = error.to_string();
                asked = format!(
                    "{first}\nYour last draft didn't fit the form: {problem}. Draft all {count} questions again, following every rule above.\n"
                );
            }
        }
    }
    Err(invalid(text::draft_failed(&problem)))
}

/// The person's answers as training records (the form's round, project `interview`), by
/// `round`'s rules and through the filters. Nothing kept is refused, so an earlier file stays.
pub fn answer(
    form: Form,
    member: &Member,
    filters: &Filters,
    questions: &Map<String, Value>,
    answers: &Map<String, Value>,
) -> Result<Round> {
    let mut kept = round(member, filters, form.step(), questions, answers, false)?;
    if kept.records.is_empty() {
        return Err(invalid(if kept.personal + kept.secret == 0 {
            text::NO_ANSWERS
        } else {
            text::ALL_LEFT_OUT
        }));
    }
    for record in &mut kept.records {
        record.insert("project".to_owned(), Value::from(STEP));
    }
    Ok(kept)
}

pub fn summary(kept: &Round) -> String {
    format!(
        "kept {} interview answers, always for training; {} unanswered\nleft out {} that looked personal and {} that looked secret\n",
        kept.records.len(),
        kept.unanswered,
        kept.personal,
        kept.secret
    )
}
