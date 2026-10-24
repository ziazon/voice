use std::{fs, path::Path};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPack {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub decisions: Vec<(String, String)>,
    pub words: Vec<String>,
}

impl FieldPack {
    pub fn parse(text: &str, categories: &[String]) -> Result<Self> {
        let mut lines = text.lines();
        if lines.next() != Some("---") {
            return Err(Error::Invalid("field pack: missing front matter".into()));
        }
        let label = text
            .lines()
            .find_map(|line| line.strip_prefix("id: "))
            .unwrap_or("<unknown>");
        let invalid = |problem: &str| Error::Invalid(format!("field pack {label}: {problem}"));
        let mut metadata = std::collections::HashMap::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line == "---" {
                closed = true;
                break;
            }
            let (key, value) = line
                .split_once(": ")
                .ok_or_else(|| invalid("invalid front matter line"))?;
            if !["id", "name", "summary"].contains(&key) {
                return Err(invalid(&format!("unknown front matter key {key:?}")));
            }
            if value.trim().is_empty() {
                return Err(invalid(&format!("{key} must not be empty")));
            }
            if metadata.insert(key, value).is_some() {
                return Err(invalid(&format!("{key} appears twice")));
            }
        }
        if !closed {
            return Err(invalid("unclosed front matter"));
        }
        for key in ["id", "name", "summary"] {
            if !metadata.contains_key(key) {
                return Err(Error::Invalid(format!(
                    "field pack {}: missing {key}",
                    metadata.get("id").unwrap_or(&"<unknown>")
                )));
            }
        }
        if !valid_id(metadata["id"]) {
            return Err(invalid("id must match [a-z0-9-]+"));
        }
        let mut pack = Self {
            id: metadata.get("id").unwrap_or(&"").to_string(),
            name: metadata.get("name").unwrap_or(&"").to_string(),
            summary: metadata.get("summary").unwrap_or(&"").to_string(),
            decisions: Vec::new(),
            words: Vec::new(),
        };
        let mut section = "";
        for line in lines {
            if let Some(heading) = line.strip_prefix("## ") {
                section = heading;
                continue;
            }
            if section == "Typical decisions" {
                if let Some((category, text)) = line
                    .strip_prefix("- `")
                    .and_then(|rest| rest.split_once("`: "))
                {
                    if !categories.iter().any(|known| known == category) {
                        return Err(Error::Invalid(format!(
                            "field pack {}: unknown category {category:?}",
                            pack.id
                        )));
                    }
                    if pack.decisions.iter().any(|(known, _)| known == category) {
                        return Err(Error::Invalid(format!(
                            "field pack {}: repeated category {category:?}",
                            pack.id
                        )));
                    }
                    pack.decisions.push((category.into(), text.into()));
                }
            } else if section == "Words"
                && let Some(word) = line.strip_prefix("- ")
            {
                pack.words.push(word.into());
            }
        }
        if pack.decisions.is_empty() {
            return Err(Error::Invalid(format!(
                "field pack {}: Typical decisions list is empty",
                pack.id
            )));
        }
        if pack.words.is_empty() {
            return Err(Error::Invalid(format!(
                "field pack {}: Words list is empty",
                pack.id
            )));
        }
        Ok(pack)
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub fn load(dir: &Path, id: &str, categories: &[String]) -> Result<FieldPack> {
    if !valid_id(id) {
        return Err(Error::Invalid(format!(
            "field pack {id}: id must match [a-z0-9-]+"
        )));
    }
    let path = dir.join(format!("{id}.md"));
    let metadata = fs::symlink_metadata(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(Error::Invalid(format!(
            "field pack {id}: {} is a symlink",
            path.display()
        )));
    }
    let text = fs::read_to_string(&path).map_err(|source| Error::Io { path, source })?;
    let pack = FieldPack::parse(&text, categories)?;
    if pack.id != id {
        return Err(Error::Invalid(format!(
            "field pack {id}: id is {}",
            pack.id
        )));
    }
    Ok(pack)
}

pub fn list(dir: &Path) -> Result<Vec<String>> {
    let entries = fs::read_dir(dir).map_err(|source| Error::Io {
        path: dir.into(),
        source,
    })?;
    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: dir.into(),
            source,
        })?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if kind.is_file()
            && path.extension().is_some_and(|extension| extension == "md")
            && let Some(id) = path.file_stem().and_then(|stem| stem.to_str())
        {
            ids.push(id.into());
        }
    }
    ids.sort();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_field_pack_loads_template_categories() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crew/_fields");
        let template: serde_json::Value =
            serde_json::from_str(include_str!("../../../crew/_template/member.json")).unwrap();
        let categories: Vec<String> = template["codebook"]["categories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect();
        let loaded = load(&dir, "general", &categories);
        assert!(loaded.is_ok(), "{loaded:?}");
        let pack = loaded.unwrap();
        assert_eq!(pack.id, "general");
        assert_eq!(pack.name, "Any line of work");
        assert_eq!(pack.decisions.len(), 12);
        assert_eq!(pack.words.len(), 10);
    }
    const PACK: &str = "---\nid: general\nname: General\nsummary: A start\n---\n\n## Typical decisions\n\n- `scope`: choose\n\n## Words\n\n- draft\n";

    fn categories() -> Vec<String> {
        vec!["scope".into()]
    }

    #[test]
    fn field_pack_refuses_unknown_category() {
        let result = FieldPack::parse(&PACK.replace("`scope`", "`x`"), &categories());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "field pack general: unknown category \"x\""
        );
    }

    #[test]
    fn field_pack_refuses_repeated_category() {
        let text = PACK.replace("- `scope`: choose", "- `scope`: choose\n- `scope`: repeat");
        let result = FieldPack::parse(&text, &categories());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "field pack general: repeated category \"scope\""
        );
    }

    #[test]
    fn field_pack_refuses_missing_front_matter_key() {
        let result = FieldPack::parse(&PACK.replace("summary: A start\n", ""), &categories());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "field pack general: missing summary"
        );
    }
    #[test]
    fn field_pack_refuses_empty_words() {
        let result = FieldPack::parse(&PACK.replace("- draft\n", ""), &categories());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "field pack general: Words list is empty"
        );
    }

    #[test]
    fn field_pack_refuses_empty_decisions() {
        let text = PACK.replace("- `scope`: choose\n", "");
        let result = FieldPack::parse(&text, &categories());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "field pack general: Typical decisions list is empty"
        );
    }

    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "pb1-fields-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn field_pack_load_refuses_mismatched_id() {
        let scratch = Scratch::new();
        fs::write(scratch.0.join("other.md"), PACK).unwrap();
        let result = load(&scratch.0, "other", &categories());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "field pack other: id is general"
        );
    }

    #[test]
    fn field_pack_list_is_sorted_markdown_ids() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crew/_fields");
        assert_eq!(list(&dir).unwrap(), ["general"]);
        let scratch = Scratch::new();
        for file in ["z.md", "a.md", "ignored.txt"] {
            fs::write(scratch.0.join(file), "").unwrap();
        }
        fs::create_dir(scratch.0.join("directory.md")).unwrap();
        assert_eq!(list(&scratch.0).unwrap(), ["a", "z"]);
    }

    #[test]
    fn field_pack_front_matter_is_strict() {
        let cases = [
            PACK.replace("id: general", "id: General"),
            PACK.replace("name: General", "name: "),
            PACK.replace("name: General", "name: General\nname: Again"),
            PACK.replace("summary: A start", "summary: A start\nextra: nope"),
            PACK.replacen("---\n\n##", "\n##", 1),
        ];
        let refused: Vec<bool> = cases
            .iter()
            .map(|text| FieldPack::parse(text, &categories()).is_err())
            .collect();
        assert_eq!(refused, [true; 5]);
    }

    #[test]
    fn field_pack_load_refuses_symlinks_and_invalid_path_ids() {
        let scratch = Scratch::new();
        fs::write(scratch.0.join("general.md"), PACK).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(scratch.0.join("general.md"), scratch.0.join("linked.md"))
                .unwrap();
            let result = load(&scratch.0, "linked", &categories());
            assert!(
                matches!(result, Err(Error::Invalid(ref message)) if message.contains("symlink") && message.contains("linked.md"))
            );
        }
        let nested = scratch.0.join("nested");
        fs::create_dir(&nested).unwrap();
        for id in ["../general", "", "General", "a/b"] {
            let result = load(&nested, id, &categories());
            assert!(
                matches!(result, Err(Error::Invalid(ref message)) if message.contains("id must match")),
                "{id}: {result:?}"
            );
        }
    }
}
