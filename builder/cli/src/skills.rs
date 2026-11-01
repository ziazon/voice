use clap::Subcommand;
use persona_builder::error::{Error, Result};
use persona_builder_ingest::{
    anonymize::{self, Mapping, Rules},
    private,
    skills::{self, Agreements, State},
};
use std::{collections::BTreeMap, path::PathBuf, process::ExitCode, time::SystemTime};

#[derive(Subcommand)]
pub enum SkillsCommand {
    /// Skills on this computer, and which ones your assistant may copy
    List { member_dir: PathBuf },
    /// Let your assistant copy these skills, with private details taken out
    Agree {
        member_dir: PathBuf,
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },
    /// Stop copying these skills
    Withdraw {
        member_dir: PathBuf,
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },
}

pub fn run(command: SkillsCommand) -> Result<()> {
    let home = super::home()?;
    let member_dir = match &command {
        SkillsCommand::List { member_dir }
        | SkillsCommand::Agree { member_dir, .. }
        | SkillsCommand::Withdraw { member_dir, .. } => member_dir,
    };
    let member_dir = super::expand_home(member_dir.clone(), &home);
    let member = super::checked_member(&member_dir, &home)?;
    let _lock = private::lock_state(&home, &member.slug)?;
    let rules = Rules::for_member(&member, &home)?;
    let found = skills::discover(&home, &rules)?;
    let mut agreements = Agreements::load(&home, &member.slug)?;
    match command {
        SkillsCommand::List { .. } => {
            if found.skills.is_empty() {
                println!("No skills found on this computer.");
            }
            for (index, skill) in found.skills.iter().enumerate() {
                println!(
                    "{}. {}\t{}\t{}",
                    index + 1,
                    skill.id,
                    agreements.state(skill, &rules).words(),
                    skill.shown
                );
            }
            for note in found.notes {
                println!("{note}");
            }
        }
        SkillsCommand::Agree { ids, .. } => {
            for id in &ids {
                if !found.skills.iter().any(|skill| skill.id == *id) {
                    return Err(Error::Invalid(format!(
                        "{id} isn't one of the skills on this computer. See them with: persona-builder skills list {}",
                        skills::display_path(&member_dir, &home)
                    )));
                }
            }
            let date = persona_builder_agent::ledger::local_day(SystemTime::now());
            for id in &ids {
                let skill = found
                    .skills
                    .iter()
                    .find(|skill| skill.id == *id)
                    .expect("checked id");
                agreements.agree(skill, &date);
            }
            agreements.save(&home, &member.slug)?;
            for id in ids {
                let skill = found
                    .skills
                    .iter()
                    .find(|skill| skill.id == id)
                    .expect("checked id");
                let suffix = if rules.never_copy(&skill.name) {
                    " It is on your never_copy list, so it still won't be copied."
                } else {
                    ""
                };
                println!("{id}: agreed {date}.{suffix}");
            }
        }
        SkillsCommand::Withdraw { ids, .. } => {
            for id in &ids {
                agreements.withdraw(id);
            }
            agreements.save(&home, &member.slug)?;
            for id in ids {
                println!("{id}: no longer copied.");
            }
        }
    }
    Ok(())
}

pub fn anonymize(member_dir: PathBuf, out: PathBuf) -> Result<ExitCode> {
    let home = super::home()?;
    let member_dir = super::expand_home(member_dir, &home);
    let member = super::checked_member(&member_dir, &home)?;
    let _lock = private::lock_state(&home, &member.slug)?;
    let rules = Rules::for_member(&member, &home)?;
    let found = skills::discover(&home, &rules)?;
    let agreements = Agreements::load(&home, &member.slug)?;
    let mut mapping = match private::read_private_optional_at_max(
        &home,
        &member.slug,
        Some("anonymizer"),
        "placeholders.json",
        4 << 20,
    )? {
        Some(bytes) => Mapping::from_json(&bytes)?,
        None => Mapping::default(),
    };
    let out = skills::prepare_output_with_projects(
        &super::expand_home(out, &home),
        &home,
        &found.skills,
        rules.projects(),
    )?;
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut agreed = 0;
    let mut copied = 0;
    let mut waiting = 0;
    for (index, skill) in found.skills.iter().enumerate() {
        match agreements.state(skill, &rules) {
            State::Agreed(_) => {}
            State::Waiting | State::NotOffered | State::Moved => {
                waiting += 1;
                continue;
            }
            State::NeverCopied => continue,
        }
        agreed += 1;
        if anonymize::name_holds_private_detail(&skill.name, &rules) {
            println!(
                "A skill you agreed to: not copied, its name holds a private detail (number {} in persona-builder skills list)",
                index + 1
            );
            continue;
        }
        let name_key = skill.name.to_lowercase();
        if let Some(other) = names.get(&name_key) {
            println!(
                "{}: not copied, another skill you agreed to has the same name ({})",
                anonymize::report_id(&skill.id, &rules),
                anonymize::report_id(other, &rules)
            );
            continue;
        }
        names.insert(name_key, skill.id.clone());
        let files = match skills::read_with_rules(skill, &rules) {
            Ok(files) => files,
            Err(Error::Invalid(message))
                if message == skills::SKILL_TOO_LARGE || message == skills::SKILL_UNREADABLE =>
            {
                println!("{}: not copied, {}", skill.name, message);
                continue;
            }
            Err(error) => return Err(error),
        };
        let result = anonymize::skill(&files, &rules, &mut mapping);
        private::write_private_at(
            &home,
            &member.slug,
            Some("anonymizer"),
            "placeholders.json",
            &mapping.to_json(),
        )?;
        if result.refused.is_empty() {
            skills::write_skill(&out, &skill.name, &result.kept)?;
            copied += 1;
        }
        print!("{}", anonymize::report(&skill.name, &result));
    }
    private::write_private_at(
        &home,
        &member.slug,
        Some("anonymizer"),
        "placeholders.json",
        &mapping.to_json(),
    )?;
    println!(
        "Copied {copied} of {agreed} skills into {}.",
        anonymize::report_path(&PathBuf::from(skills::display_path(&out, &home)), &rules).display()
    );
    if waiting > 0 {
        println!(
            "{waiting} skills wait for your yes: persona-builder skills list {}",
            anonymize::report_path(
                &PathBuf::from(skills::display_path(&member_dir, &home)),
                &rules
            )
            .display()
        );
    }
    Ok(if copied == agreed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
