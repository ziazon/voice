use std::{path::PathBuf, time::SystemTime};
use voice_core::{
    assistants::{self, New},
    cli_command::shipped_cli_word,
    consent, crew,
    error::{Error, Result},
    existing,
};

fn today() -> String {
    voice_agent::ledger::local_day(SystemTime::now())
}

fn checked_name(name: &str) -> Result<String> {
    crew::check_name("name", name)?;
    crew::slugify(name).ok_or_else(|| Error::Invalid("name must contain a letter or digit".into()))
}

pub fn make(
    name: &str,
    pronouns: &str,
    principal: &str,
    principal_pronouns: &str,
    bring_in: bool,
    leave: &[String],
    claude_home: Option<PathBuf>,
) -> Result<()> {
    let places = super::places(claude_home)?;
    let home = places.home;
    for (what, value) in [
        ("name", name),
        ("pronouns", pronouns),
        ("your name", principal),
        ("your pronouns", principal_pronouns),
    ] {
        crew::check_name(what, value)?;
    }
    let slug = checked_name(name)?;
    if !bring_in && existing::find(&home, &places.claude_home, &slug)?.any() {
        return Err(Error::Invalid(format!(
            "Voice found an older {name} on this computer, so it made nothing. See what it found: voice existing {name}. To back it up and bring it in: voice new {name} --bring-in (with the same options)."
        )));
    }
    let cli = shipped_cli_word(&home)?;
    let new = New {
        name,
        pronouns,
        principal,
        principal_pronouns,
    };
    let result = if bring_in {
        let found = existing::find(&home, &places.claude_home, &slug)?;
        let offered = match found.old_folder {
            Some(folder) => voice_api::assistants::offer_old_files(&home, &folder)?,
            None => Vec::new(),
        };
        for file in leave {
            if !offered.iter().any(|offered| offered.name == file) {
                return Err(Error::Invalid(format!(
                    "Voice found no {file} to leave out. See what it found: voice existing {name}"
                )));
            }
        }
        let files: Vec<String> = offered
            .into_iter()
            .filter(|file| file.problem.is_none() && !leave.iter().any(|name| name == file.name))
            .map(|file| file.name.to_owned())
            .collect();
        voice_api::assistants::bring_in(
            &home,
            &places.claude_home,
            &new,
            &today(),
            &voice_agent::ledger::local_stamp(SystemTime::now()),
            &cli,
            &files,
        )
    } else {
        assistants::make(&home, &new, &today(), &cli).and_then(|listed| {
            if voice_api::clear_consent(&home, &listed.member.slug).is_err() {
                let _ = std::fs::remove_dir_all(&listed.dir);
                return Err(Error::Invalid(
                    "Voice couldn't make the assistant's folder. Check there's space on this computer, then try again.".into(),
                ));
            }
            Ok((listed, None, Vec::new()))
        })
    };
    let (listed, backup, files) = result.map_err(|error| match error {
        Error::Exists(slug) => Error::Invalid(format!(
            "You already have an assistant in ~/.claude/crew/{slug}/."
        )),
        other => other,
    })?;
    println!(
        "Made {} in ~/.claude/crew/{}/.",
        listed.member.name, listed.member.slug
    );
    if let Some(backup) = backup {
        let relative = backup
            .strip_prefix(&home)
            .map_err(|_| Error::Invalid("Voice couldn't show the backup folder.".into()))?;
        println!(
            "Voice backed up the older {name} first, in ~/{}.",
            relative.display()
        );
    }
    if !files.is_empty() {
        println!(
            "It brought in the older {name}'s own files: {}.",
            files.join(", ")
        );
    }
    println!(
        "Next: voice consent show, then voice consent agree ~/.claude/crew/{}",
        listed.member.slug
    );
    Ok(())
}

pub fn list() -> Result<()> {
    let home = super::home()?;
    let listed = assistants::list(&home)?;
    if listed.is_empty() {
        println!(
            "You have no assistants yet. Make one with: voice new <name> --principal <your name>"
        );
    }
    for assistant in listed {
        let member = assistant.member;
        let permission = match voice_api::read_consent(&home, &member.slug).agreed_on {
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
    let consent = voice_api::write_consent(&home, &member.slug)?;
    let date = consent.agreed_on.ok_or_else(|| Error::Invalid(
        "Voice couldn't keep your answer. Check there's space on this computer, then try again.".into(),
    ))?;
    println!("Thank you. Permission given on {date}.");
    Ok(())
}

pub fn existing(name: &str, claude_home: Option<PathBuf>) -> Result<()> {
    let places = super::places(claude_home)?;
    let slug = checked_name(name)?;
    let dir = assistants::assistant_dir(&places.home, &slug)?;
    if std::fs::symlink_metadata(&dir).is_ok() {
        println!("You already have an assistant in ~/.claude/crew/{slug}/.");
        return Ok(());
    }
    let found = existing::find(&places.home, &places.claude_home, &slug)?;
    if !found.any() {
        println!("Nothing from before is called {name} on this computer.");
        return Ok(());
    }
    let refusal = voice_api::assistants::bring_in_refusal(&places.home, &found)?;
    if let Some(state) = found.state {
        let detail = if state.link {
            "a link to somewhere else".into()
        } else if state.not_folder {
            "a file, not a folder".into()
        } else {
            let noun = if state.files == 1 { "file" } else { "files" };
            let mut detail = format!("{} {noun}", state.files);
            if state.corpus {
                detail.push_str(", a put-together history (decisions.jsonl)");
            }
            if state.live_log {
                detail.push_str(", a log of recorded answers (live.jsonl)");
            }
            detail
        };
        println!("Private folder: {} ({detail})", state.path.display());
    }
    if let Some(skill) = found.skill {
        println!("Skill Voice didn't add: {}", skill.display());
    }
    if let Some(path) = found.claude_md {
        let lines = found
            .lines
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "CLAUDE.md lines naming /{slug}: {} lines {lines}",
            path.display()
        );
    }
    let offered = if let Some(folder) = found.old_folder {
        println!("Folder it was made in: {}", folder.display());
        voice_api::assistants::offer_old_files(&places.home, &folder)?
    } else {
        Vec::new()
    };
    for file in &offered {
        match file.problem {
            Some(problem) => println!("Leaves out {}: {problem}", file.name),
            None if file.name == "private.json" => println!(
                "Can bring in private.json: Its privacy settings: what the older assistant hid, and any words it let through."
            ),
            None => println!("Can bring in {}", file.name),
        }
    }
    if let Some(reason) = refusal {
        println!("{reason}");
    } else if offered.is_empty() {
        println!("Voice can back it up and bring it in: voice new {name} --bring-in");
    } else {
        println!(
            "Voice can back it up and bring it in: voice new {name} --bring-in (add --leave <file> to leave a file out)"
        );
    }
    Ok(())
}
