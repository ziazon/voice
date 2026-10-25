//! Reading the step files: every one is untrusted and checked whole before it is used.
use persona_builder::error::{Error, Result};
use persona_builder_ingest::corpus::exact_numbers;
use serde_json::{Map, Value};

/// A corpus as `persona_builder_ingest::corpus::read_records` reads it: line numbers and records.
pub type Records = [(usize, Map<String, Value>)];
/// The same, owned.
pub type RecordList = Vec<(usize, Map<String, Value>)>;

/// A JSON file read whole: UTF-8, serde_json's grammar, and only numbers that copy exactly.
pub fn read_json(name: &str, bytes: &[u8]) -> Result<Value> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::Invalid(format!("{name} is not UTF-8 text")))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|_| Error::Invalid(format!("{name} is not valid JSON")))?;
    if !exact_numbers(&value) {
        return Err(Error::Invalid(format!(
            "{name} holds a number Persona Builder can't copy exactly"
        )));
    }
    Ok(value)
}

/// A JSON file that must hold one object.
pub fn read_object(name: &str, bytes: &[u8]) -> Result<Map<String, Value>> {
    match read_json(name, bytes)? {
        Value::Object(map) => Ok(map),
        _ => Err(Error::Invalid(format!("{name} must be a JSON object"))),
    }
}

/// A labels or predictions file: an object whose every value is an object.
pub fn read_objects(name: &str, bytes: &[u8]) -> Result<Map<String, Value>> {
    let map = read_object(name, bytes)?;
    if let Some(id) = map.iter().find(|(_, v)| !v.is_object()).map(|(k, _)| k) {
        return Err(Error::Invalid(format!(
            "{name}: the entry for {id} is not a JSON object"
        )));
    }
    Ok(map)
}

/// One corpus record and where it came from, read the way the Python's `r[key]` reads it.
#[derive(Clone, Copy)]
pub struct Record<'a> {
    pub name: &'a str,
    pub line: usize,
    pub map: &'a Map<String, Value>,
}
impl<'a> Record<'a> {
    pub fn all(name: &'a str, records: &'a Records) -> impl Iterator<Item = Record<'a>> {
        records.iter().map(move |(line, map)| Record {
            name,
            line: *line,
            map,
        })
    }
    pub fn shape(&self, key: &str) -> Error {
        Error::Invalid(format!(
            "{}:{}: a record has an unexpected shape ({key})",
            self.name, self.line
        ))
    }
    /// An error from a shared helper, placed at this record.
    pub fn at(&self, error: Error) -> Error {
        Error::Invalid(format!("{}:{}: {error}", self.name, self.line))
    }
    pub fn get(&self, key: &str) -> Result<&'a Value> {
        self.map.get(key).ok_or_else(|| self.shape(key))
    }
    pub fn str(&self, key: &str) -> Result<&'a str> {
        self.get(key)?.as_str().ok_or_else(|| self.shape(key))
    }
    pub fn array(&self, key: &str) -> Result<&'a [Value]> {
        self.get(key)?
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| self.shape(key))
    }
    /// Every string in an array of strings.
    pub fn strings(&self, key: &str) -> Result<Vec<&'a str>> {
        self.array(key)?
            .iter()
            .map(|v| v.as_str().ok_or_else(|| self.shape(key)))
            .collect()
    }
    /// The options' labels, each a string (all the Python's `searchable` and `precedent` read).
    pub fn labels(&self) -> Result<Vec<&'a str>> {
        self.array("options")?
            .iter()
            .map(|o| {
                o.get("label")
                    .and_then(Value::as_str)
                    .ok_or_else(|| self.shape("options"))
            })
            .collect()
    }
    /// The options' labels and descriptions, each a string.
    pub fn options(&self) -> Result<Vec<(&'a str, &'a str)>> {
        self.array("options")?
            .iter()
            .map(|o| {
                let field = |key| {
                    o.get(key)
                        .and_then(Value::as_str)
                        .ok_or_else(|| self.shape("options"))
                };
                Ok((field("label")?, field("description")?))
            })
            .collect()
    }
    pub fn id(&self) -> Result<&'a str> {
        self.str("id")
    }
}

/// The first `n` characters, as Python's `text[:n]` slices.
pub fn first(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// serde_json's pretty printer is `json.dumps(indent=2, ensure_ascii=False)`, plus print's newline.
pub fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("JSON values always serialize");
    text.push('\n');
    text
}
