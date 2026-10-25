//! The member's codebook, as `crew_decisions.py`'s `codebook()` reads it.
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_ingest::private::validate_name;
use serde_json::Value;

pub struct Book {
    pub categories: Vec<String>,
    pub domains: Vec<String>,
    pub escalates: Vec<String>,
    pub signals: Vec<String>,
    /// Each group's name and its categories, in the member's order.
    pub groups: Vec<(String, Vec<String>)>,
}

/// The codebook, or the Python's error naming what is missing. An empty list counts as missing:
/// the member format can't tell an empty list from an absent one.
pub fn book(member: &Member) -> Result<Book> {
    let missing_book = || Error::Invalid("member config: missing codebook".into());
    let book = member.codebook.as_ref().ok_or_else(missing_book)?;
    if book.categories.is_empty()
        && book.domains.is_empty()
        && book.escalates.is_empty()
        && book.signals.is_empty()
        && book.groups.is_empty()
        && book.extra.is_empty()
    {
        return Err(missing_book());
    }
    let missing: Vec<_> = [
        ("categories", book.categories.is_empty()),
        ("escalates", book.escalates.is_empty()),
        ("signals", book.signals.is_empty()),
        ("groups", book.groups.is_empty()),
    ]
    .into_iter()
    .filter(|(_, empty)| *empty)
    .map(|(key, _)| format!("codebook.{key}"))
    .collect();
    if !missing.is_empty() {
        return Err(Error::Invalid(format!(
            "member config: missing {}",
            missing.join(", ")
        )));
    }
    let groups = book
        .groups
        .iter()
        .map(|(group, categories)| {
            validate_name(&format!("{group}.jsonl")).map_err(|_| {
                Error::Invalid(format!(
                    "member config: codebook.groups: {group} can't be a file name; use letters, digits and hyphens"
                ))
            })?;
            let categories = categories
                .as_array()
                .and_then(|list| {
                    list.iter()
                        .map(|c| c.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or_else(|| {
                    Error::Invalid(format!(
                        "member config: codebook.groups.{group} must be a list of categories"
                    ))
                })?;
            Ok((group.clone(), categories))
        })
        .collect::<Result<_>>()?;
    Ok(Book {
        categories: book.categories.clone(),
        domains: book.domains.clone(),
        escalates: book.escalates.clone(),
        signals: book.signals.clone(),
        groups,
    })
}

/// True when the value is one of the strings.
pub(crate) fn one_of(value: Option<&Value>, list: &[String]) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|v| list.iter().any(|item| item == v))
}
