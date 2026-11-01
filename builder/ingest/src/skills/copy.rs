//! One copying loop for exports and the assistant's own mirror.
use super::{Agreements, Found, Manifest, State};
use crate::{
    anonymize::{self, Mapping, Rules},
    private, skills,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt::Write, path::Path};
use voice_core::{
    error::{Error, Result},
    member::Member,
};

pub struct Copied {
    /// Per-skill reports followed by discovery notes, each ending in a newline.
    pub lines: String,
    pub agreed: usize,
    pub copied: usize,
    /// Skills waiting for a yes (waiting, not offered, or moved).
    pub waiting: usize,
    pub manifest: Manifest,
}

pub fn load_copy(home: &Path, member: &Member) -> Result<(Rules, Found, Agreements, Mapping)> {
    let rules = Rules::for_member(member, home)?;
    let (found, agreements, mapping) = load_agreed(home, member, &rules)?;
    Ok((rules, found, agreements, mapping))
}

pub(super) fn load_agreed(
    home: &Path,
    member: &Member,
    rules: &Rules,
) -> Result<(Found, Agreements, Mapping)> {
    let found = skills::discover(home, rules)?;
    let agreements = Agreements::load(home, &member.slug)?;
    let mapping = match private::read_private_optional_at_max(
        home,
        &member.slug,
        Some("anonymizer"),
        "placeholders.json",
        4 << 20,
    )? {
        Some(bytes) => Mapping::from_json(&bytes)?,
        None => Mapping::default(),
    };
    Ok((found, agreements, mapping))
}

pub fn copy_agreed(
    home: &Path,
    slug: &str,
    rules: &Rules,
    found: &Found,
    agreements: &Agreements,
    mapping: &mut Mapping,
    out: &Path,
) -> Result<Copied> {
    let mut lines = String::new();
    let mut manifest = Manifest::default();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut agreed = 0;
    let mut copied = 0;
    let mut waiting = 0;
    for (index, skill) in found.skills.iter().enumerate() {
        match agreements.state(skill, rules) {
            State::Agreed(_) => {}
            State::Waiting | State::NotOffered | State::Moved => {
                waiting += 1;
                continue;
            }
            State::NeverCopied => continue,
        }
        agreed += 1;
        if anonymize::name_holds_private_detail(&skill.name, rules) {
            writeln!(lines,
                "A skill you agreed to: not copied, its name holds a private detail (number {} in voice skills list)",
                index + 1
            ).expect("write to String");
            continue;
        }
        let name_key = skill.name.to_lowercase();
        if let Some(other) = names.get(&name_key) {
            writeln!(
                lines,
                "{}: not copied, another skill you agreed to has the same name ({})",
                anonymize::report_id(&skill.id, rules),
                anonymize::report_id(other, rules)
            )
            .expect("write to String");
            continue;
        }
        names.insert(name_key, skill.id.clone());
        let files = match skills::read_with_rules(skill, rules) {
            Ok(files) => files,
            Err(Error::Invalid(message))
                if message == skills::SKILL_TOO_LARGE || message == skills::SKILL_UNREADABLE =>
            {
                writeln!(lines, "{}: not copied, {}", skill.name, message)
                    .expect("write to String");
                continue;
            }
            Err(error) => return Err(error),
        };
        let result = anonymize::skill(&files, rules, mapping);
        private::write_private_at(
            home,
            slug,
            Some("anonymizer"),
            "placeholders.json",
            &mapping.to_json(),
        )?;
        if result.refused.is_empty() {
            skills::write_skill(out, &skill.name, &result.kept)?;
            manifest.skills.insert(
                skill.name.clone(),
                result
                    .kept
                    .iter()
                    .map(|(path, text)| {
                        (
                            path.components()
                                .map(|part| part.as_os_str().to_string_lossy())
                                .collect::<Vec<_>>()
                                .join("/"),
                            format!("{:x}", Sha256::digest(text.as_bytes())),
                        )
                    })
                    .collect(),
            );
            copied += 1;
        }
        lines.push_str(&anonymize::report(&skill.name, &result));
    }
    private::write_private_at(
        home,
        slug,
        Some("anonymizer"),
        "placeholders.json",
        &mapping.to_json(),
    )?;
    for note in &found.notes {
        writeln!(lines, "{note}").expect("write to String");
    }
    Ok(Copied {
        lines,
        agreed,
        copied,
        waiting,
        manifest,
    })
}
