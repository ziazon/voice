use persona_builder::{
    assistants::{self, New},
    cli_command::shipped_cli_word,
    consent,
    error::{Error, Result},
};
use std::{path::PathBuf, time::SystemTime};

fn today() -> String {
    persona_builder_agent::ledger::local_day(SystemTime::now())
}

pub fn make(name: &str, pronouns: &str, principal: &str, principal_pronouns: &str) -> Result<()> {
    let home = super::home()?;
    let cli = shipped_cli_word(&home)?;
    let listed = assistants::make(
        &home,
        &New {
            name,
            pronouns,
            principal,
            principal_pronouns,
        },
        &today(),
        &cli,
    )
    .map_err(|error| match error {
        Error::Exists(slug) => Error::Invalid(format!(
            "You already have an assistant in ~/.claude/crew/{slug}/."
        )),
        other => other,
    })?;
    if persona_builder_api::clear_consent(&home, &listed.member.slug).is_err() {
        let _ = std::fs::remove_dir_all(&listed.dir);
        return Err(Error::Invalid(
            "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again.".into(),
        ));
    }
    println!(
        "Made {} in ~/.claude/crew/{}/.",
        listed.member.name, listed.member.slug
    );
    println!(
        "Next: persona-builder consent show, then persona-builder consent agree ~/.claude/crew/{}",
        listed.member.slug
    );
    Ok(())
}

pub fn list() -> Result<()> {
    let home = super::home()?;
    let listed = assistants::list(&home)?;
    if listed.is_empty() {
        println!(
            "You have no assistants yet. Make one with: persona-builder new <name> --principal <your name>"
        );
    }
    for assistant in listed {
        let member = assistant.member;
        let permission = match persona_builder_api::read_consent(&home, &member.slug).agreed_on {
            Some(date) => format!("permission given {date}"),
            None => "permission not given yet".into(),
        };
        println!(
            "{} ({}): ~/.claude/crew/{}/ · {permission}",
            member.name, member.slug, member.slug
        );
    }
    Ok(())
}

pub fn show_consent() {
    print!("{}", consent::plain_text());
    println!("Version {}.", consent::VERSION);
}

pub fn agree(member_dir: PathBuf) -> Result<()> {
    let home = super::home()?;
    let member_dir = super::expand_home(member_dir, &home);
    let member = super::checked_member(&member_dir, &home)?;
    let consent = persona_builder_api::write_consent(&home, &member.slug)?;
    let date = consent.agreed_on.ok_or_else(|| Error::Invalid(
        "Persona Builder couldn't keep your answer. Check there's space on this computer, then try again.".into(),
    ))?;
    println!("Thank you. Permission given on {date}.");
    Ok(())
}
