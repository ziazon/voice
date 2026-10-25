//! `round`: an off-Claude question round imported as question records, training by default or held
//! out as a whole round.
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_ingest::{
    dates::{civil_from_days, days_from_civil, days_in_month},
    filters::{Filters, Kind},
    mine::outcome,
    privacy::privacy,
    python::py_space,
};
use serde_json::{Map, Value};
use std::collections::HashSet;

const LETTERS: [&str; 4] = ["A", "B", "C", "D"];

fn failed(message: &str) -> Error {
    Error::Invalid(format!("round: {message}"))
}
fn question_error(number: &str, message: &str) -> Error {
    Error::Invalid(format!("round: question {number}: {message}"))
}

/// A round's name: 1 to 40 lowercase letters, digits or hyphens, starting with a letter or digit.
pub fn validate_name(name: &str) -> Result<()> {
    let plain = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    let bytes = name.as_bytes();
    if !(1..=40).contains(&bytes.len())
        || !plain(&bytes[0])
        || !bytes.iter().all(|b| plain(b) || *b == b'-')
    {
        return Err(failed(
            "name must be 1 to 40 lowercase letters, digits or hyphens, starting with a letter or digit",
        ));
    }
    Ok(())
}

/// Where a round's records go when `--out` is not given.
pub fn default_out(name: &str) -> String {
    format!("decisions-round-{name}.jsonl")
}

struct Question {
    n: u64,
    question: String,
    header: String,
    multi: bool,
    /// Each option's label (with ` (Recommended)` on the recommended one) and description.
    options: Vec<(String, String)>,
    recommended: Option<String>,
}

/// The Python's `round_string`: a string (a non-empty one when `nonempty`), or `default` when absent.
fn string(
    object: &Map<String, Value>,
    key: &str,
    default: Option<&str>,
    number: &str,
    nonempty: bool,
) -> Result<String> {
    let value = match (object.get(key), default) {
        (Some(Value::String(s)), _) => Some(s.as_str()),
        (None, default) => default,
        _ => None,
    };
    match value {
        Some(v) if !nonempty || !v.chars().all(py_space) => Ok(v.to_owned()),
        _ => Err(question_error(
            number,
            &format!(
                "{key} must be {}",
                if nonempty {
                    "a non-empty string"
                } else {
                    "a string"
                }
            ),
        )),
    }
}

fn question(value: &Value, seen: &mut HashSet<u64>) -> Result<Question> {
    let q = value
        .as_object()
        .ok_or_else(|| question_error("?", "question must be an object"))?;
    let given = q.get("n").and_then(Value::as_u64);
    let Some(n) = given.filter(|n| *n > 0 && !seen.contains(n)) else {
        let shown = given.map_or("?".to_owned(), |n| n.to_string());
        return Err(question_error(
            &shown,
            "n must be a distinct positive integer",
        ));
    };
    seen.insert(n);
    let number = n.to_string();
    let text = string(q, "question", None, &number, true)?;
    let header = string(q, "header", Some(""), &number, false)?;
    let multi = match q.get("multiSelect") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(question_error(&number, "multiSelect must be a bool")),
    };
    let options = q
        .get("options")
        .and_then(Value::as_array)
        .filter(|o| (2..=4).contains(&o.len()))
        .ok_or_else(|| question_error(&number, "options must contain 2 to 4 options"))?;
    let mut labels = HashSet::new();
    let mut normalized = Vec::new();
    for option in options {
        let option = option
            .as_object()
            .ok_or_else(|| question_error(&number, "options must be objects with a label"))?;
        let label = string(option, "label", None, &number, true)?;
        if !labels.insert(label.clone()) {
            return Err(question_error(&number, "label must be distinct"));
        }
        let description = string(option, "description", Some(""), &number, false)?;
        normalized.push((label, description));
    }
    let letter = match q.get("recommended") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if LETTERS[..normalized.len()].contains(&s.as_str()) => {
            LETTERS.iter().position(|l| *l == s.as_str())
        }
        Some(_) => {
            return Err(question_error(
                &number,
                "recommended must be an option letter",
            ));
        }
    };
    if let Some(at) = letter {
        normalized[at].0.push_str(" (Recommended)");
    }
    Ok(Question {
        n,
        question: text,
        header,
        multi,
        recommended: letter.map(|at| normalized[at].0.clone()),
        options: normalized,
    })
}

/// The Python's `round_answer`: the picked labels in option order, the own words and the notes.
fn answer(q: &Question, value: Option<&Value>) -> Result<(Vec<String>, String, String)> {
    let number = q.n.to_string();
    let empty = Map::new();
    let a = match value {
        None => &empty,
        Some(Value::Object(a)) => a,
        Some(_) => return Err(question_error(&number, "answer must be an object")),
    };
    let own_words = string(a, "own_words", Some(""), &number, false)?;
    let notes = string(a, "notes", Some(""), &number, false)?;
    let letters = &LETTERS[..q.options.len()];
    let not_an_option = || question_error(&number, "pick is not an option");
    let picks: Vec<&str> = match a.get("pick") {
        None => vec![],
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(list)) => list
            .iter()
            .map(Value::as_str)
            .collect::<Option<_>>()
            .ok_or_else(not_an_option)?,
        Some(_) => return Err(not_an_option()),
    };
    if picks.iter().any(|p| !letters.contains(p)) {
        return Err(not_an_option());
    }
    if !q.multi && picks.len() > 1 {
        return Err(question_error(
            &number,
            "pick must have one option for a single-select question",
        ));
    }
    let labels = letters
        .iter()
        .zip(&q.options)
        .filter(|(letter, _)| picks.contains(*letter))
        .map(|(_, (label, _))| label.clone())
        .collect();
    Ok((labels, own_words, notes))
}

