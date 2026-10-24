use crate::{
    error::{Error, Result},
    member::Member,
};
use std::path::{Path, PathBuf};

/// Matches install.sh run under `LC_ALL=C`.
/// A UTF-8 locale's `tr` may lowercase some non-ASCII letters differently.
pub fn slugify(text: &str) -> Option<String> {
    let mut slug = String::new();
    let mut separator = false;
    for byte in text.bytes() {
        let byte = byte.to_ascii_lowercase();
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(char::from(byte));
            separator = false;
        } else {
            separator = true;
        }
    }
    if slug.is_empty() { None } else { Some(slug) }
}

pub(crate) fn check_name(what: &str, value: &str) -> Result<()> {
    if !(1..=64).contains(&value.chars().count())
        || value.trim() != value
        || value
            .chars()
            .any(|c| c.is_control() || "@<>{}[]()#\\`".contains(c))
    {
        return Err(Error::Invalid(format!(
            "the {what} {value:?} can't be used: a name is one line of up to 64 characters without @ < > {{ }} [ ] ( ) # \\ or backticks"
        )));
    }
    Ok(())
}

pub fn shown_path(repo: &Path, home: &Path) -> Result<String> {
    let repo = repo.canonicalize().map_err(|source| Error::Io {
        path: repo.to_owned(),
        source,
    })?;
    let home = home.canonicalize().map_err(|source| Error::Io {
        path: home.to_owned(),
        source,
    })?;
    Ok(match repo.strip_prefix(&home) {
        Ok(relative) if relative.as_os_str().is_empty() => "~/.".into(),
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => repo.display().to_string(),
    })
}

pub struct NewMember<'a> {
    pub name: &'a str,
    pub pronouns: &'a str,
    pub principal: &'a str,
    pub principal_pronouns: &'a str,
    pub repo_shown: &'a str,
    pub date: &'a str,
}

pub fn new_member(template: &Path, target: &Path, new: &NewMember<'_>) -> Result<Member> {
    match std::fs::symlink_metadata(target) {
        Ok(_) => return Err(Error::Invalid(format!("{} exists", target.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(Error::Io {
                path: target.to_owned(),
                source,
            });
        }
    }
    let slug = slugify(new.name)
        .ok_or_else(|| Error::Invalid("name must contain a letter or digit".into()))?;
    let principal_slug = slugify(new.principal)
        .ok_or_else(|| Error::Invalid("principal must contain a letter or digit".into()))?;
    let values = [
        ("NAME", new.name),
        ("SLUG", &slug),
        ("PRONOUNS", new.pronouns),
        ("PRINCIPAL", new.principal),
        ("PRINCIPAL_SLUG", &principal_slug),
        ("PRINCIPAL_PRONOUNS", new.principal_pronouns),
        ("REPO", new.repo_shown),
        ("DATE", new.date),
    ];
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_owned(),
            source,
        })?;
    }
    std::fs::create_dir(target).map_err(|source| Error::Io {
        path: target.to_owned(),
        source,
    })?;
    let result = copy_template(template, target, &values, true)
        .and_then(|()| crate::member::read_member(&target.join("member.json")));
    if result.is_err() {
        let _ = std::fs::remove_dir_all(target);
    }
    result
}

pub(crate) fn fill(text: &str, values: &[(&str, &str)]) -> String {
    let mut filled = text.to_owned();
    for (key, value) in values {
        filled = filled.replace(&format!("{{{{{key}}}}}"), value);
    }
    filled
}

fn fill_json(value: serde_json::Value, values: &[(&str, &str)]) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::String(text) => Value::String(fill(&text, values)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| fill_json(item, values))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (fill(&key, values), fill_json(value, values)))
                .collect(),
        ),
        other => other,
    }
}

fn copy_template(
    source: &Path,
    target: &Path,
    values: &[(&str, &str)],
    top_level: bool,
) -> Result<()> {
    use std::fs;
    crate::member::refuse_symlink(source)?;
    let entries = fs::read_dir(source).map_err(|error| Error::Io {
        path: source.to_owned(),
        source: error,
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| Error::Io {
            path: source.to_owned(),
            source: error,
        })?;
        let path = entry.path();
        crate::member::refuse_symlink(&path)?;
        let destination = target.join(entry.file_name());
        let file_type = entry.file_type().map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            fs::create_dir(&destination).map_err(|source| Error::Io {
                path: destination.clone(),
                source,
            })?;
            copy_template(&path, &destination, values, false)?;
            continue;
        }
        if top_level && entry.file_name() == "README.md" {
            continue;
        }
        if !top_level && entry.file_name() == "member.json" {
            fs::copy(&path, &destination).map_err(|source| Error::Io {
                path: destination,
                source,
            })?;
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        let filled = if top_level && entry.file_name() == "member.json" {
            let value = serde_json::from_str(&text).map_err(|error| {
                Error::Invalid(format!(
                    "member config: {} is not JSON ({error})",
                    path.display()
                ))
            })?;
            crate::json::to_python_json(&fill_json(value, values))?
        } else {
            fill(&text, values)
        };
        fs::write(&destination, filled).map_err(|source| Error::Io {
            path: destination,
            source,
        })?;
    }
    Ok(())
}

