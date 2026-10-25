use crate::{
    record::{Case, Grade},
    text,
};
use persona_builder_build::build::Misfit;
use persona_builder_steps::text_check::hidden;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const ANSWER_SYSTEM: &str = include_str!("prompts/answer.md");
pub const GRADE_SYSTEM: &str = include_str!("prompts/grade.md");

pub fn answer_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["answers"],"properties":{"answers":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["id","answer"],"properties":{"id":{"type":"string"},"answer":{"type":"string"}}}}}})
}
pub fn grade_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["grades"],"properties":{"grades":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["id","grade"],"properties":{"id":{"type":"string"},"grade":{"type":"string","enum":["pass","fail"]}}}}}})
}
pub fn answer_prompt(decisions: &str, batch: &[&Case]) -> String {
    let mut prompt = text::ANSWER_PROMPT.to_owned();
    prompt.push_str(decisions);
    prompt.push_str(text::ANSWER_TESTS);
    for case in batch {
        let mut item = json!({"id":case.id,"area":case.area,"situation":case.situation});
        if let Some(options) = &case.options {
            item["options"] = json!(options);
        }
        prompt.push_str(&item.to_string());
        prompt.push('\n');
    }
    prompt
}
pub fn grade_prompt(items: &[(String, &str, &str)]) -> String {
    let mut prompt = text::GRADE_PROMPT.to_owned();
    for (id, expected, answer) in items {
        prompt.push_str(&json!({"id":id,"expected":expected,"answer":answer}).to_string());
        prompt.push('\n');
    }
    prompt
}

fn rows<'a>(reply: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    let object = reply.as_object()?;
    if object.len() != 1 {
        return None;
    }
    object.get(key)?.as_array()
}
fn pair<'a>(row: &'a Value, field: &str) -> Option<(&'a str, &'a str)> {
    let object = row.as_object()?;
    if object.len() != 2 {
        return None;
    }
    Some((object.get("id")?.as_str()?, object.get(field)?.as_str()?))
}
fn misfit(problems: &[String], answers: bool) -> Misfit {
    Misfit::new(
        problems.join("; "),
        if answers {
            text::apply_answers_count(problems.len())
        } else {
            text::apply_grades_count(problems.len())
        },
    )
}

pub fn check_answers(batch: &[&Case], reply: &Value) -> Result<BTreeMap<String, String>, Misfit> {
    let first = batch.first().map_or("", |case| case.id.as_str());
    let Some(rows) = rows(reply, "answers") else {
        return Err(misfit(&[text::rule(first, text::APPLY_REPLY_SHAPE)], true));
    };
    let mut answers = BTreeMap::new();
    let mut problems = Vec::new();
    for row in rows {
        let Some((id, answer)) = pair(row, "answer") else {
            problems.push(text::rule(first, text::APPLY_REPLY_SHAPE));
            continue;
        };
        let Some(case) = batch.iter().find(|case| case.id == id) else {
            problems.push(text::rule(first, text::APPLY_REPLY_IDS));
            continue;
        };
        if answers.insert(id.to_owned(), answer.to_owned()).is_some() {
            problems.push(text::rule(id, text::APPLY_REPLY_IDS));
        }
        if let Some(options) = &case.options {
            if !options.iter().any(|option| option == answer) {
                problems.push(text::rule(id, text::APPLY_ANSWER_CHOICE));
            }
        } else if answer.is_empty()
            || answer.trim() != answer
            || answer.chars().count() > 1000
            || answer.chars().any(hidden)
        {
            problems.push(text::rule(id, text::APPLY_ANSWER_LINE));
        }
    }
    for case in batch {
        if !answers.contains_key(&case.id) {
            problems.push(text::rule(&case.id, text::APPLY_REPLY_IDS));
        }
    }
    if problems.is_empty() {
        Ok(answers)
    } else {
        Err(misfit(&problems, true))
    }
}

pub fn check_grades(
    items: &[(String, &str, &str)],
    reply: &Value,
) -> Result<BTreeMap<String, Grade>, Misfit> {
    let first = items.first().map_or("", |item| item.0.as_str());
    let Some(rows) = rows(reply, "grades") else {
        return Err(misfit(&[text::rule(first, text::APPLY_REPLY_SHAPE)], false));
    };
    let mut grades = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut problems = Vec::new();
    for row in rows {
        let Some((id, value)) = pair(row, "grade") else {
            problems.push(text::rule(first, text::APPLY_REPLY_SHAPE));
            continue;
        };
        if !items.iter().any(|item| item.0 == id) {
            problems.push(text::rule(first, text::APPLY_REPLY_IDS));
            continue;
        }
        if !seen.insert(id) {
            problems.push(text::rule(id, text::APPLY_REPLY_IDS));
        }
        let grade = match value {
            "pass" => Grade::Pass,
            "fail" => Grade::Fail,
            _ => {
                problems.push(text::rule(id, text::APPLY_GRADE));
                continue;
            }
        };
        grades.insert(id.to_owned(), grade);
    }
    for (id, _, _) in items {
        if !seen.contains(id.as_str()) {
            problems.push(text::rule(id, text::APPLY_REPLY_IDS));
        }
    }
    if problems.is_empty() {
        Ok(grades)
    } else {
        Err(misfit(&problems, false))
    }
}
