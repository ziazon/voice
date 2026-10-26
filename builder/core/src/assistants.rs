use crate::{
    crew::{self, NewMember},
    error::{Error, Result},
    member::{Member, read_member},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub const SHOWN_REPO: &str = "~/.claude";
pub fn crew_dir(home: &Path) -> PathBuf {
    home.join(".claude/crew")
}
pub fn assistant_dir(home: &Path, slug: &str) -> Result<PathBuf> {
    if crew::slugify(slug).as_deref() != Some(slug) {
        return Err(Error::Invalid("invalid assistant slug".into()));
    }
    Ok(crew_dir(home).join(slug))
}
#[derive(Debug)]
pub struct Listed {
    pub dir: PathBuf,
    pub member: Member,
}
pub struct New<'a> {
    pub name: &'a str,
    pub pronouns: &'a str,
    pub principal: &'a str,
    pub principal_pronouns: &'a str,
}
pub fn make(home: &Path, new: &New<'_>, date: &str, cli: &str) -> Result<Listed> {
    for (what, value) in [
        ("name", new.name),
        ("pronouns", new.pronouns),
        ("your name", new.principal),
        ("your pronouns", new.principal_pronouns),
    ] {
        crew::check_name(what, value)?;
    }
    let slug = crew::slugify(new.name)
        .ok_or_else(|| Error::Invalid("name must contain a letter or digit".into()))?;
    let dir = assistant_dir(home, &slug)?;
    match fs::symlink_metadata(&dir) {
        Ok(_) => return Err(Error::Exists(slug)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => return Err(Error::Io { path: dir, source }),
    }
    let member = crew::new_member_embedded(
        &dir,
        &NewMember {
            name: new.name,
            pronouns: new.pronouns,
            principal: new.principal,
            principal_pronouns: new.principal_pronouns,
            repo_shown: SHOWN_REPO,
            date,
            decisions: crew::Decisions::Cli(cli),
        },
    )?;
    Ok(Listed { dir, member })
}
pub fn list(home: &Path) -> Result<Vec<Listed>> {
    let crew = crew_dir(home);
    let entries = match fs::read_dir(&crew) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(source) => return Err(Error::Io { path: crew, source }),
    };
    let mut listed = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(slug) = name.to_str() else {
            continue;
        };
        let Ok(dir) = assistant_dir(home, slug) else {
            continue;
        };
        let Ok(member) = read_member(&dir.join("member.json")) else {
            continue;
        };
        if member.slug == slug {
            listed.push(Listed { dir, member });
        }
    }
    listed.sort_by(|a, b| {
        a.member
            .name
            .to_lowercase()
            .cmp(&b.member.name.to_lowercase())
            .then_with(|| a.member.slug.cmp(&b.member.slug))
    });
    Ok(listed)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    fn input(name: &str) -> New<'_> {
        New {
            name,
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
        }
    }
    #[test]
    fn assistant_dir_refuses_noncanonical_slugs() {
        let home = Path::new("home");
        for slug in ["../x", "..", "A", "a--b", "-a", "a/b", "", "_template"] {
            assert!(
                matches!(assistant_dir(home, slug), Err(Error::Invalid(_))),
                "accepted invalid assistant slug: {slug:?}"
            );
        }
        assert_eq!(
            assistant_dir(home, "a-42").unwrap(),
            home.join(".claude/crew/a-42")
        );
    }
    #[test]
    fn make_uses_claude_crew_without_creating_private_state() {
        let home = Scratch::new();
        let made = make(home.path(), &input("Ari"), "2026-10-25", "persona-builder").unwrap();
        assert_eq!(made.dir, home.path().join(".claude/crew/ari"));
        assert_eq!(
            read_member(&made.dir.join("member.json")).unwrap(),
            made.member
        );
        assert!(
            fs::read_to_string(made.dir.join("SKILL.md"))
                .unwrap()
                .contains("~/.claude/crew/ari/")
        );
        assert!(!home.path().join(".claude/state").exists());
    }
    #[test]
    fn make_refuses_existing_folder_untouched() {
        let home = Scratch::new();
        let dir = crew_dir(home.path()).join("ari");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("keep"), "untouched").unwrap();
        assert!(
            matches!(make(home.path(), &input("Ari"), "2026-10-25", "persona-builder"), Err(Error::Exists(slug)) if slug == "ari")
        );
        assert_eq!(fs::read_to_string(dir.join("keep")).unwrap(), "untouched");
    }
    #[cfg(unix)]
    #[test]
    fn make_refuses_dangling_link_untouched() {
        let home = Scratch::new();
        fs::create_dir_all(crew_dir(home.path())).unwrap();
        let dir = crew_dir(home.path()).join("ari");
        let missing = home.path().join("missing");
        std::os::unix::fs::symlink(&missing, &dir).unwrap();
        assert!(
            matches!(make(home.path(), &input("Ari"), "2026-10-25", "persona-builder"), Err(Error::Exists(slug)) if slug == "ari")
        );
        assert_eq!(fs::read_link(dir).unwrap(), missing);
    }
    #[test]
    fn bad_names_are_refused_per_field() {
        let home = Scratch::new();
        for field in ["name", "pronouns", "your name", "your pronouns"] {
            let mut new = input("Ari");
            match field {
                "name" => new.name = "bad@",
                "pronouns" => new.pronouns = "bad@",
                "your name" => new.principal = "bad@",
                _ => new.principal_pronouns = "bad@",
            }
            assert!(
                matches!(make(home.path(), &new, "2026-10-25", "persona-builder"), Err(Error::Invalid(message)) if message.contains(&format!("the {field} "))),
                "wrong validation for {field}"
            );
        }
        assert!(
            matches!(make(home.path(), &input("???"), "2026-10-25", "persona-builder"), Err(Error::Invalid(message)) if message == "name must contain a letter or digit")
        );
        assert!(!crew_dir(home.path()).exists());
    }
    #[test]
    fn list_skips_invalid_members_and_sorts_names_then_slugs() {
        let home = Scratch::new();
        assert!(list(home.path()).unwrap().is_empty());
        for name in ["Zulu", "beta", "Alpha"] {
            make(home.path(), &input(name), "2026-10-25", "persona-builder").unwrap();
        }
        let crew = crew_dir(home.path());
        fs::create_dir(crew.join("empty")).unwrap();
        fs::create_dir(crew.join("damaged")).unwrap();
        fs::write(crew.join("damaged/member.json"), "{").unwrap();
        for name in ["wrong-slug", "_template", ".hidden"] {
            fs::create_dir(crew.join(name)).unwrap();
            fs::copy(
                crew.join("alpha/member.json"),
                crew.join(name).join("member.json"),
            )
            .unwrap();
        }
        assert_eq!(
            list(home.path())
                .unwrap()
                .iter()
                .map(|listed| listed.member.slug.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "beta", "zulu"]
        );
    }
    #[cfg(unix)]
    #[test]
    fn list_follows_links_and_breaks_name_ties_by_slug() {
        let home = Scratch::new();
        let other = Scratch::new();
        make(home.path(), &input("Ari"), "2026-10-25", "persona-builder").unwrap();
        let member = make(
            other.path(),
            &input("Beta"),
            "2026-10-25",
            "persona-builder",
        )
        .unwrap();
        let path = member.dir.join("member.json");
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        value["name"] = serde_json::json!("ari");
        fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
        std::os::unix::fs::symlink(&member.dir, crew_dir(home.path()).join("beta")).unwrap();
        assert_eq!(
            list(home.path())
                .unwrap()
                .iter()
                .map(|listed| listed.member.slug.as_str())
                .collect::<Vec<_>>(),
            ["ari", "beta"]
        );
    }
}