pub struct MemberDir {
    pub dir: PathBuf,
    pub member: Member,
    pub has_lens: bool,
}

impl MemberDir {
    pub fn open(dir: &Path) -> Result<Self> {
        let member = crate::member::read_member(&dir.join("member.json"))?;
        for file in ["IDENTITY.md", "SKILL.md", "autonomy.md", "decisions.md"] {
            if !dir.join(file).is_file() {
                return Err(Error::Invalid(format!("{}: missing {file}", dir.display())));
            }
        }
        Ok(Self {
            dir: dir.to_owned(),
            member,
            has_lens: dir.join("lens.md").is_file(),
        })
    }
    pub fn autonomy(&self) -> Result<crate::autonomy::Autonomy> {
        let path = self.dir.join("autonomy.md");
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Io { path, source })?;
        crate::autonomy::Autonomy::parse(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Scratch, repo};
    use std::fs;

    fn input() -> NewMember<'static> {
        NewMember {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
            repo_shown: "~/projects/voice",
            date: "2026-10-24",
        }
    }

    #[test]
    fn slugs_use_ascii_bytes_and_trim_runs() {
        for (text, expected) in [
            ("Mo Lee", Some("mo-lee")),
            (" --ARI!! ", Some("ari")),
            ("Zoë", Some("zo")),
            ("éA🦀Bé", Some("a-b")),
            ("💬_", None),
            ("", None),
            ("42", Some("42")),
        ] {
            assert_eq!(slugify(text).as_deref(), expected);
        }
    }

    #[test]
    fn paths_are_canonical_and_home_relative() {
        let scratch = Scratch::new();
        let home = scratch.path().join("home");
        let repo = home.join("projects/voice");
        fs::create_dir_all(&repo).unwrap();
        assert_eq!(
            shown_path(&repo.join("..").join("voice"), &home).unwrap(),
            "~/projects/voice"
        );
        assert_eq!(
            shown_path(scratch.path(), &home).unwrap(),
            scratch.path().canonicalize().unwrap().display().to_string()
        );
        assert_eq!(shown_path(&home, &home).unwrap(), "~/.");
    }

    #[test]
    fn shown_path_reports_missing_paths_without_panicking() {
        let scratch = Scratch::new();
        let missing = scratch.path().join("missing");
        for (repo, home) in [
            (missing.as_path(), scratch.path()),
            (scratch.path(), missing.as_path()),
        ] {
            let result = std::panic::catch_unwind(|| shown_path(repo, home));
            assert!(
                result.is_ok(),
                "shown_path panicked for {}",
                missing.display()
            );
            let error = result.unwrap().unwrap_err();
            assert!(error.to_string().contains(&missing.display().to_string()));
            assert!(matches!(error, Error::Io { path, source }
                if path == missing && source.kind() == std::io::ErrorKind::NotFound));
        }
    }

    #[test]
    fn new_member_opens_and_reports_first_missing_file() {
        let scratch = Scratch::new();
        let target = scratch.path().join("ari");
        let member = new_member(&repo().join("crew/_template"), &target, &input()).unwrap();
        assert_eq!(member.name, "Ari");
        assert!(!target.join("README.md").exists());
        let opened = MemberDir::open(&target).unwrap();
        assert_eq!(opened.member, member);
        assert_eq!(opened.dir, target);
        assert!(!opened.has_lens);
        assert_eq!(opened.autonomy().unwrap().dials().len(), 13);
        fs::write(target.join("lens.md"), "lens").unwrap();
        assert!(MemberDir::open(&target).unwrap().has_lens);
        for file in ["IDENTITY.md", "SKILL.md", "autonomy.md", "decisions.md"] {
            fs::remove_file(target.join(file)).unwrap();
            assert_eq!(
                MemberDir::open(&target).err().unwrap().to_string(),
                format!("{}: missing {file}", target.display())
            );
            fs::write(target.join(file), "restored").unwrap();
        }
    }

    #[test]
    fn new_member_refuses_existing_target_and_empty_slugs() {
        let scratch = Scratch::new();
        let template = repo().join("crew/_template");
        assert_eq!(
            new_member(&template, scratch.path(), &input())
                .unwrap_err()
                .to_string(),
            format!("{} exists", scratch.path().display())
        );
        let target = scratch.path().join("new");
        let mut new = input();
        new.name = "???";
        assert_eq!(
            new_member(&template, &target, &new)
                .unwrap_err()
                .to_string(),
            "name must contain a letter or digit"
        );
        new.name = "Ari";
        new.principal = "!!!";
        assert_eq!(
            new_member(&template, &target, &new)
                .unwrap_err()
                .to_string(),
            "principal must contain a letter or digit"
        );
        assert!(!target.exists());
    }

    #[test]
    fn nested_member_json_is_copied_byte_for_byte() {
        let scratch = Scratch::new();
        let template = scratch.path().join("template");
        fs::create_dir_all(template.join("nested")).unwrap();
        fs::write(
            template.join("member.json"),
            serde_json::to_string(&crate::test_support::config()).unwrap(),
        )
        .unwrap();
        let bytes = br#"{"who":"{{NAME}}"}"#;
        fs::write(template.join("nested/member.json"), bytes).unwrap();
        let target = scratch.path().join("output");
        new_member(&template, &target, &input()).unwrap();
        assert_eq!(fs::read(target.join("nested/member.json")).unwrap(), bytes);
    }

    #[test]
    fn placeholders_fill_json_keys_lists_and_in_order() {
        let scratch = Scratch::new();
        let template = scratch.path().join("template");
        fs::create_dir_all(template.join("nested")).unwrap();
        let mut config = crate::test_support::config();
        config["principal"] = serde_json::json!("{{PRINCIPAL}}");
        config["{{NAME}}"] = serde_json::json!(["{{PRINCIPAL}}", {"{{SLUG}}": "{{DATE}}"}]);
        fs::write(
            template.join("member.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .unwrap();
        fs::write(template.join("README.md"), "skip").unwrap();
        fs::write(
            template.join("nested/README.md"),
            "{{PRINCIPAL}} {{PRINCIPAL_SLUG}} {{REPO}}",
        )
        .unwrap();
        let target = scratch.path().join("output");
        let mut new = input();
        new.principal = "{{PRINCIPAL_SLUG}}";
        let member = new_member(&template, &target, &new).unwrap();
        assert_eq!(member.principal, "principal-slug");
        assert_eq!(
            member.extra["Ari"],
            serde_json::json!(["principal-slug", {"ari": "2026-10-24"}])
        );
        assert_eq!(
            fs::read_to_string(target.join("nested/README.md")).unwrap(),
            "principal-slug principal-slug ~/projects/voice"
        );
        assert!(!target.join("README.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn new_member_removes_target_after_nested_symlink_failure() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let template = scratch.path().join("template");
        fs::create_dir_all(template.join("briefs")).unwrap();
        fs::write(template.join("source.md"), "source").unwrap();
        let link = template.join("briefs/link.md");
        symlink("../source.md", &link).unwrap();
        let target = scratch.path().join("new");
        let error = new_member(&template, &target, &input()).unwrap_err();
        assert!(matches!(error, Error::Invalid(message)
            if message.contains("symlink") && message.contains(&link.display().to_string())));
        assert!(!target.exists(), "failed new_member left its target behind");
    }

    #[test]
    fn copy_template_does_not_create_the_top_level_target() {
        let scratch = Scratch::new();
        let template = scratch.path().join("template");
        fs::create_dir(&template).unwrap();
        fs::write(template.join("file.md"), "text").unwrap();
        let target = scratch.path().join("new");
        let result = copy_template(&template, &target, &[], true);
        assert!(
            result.is_err(),
            "copy_template must require a target owned by new_member"
        );
        assert!(!target.exists());
    }

    #[test]
    fn new_member_removes_target_after_read_member_failure() {
        let scratch = Scratch::new();
        let template = scratch.path().join("template");
        fs::create_dir(&template).unwrap();
        fs::write(template.join("member.json"), "{}").unwrap();
        let target = scratch.path().join("new");
        let error = new_member(&template, &target, &input()).unwrap_err();
        assert!(matches!(error, Error::Invalid(message) if message.contains("missing")));
        assert!(!target.exists(), "failed new_member left its target behind");
    }

    #[cfg(unix)]
    #[test]
    fn new_member_refuses_template_symlinks() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let template = scratch.path().join("template");
        fs::create_dir(&template).unwrap();
        let file = template.join("linked");
        symlink(scratch.path().join("absent"), &file).unwrap();
        let result = new_member(&template, &scratch.path().join("new"), &input());
        assert!(
            matches!(result, Err(Error::Invalid(message)) if message.contains("symlink") && message.contains(&file.display().to_string()))
        );
    }
}
