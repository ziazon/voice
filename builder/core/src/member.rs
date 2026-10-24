use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Component, Path, PathBuf};

/// The writer emits keys in the struct's order with unknown keys last, and drops
/// null optional fields and empty codebook lists. Numbers go through serde_json's f64/i64
/// handling, so byte parity with Python holds for configs in the template's shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pronouns: Option<String>,
    pub principal: String,
    pub principal_slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_pronouns: Option<String>,
    pub state_dir: String,
    pub mark_word: String,
    pub filters: Filters,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codebook: Option<Codebook>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filters {
    pub excluded_folders: String,
    pub personal: String,
    pub secret: String,
    pub pasted_prefixes: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Codebook {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escalates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub groups: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty", flatten)]
    pub extra: Map<String, Value>,
}

impl Member {
    pub fn from_value(value: Value, source: &str) -> Result<Self> {
        validate_config(&value, source)?;
        let object = value.as_object().expect("validated object");
        let mut missing = Vec::new();
        for key in [
            "name",
            "slug",
            "principal",
            "principal_slug",
            "state_dir",
            "mark_word",
            "filters",
        ] {
            if !object.contains_key(key) {
                missing.push(key.to_owned());
            }
        }
        for key in ["excluded_folders", "personal", "secret", "pasted_prefixes"] {
            if object
                .get("filters")
                .and_then(Value::as_object)
                .is_none_or(|filters| !filters.contains_key(key))
            {
                missing.push(format!("filters.{key}"));
            }
        }
        if !missing.is_empty() {
            return Err(Error::Invalid(format!(
                "member config: missing {}",
                missing.join(", ")
            )));
        }
        serde_json::from_value(value)
            .map_err(|error| Error::Invalid(format!("member config: {source}: {error}")))
    }

    pub fn pick_field(&self) -> String {
        format!("{}_pick", self.slug)
    }
    pub fn answer_field(&self) -> String {
        format!("{}_answer", self.principal_slug)
    }
    pub fn principal_subject(&self) -> &str {
        self.principal_pronouns
            .as_deref()
            .unwrap_or("they/them")
            .split('/')
            .next()
            .unwrap_or("they")
    }
}

/// Resolve only the member's exact private state folder, without touching the filesystem.
pub fn private_state_dir(member: &Member, home: &Path) -> Option<PathBuf> {
    let path = match member.state_dir.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(&member.state_dir),
    };
    let expected = home.join(".claude/state").join(&member.slug);
    // Path::components normalizes interior dots, so reject them in the written path too.
    let has_dots = path
        .as_os_str()
        .as_encoded_bytes()
        .split(|byte| *byte == b'/')
        .any(|part| matches!(part, b"." | b".."));
    let mut slug = Path::new(&member.slug).components();
    let single_slug = matches!(slug.next(), Some(Component::Normal(_))) && slug.next().is_none();
    if !single_slug
        || !path.is_absolute()
        || has_dots
        || !path.components().eq(expected.components())
    {
        return None;
    }
    Some(path)
}

fn validate_config(value: &Value, source: &str) -> Result<()> {
    let invalid = |message| Error::Invalid(format!("member config: {source}{message}"));
    let object = value
        .as_object()
        .ok_or_else(|| invalid(" must be a JSON object"))?;
    if let Some(filters) = object.get("filters") {
        let filters = filters
            .as_object()
            .ok_or_else(|| invalid(": filters must be a JSON object"))?;
        if let Some(prefixes) = filters.get("pasted_prefixes")
            && !prefixes
                .as_array()
                .is_some_and(|items| items.iter().all(Value::is_string))
        {
            return Err(invalid(
                ": filters.pasted_prefixes must be a list of strings",
            ));
        }
    }
    if let Some(answerer) = object.get("queue_answerer")
        && !answerer.as_str().is_some_and(|value| !value.is_empty())
    {
        return Err(invalid(": queue_answerer must be a non-empty string"));
    }
    Ok(())
}