/// Seconds since 1970 of noon UTC on a `YYYY-MM-DD` date in ASCII digits. The Python's pattern
/// and `strptime` also take other scripts' digits in places (the year, a day's second digit).
fn noon(answered: Option<&Value>) -> Result<i128> {
    let shape = || failed("answered must be YYYY-MM-DD");
    let answered = answered.and_then(Value::as_str).ok_or_else(shape)?;
    let bytes = answered.as_bytes();
    let digits = |from: usize, to: usize| bytes[from..to].iter().all(u8::is_ascii_digit);
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !digits(0, 4)
        || !digits(5, 7)
        || !digits(8, 10)
    {
        return Err(shape());
    }
    let part = |from: usize, to: usize| answered[from..to].parse::<i64>().expect("ASCII digits");
    let (year, month, day) = (part(0, 4), part(5, 7), part(8, 10));
    if year < 1 || !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return Err(failed("answered must be a valid date"));
    }
    Ok(i128::from(days_from_civil(year, month, day)) * 86_400 + 43_200)
}

/// `datetime.isoformat(timespec="milliseconds")` in UTC with a Z, or `None` past the year 9999.
fn timestamp(seconds: i128) -> Option<String> {
    let last = i128::from(days_from_civil(9999, 12, 31)) * 86_400 + 86_399;
    if seconds > last {
        return None;
    }
    let days = i64::try_from(seconds.div_euclid(86_400)).ok()?;
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.000Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    ))
}

pub struct Round {
    pub name: String,
    pub split: &'static str,
    pub records: Vec<Map<String, Value>>,
    pub unanswered: usize,
    pub personal: usize,
    pub secret: usize,
}

/// The Python's `cmd_round`, every check in its order, from the two files' parsed objects.
pub fn round(
    member: &Member,
    filters: &Filters,
    name: &str,
    questions_file: &Map<String, Value>,
    answers_file: &Map<String, Value>,
    heldout: bool,
) -> Result<Round> {
    validate_name(name)?;
    let questions = questions_file
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| failed("questions must be a list"))?;
    let mut seen = HashSet::new();
    let questions = questions
        .iter()
        .map(|q| question(q, &mut seen))
        .collect::<Result<Vec<_>>>()?;
    let noon = noon(answers_file.get("answered"))?;
    let answers = answers_file
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| failed("answers must be an object"))?;
    let numbers: HashSet<String> = seen.iter().map(u64::to_string).collect();
    if let Some(key) = answers.keys().find(|key| !numbers.contains(*key)) {
        let shown = if key.len() <= 20 && key.bytes().all(|b| b.is_ascii_digit()) {
            key.as_str()
        } else {
            "?"
        };
        return Err(question_error(shown, "answer has no question"));
    }
    let split = if heldout { "heldout" } else { "train" };
    let mut round = Round {
        name: name.to_owned(),
        split,
        records: Vec::new(),
        unanswered: 0,
        personal: 0,
        secret: 0,
    };
    for q in &questions {
        let (picks, own_words, notes) = answer(q, answers.get(&q.n.to_string()))?;
        if picks.is_empty() && own_words.chars().all(py_space) {
            round.unanswered += 1;
            continue;
        }
        let when = timestamp(noon + i128::from(q.n))
            .ok_or_else(|| question_error(&q.n.to_string(), "n exceeds the timestamp range"))?;
        let options: Vec<Value> = q
            .options
            .iter()
            .map(|(label, description)| {
                Value::Object(Map::from_iter([
                    ("label".to_owned(), Value::from(label.as_str())),
                    ("description".to_owned(), Value::from(description.as_str())),
                ]))
            })
            .collect();
        let outcome = outcome(&picks, &own_words, q.recommended.as_deref());
        let mut record = Map::new();
        record.insert("question".into(), Value::from(q.question.as_str()));
        record.insert("header".into(), Value::from(q.header.as_str()));
        record.insert("multiSelect".into(), Value::from(q.multi));
        record.insert("options".into(), Value::from(options));
        record.insert("recommended".into(), Value::from(q.recommended.clone()));
        record.insert(
            "id".into(),
            Value::from(format!("question:{name}-{:02}", q.n)),
        );
        record.insert("kind".into(), Value::from("question"));
        record.insert("timestamp".into(), Value::from(when));
        record.insert("project".into(), Value::from(format!("round-{name}")));
        record.insert("picks".into(), Value::from(picks));
        record.insert("own_words".into(), Value::from(own_words));
        record.insert("notes".into(), Value::from(notes));
        record.insert("outcome".into(), Value::from(outcome));
        record.insert("split".into(), Value::from(split));
        record.insert(member.pick_field(), Value::Null);
        let record = Value::Object(record);
        match privacy(filters, &record)? {
            Some(Kind::Personal) => round.personal += 1,
            Some(Kind::Secret) => round.secret += 1,
            None => {
                let Value::Object(record) = record else {
                    unreachable!()
                };
                round.records.push(record);
            }
        }
    }
    Ok(round)
}

pub fn summary(r: &Round) -> String {
    format!(
        "imported {} answers from round {} ({}); {} unanswered\ndropped {} personal records, {} secret records\n",
        r.records.len(),
        r.name,
        r.split,
        r.unanswered,
        r.personal,
        r.secret
    )
}
