use clap::Subcommand;
use std::{path::PathBuf, process::ExitCode, time::SystemTime};
use voice_core::error::{Error, Result};
use voice_ingest::{
    anonymize::{self, Rules},
    private,
    skills::{self, Agreements},
};

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
    /// Bring your assistant's copy of your skills up to date
    Refresh { member_dir: PathBuf },
    /// Stop copying these skills
    Withdraw {
        member_dir: PathBuf,
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },
}

pub fn run(command: SkillsCommand) -> Result<ExitCode> {
    match command {
        SkillsCommand::Refresh { member_dir } => refresh(member_dir),
        SkillsCommand::List { member_dir } => manage(member_dir, Action::List),
        SkillsCommand::Agree { member_dir, ids } => manage(member_dir, Action::Agree(ids)),
        SkillsCommand::Withdraw { member_dir, ids } => manage(member_dir, Action::Withdraw(ids)),
    }
}

enum Action {
    List,
    Agree(Vec<String>),
    Withdraw(Vec<String>),
}

fn manage(member_dir: PathBuf, action: Action) -> Result<ExitCode> {
    let home = super::home()?;
    let member_dir = super::expand_home(member_dir, &home);
    let member = super::checked_member(&member_dir, &home)?;
    let _lock = private::lock_state(&home, &member.slug)?;
    let rules = Rules::for_member(&member, &home)?;
    let found = skills::discover(&home, &rules)?;
    let mut agreements = Agreements::load(&home, &member.slug)?;
    match action {
        Action::List => {
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
        Action::Agree(ids) => {
            for id in &ids {
                if !found.skills.iter().any(|skill| skill.id == *id) {
                    return Err(Error::Invalid(format!(
                        "{id} isn't one of the skills on this computer. See them with: voice skills list {}",
                        skills::display_path(&member_dir, &home)
                    )));
                }
            }
            let date = voice_agent::ledger::local_day(SystemTime::now());
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
        Action::Withdraw(ids) => {
            let mut withdrew = false;
            for id in &ids {
                withdrew |= agreements.withdraw(id);
            }
            agreements.save(&home, &member.slug)?;
            for id in ids {
                println!("{id}: no longer copied.");
            }
            if withdrew && private::private_folder_exists(&home, &member.slug, "mirror")? {
                let mirror = private::private_folder(&home, &member.slug, "mirror")?;
                let member_path = anonymize::report_path(
                    &PathBuf::from(skills::display_path(&member_dir, &home)),
                    &rules,
                );
                if matches!(skills::remove_copy(&mirror), Ok(true)) {
                    println!(
                        "Your assistant's copy of your skills was removed, so nothing you withdrew stays in it. Make it again with: voice skills refresh {}",
                        member_path.display()
                    );
                } else {
                    println!(
                        "Voice couldn't remove your assistant's copy of your skills. Move anything you want to keep out of the folder {}, delete the folder, then run voice skills refresh {}",
                        anonymize::report_path(
                            &PathBuf::from(skills::display_path(&mirror, &home)),
                            &rules
                        )
                        .display(),
                        member_path.display()
                    );
                    return Ok(ExitCode::FAILURE);
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn refresh(member_dir: PathBuf) -> Result<ExitCode> {
    let home = super::home()?;
    let member_dir = super::expand_home(member_dir, &home);
    let member = super::checked_member(&member_dir, &home)?;
    let _lock = private::lock_state(&home, &member.slug)?;
    let date = voice_agent::ledger::local_day(SystemTime::now());
    let refreshed = skills::refresh(&home, &member, &date)?;
    let copied = &refreshed.copied;
    print!("{}", copied.lines);
    println!(
        "Copied {} of {} skills into your assistant's copy, {}.",
        copied.copied,
        copied.agreed,
        anonymize::report_path(
            &PathBuf::from(skills::display_path(&refreshed.mirror, &home)),
            &refreshed.rules
        )
        .display()
    );
    println!(
        "Since the last refresh: {} new, {} updated, {} unchanged, {} removed.",
        refreshed.new, refreshed.updated, refreshed.unchanged, refreshed.removed
    );
    if copied.waiting > 0 {
        println!(
            "{} skills wait for your yes: voice skills list {}",
            copied.waiting,
            anonymize::report_path(
                &PathBuf::from(skills::display_path(&member_dir, &home)),
                &refreshed.rules
            )
            .display()
        );
    }
    Ok(if copied.copied == copied.agreed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub fn anonymize(member_dir: PathBuf, out: PathBuf) -> Result<ExitCode> {
    let home = super::home()?;
    let member_dir = super::expand_home(member_dir, &home);
    let member = super::checked_member(&member_dir, &home)?;
    let _lock = private::lock_state(&home, &member.slug)?;
    let (rules, found, agreements, mut mapping) = skills::load_copy(&home, &member)?;
    let out = skills::prepare_output_with_projects(
        &super::expand_home(out, &home),
        &home,
        &found.skills,
        rules.projects(),
    )?;
    let copied = skills::copy_agreed(
        &home,
        &member.slug,
        &rules,
        &found,
        &agreements,
        &mut mapping,
        &out,
    )?;
    let date = voice_agent::ledger::local_day(SystemTime::now());
    skills::write_manifest(&out, &copied.manifest.to_json(&date))?;
    print!("{}", copied.lines);
    let skills::Copied {
        agreed,
        copied,
        waiting,
        ..
    } = copied;
    println!(
        "Copied {copied} of {agreed} skills into {}.",
        anonymize::report_path(&PathBuf::from(skills::display_path(&out, &home)), &rules).display()
    );
    if waiting > 0 {
        println!(
            "{waiting} skills wait for your yes: voice skills list {}",
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