pub(crate) fn refuse_symlink(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() => Err(Error::Invalid(format!(
            "{}: symlink refused",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

fn read_json(path: &Path) -> Result<Value> {
    refuse_symlink(path)?;
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|error| {
        Error::Invalid(format!(
            "member config: {} is not JSON ({error})",
            path.display()
        ))
    })
}

pub fn read_member(path: &Path) -> Result<Member> {
    let mut config = read_json(path)?;
    validate_config(&config, &path.display().to_string())?;
    let private = path.with_file_name("private.json");
    refuse_symlink(&private)?;
    if private.is_file() {
        let extra = read_json(&private)?;
        validate_config(&extra, &private.display().to_string())?;
        let merged = config.as_object_mut().expect("validated object");
        for (key, value) in extra.as_object().expect("validated object") {
            if key != "filters" {
                merged.insert(key.clone(), value.clone());
            }
        }
        let mut filters = merged
            .get("filters")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if let Some(extra_filters) = extra.get("filters").and_then(Value::as_object) {
            for (key, value) in extra_filters {
                if key == "pasted_prefixes" {
                    let mut prefixes = filters
                        .get(key)
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    prefixes.extend(
                        value
                            .as_array()
                            .expect("validated prefixes")
                            .iter()
                            .cloned(),
                    );
                    filters.insert(key.clone(), Value::Array(prefixes));
                } else {
                    let private_value = value.as_str().ok_or_else(|| {
                        Error::Invalid(format!(
                            "member config: private.json: filters.{key} must be a string (a regex)"
                        ))
                    })?;
                    if filters.get(key).is_some_and(|value| !value.is_string()) {
                        return Err(Error::Invalid(format!(
                            "member config: {}: filters.{key} must be a string (a regex)",
                            path.display()
                        )));
                    }
                    let joined = match filters
                        .get(key)
                        .and_then(Value::as_str)
                        .filter(|pattern| !pattern.is_empty())
                    {
                        Some(pattern) => format!("(?:{pattern})|(?:{private_value})"),
                        None => private_value.to_owned(),
                    };
                    filters.insert(key.clone(), Value::String(joined));
                }
            }
        }
        merged.insert("filters".into(), Value::Object(filters));
    }
    Member::from_value(config, &path.display().to_string())
}

pub fn write_member(path: &Path, member: &Member) -> Result<()> {
    let text = crate::json::to_python_json(member)?;
    std::fs::write(path, text).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        json::to_python_json,
        test_support::{Scratch, config, repo},
    };
    use serde_json::json;
    use std::fs;

    #[test]
    fn private_state_dir_accepts_only_the_exact_member_folder() {
        let home = Path::new("/home/robin");
        let mut member = Member::from_value(config(), "fixture").unwrap();
        member.state_dir = "~/.claude/state/ari".into();
        assert_eq!(
            private_state_dir(&member, home),
            Some(home.join(".claude/state/ari"))
        );
        for written in [
            "~",
            "~/.claude/state",
            "~/.claude/state/other",
            "~/.claude/state/ari/..",
            "state/ari",
            "/tmp/x",
            "~/.claude/state/./ari",
            "~/.claude/./state/ari",
            "~/.claude/state/ari/.",
            "~/.claude/state/ari/drafts",
        ] {
            member.state_dir = written.into();
            assert_eq!(private_state_dir(&member, home), None, "{written}");
        }
        member.state_dir = "/home/robin/.claude/state/ari".into();
        assert_eq!(
            private_state_dir(&member, home),
            Some(home.join(".claude/state/ari"))
        );
        member.slug = "/home/robin/.claude/state/ari".into();
        assert_eq!(private_state_dir(&member, home), None);
    }

    #[test]
    fn validates_objects_before_required_fields() {
        for (value, message) in [
            (json!([]), "member config: fixture must be a JSON object"),
            (
                json!({"filters": []}),
                "member config: fixture: filters must be a JSON object",
            ),
            (
                json!({"filters": {"pasted_prefixes": "x"}}),
                "member config: fixture: filters.pasted_prefixes must be a list of strings",
            ),
            (
                json!({"filters": {"pasted_prefixes": [1]}}),
                "member config: fixture: filters.pasted_prefixes must be a list of strings",
            ),
            (
                json!({"queue_answerer": ""}),
                "member config: fixture: queue_answerer must be a non-empty string",
            ),
            (
                json!({"queue_answerer": false}),
                "member config: fixture: queue_answerer must be a non-empty string",
            ),
            (
                json!({"queue_answerer": null}),
                "member config: fixture: queue_answerer must be a non-empty string",
            ),
            (
                json!({}),
                "member config: missing name, slug, principal, principal_slug, state_dir, mark_word, filters, filters.excluded_folders, filters.personal, filters.secret, filters.pasted_prefixes",
            ),
        ] {
            assert_eq!(
                Member::from_value(value, "fixture")
                    .unwrap_err()
                    .to_string(),
                message
            );
        }
        let mut value = config();
        value.as_object_mut().unwrap().shift_remove("name");
        value["filters"]
            .as_object_mut()
            .unwrap()
            .shift_remove("secret");
        assert_eq!(
            Member::from_value(value, "fixture")
                .unwrap_err()
                .to_string(),
            "member config: missing name, filters.secret"
        );
        let mut value = config();
        value["name"] = json!(42);
        assert_eq!(
            Member::from_value(value, "fixture")
                .unwrap_err()
                .to_string(),
            "member config: fixture: invalid type: integer `42`, expected a string"
        );
    }

    #[test]
    fn derives_member_fields_and_preserves_unknown_values() {
        let mut value = config();
        value["queues"] = json!(["queue.md"]);
        value["filters"]["extra-filter"] = json!("example");
        value["codebook"] = json!({"categories": ["scope"], "future": true});
        let member = Member::from_value(value, "fixture").unwrap();
        assert_eq!(member.pick_field(), "ari_pick");
        assert_eq!(member.answer_field(), "robin_answer");
        assert_eq!(member.principal_subject(), "they");
        assert_eq!(member.extra["queues"], json!(["queue.md"]));
        assert_eq!(member.filters.extra["extra-filter"], "example");
        let serialized = serde_json::to_value(&member).unwrap();
        assert_eq!(
            serialized["codebook"],
            json!({"categories": ["scope"], "future": true})
        );
        let mut member = member;
        member.principal_pronouns = Some("she/her".into());
        assert_eq!(member.principal_subject(), "she");
    }

    #[test]
    fn template_round_trip_is_byte_identical() {
        let scratch = Scratch::new();
        let path = repo().join("crew/_template/member.json");
        let member = read_member(&path).unwrap();
        let output = scratch.path().join("member.json");
        write_member(&output, &member).unwrap();
        assert_eq!(fs::read(output).unwrap(), fs::read(path).unwrap());
    }

    #[test]
    fn private_filters_extend_and_other_keys_replace() {
        let scratch = Scratch::new();
        let path = scratch.path().join("member.json");
        fs::write(&path, to_python_json(&config()).unwrap()).unwrap();
        fs::write(scratch.path().join("private.json"), r#"{"name":"Mo Lee","filters":{"personal":"private","excluded_folders":"hidden","pasted_prefixes":["Note:"],"future":"extra"}}"#).unwrap();
        let member = read_member(&path).unwrap();
        assert_eq!(member.name, "Mo Lee");
        assert_eq!(member.filters.personal, "(?:work)|(?:private)");
        assert_eq!(member.filters.excluded_folders, "hidden");
        assert_eq!(member.filters.pasted_prefixes, ["You are ", "Note:"]);
        assert_eq!(member.filters.extra["future"], "extra");
    }

    #[test]
    fn private_filter_cannot_replace_non_string_member_filter() {
        let scratch = Scratch::new();
        let path = scratch.path().join("member.json");
        let mut value = config();
        value["filters"]["personal"] = json!(5);
        fs::write(&path, to_python_json(&value).unwrap()).unwrap();
        fs::write(
            scratch.path().join("private.json"),
            r#"{"filters":{"personal":"x"}}"#,
        )
        .unwrap();
        let result = read_member(&path);
        assert!(
            matches!(result, Err(Error::Invalid(ref message))
            if message == &format!("member config: {}: filters.personal must be a string (a regex)", path.display())),
            "expected member filter type error, got {result:?}"
        );
    }

    #[test]
    fn rejects_invalid_private_filter_and_json() {
        let scratch = Scratch::new();
        let path = scratch.path().join("member.json");
        fs::write(&path, to_python_json(&config()).unwrap()).unwrap();
        let private = scratch.path().join("private.json");
        fs::write(&private, r#"{"filters":{"personal":7}}"#).unwrap();
        assert_eq!(
            read_member(&path).unwrap_err().to_string(),
            "member config: private.json: filters.personal must be a string (a regex)"
        );
        fs::write(&private, "[]").unwrap();
        assert_eq!(
            read_member(&path).unwrap_err().to_string(),
            format!("member config: {} must be a JSON object", private.display())
        );
        fs::remove_file(private).unwrap();
        fs::write(&path, "{").unwrap();
        assert_eq!(
            read_member(&path).unwrap_err().to_string(),
            format!(
                "member config: {} is not JSON (EOF while parsing an object at line 1 column 1)",
                path.display()
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_member_and_private_symlinks() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let real = scratch.path().join("real.json");
        fs::write(&real, to_python_json(&config()).unwrap()).unwrap();
        let member = scratch.path().join("member.json");
        symlink(&real, &member).unwrap();
        assert!(
            matches!(read_member(&member), Err(Error::Invalid(message)) if message.contains(&member.display().to_string()) && message.contains("symlink"))
        );
        fs::remove_file(&member).unwrap();
        fs::copy(&real, &member).unwrap();
        let private = scratch.path().join("private.json");
        symlink(scratch.path().join("absent"), &private).unwrap();
        assert!(
            matches!(read_member(&member), Err(Error::Invalid(message)) if message.contains(&private.display().to_string()) && message.contains("symlink"))
        );
    }
}
