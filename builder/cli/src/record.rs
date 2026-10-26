//! Keep live decisions in the assistant's existing private state folder.
use clap::{Arg, Command};
use persona_builder::error::{Error, Result};
use persona_builder_ingest::{mine::to_jsonl, private::append_private};
use serde_json::{Map, Value};
use std::path::PathBuf;

pub fn run(member_dir: PathBuf, args: Vec<String>) -> Result<()> {
    let (member, home) = crate::member_and_home(&member_dir)?;
    let pick_flag = format!("{}-pick", member.slug);
    let answer_flag = format!("{}-answer", member.principal_slug);
    // clap keeps argument names for the life of the command; these member-specific names
    // belong to this invocation, rather than being static flags on the outer parser.
    let mut command = Command::new("persona-builder record");
    for name in [
        "work", "kind", "category", "tier", "question", "pick", "rule", "answer",
    ] {
        let flag = match name {
            "pick" => pick_flag.as_str(),
            "answer" => answer_flag.as_str(),
            other => other,
        };
        let mut arg = Arg::new(name)
            .long(flag.to_owned())
            .required(!matches!(name, "rule" | "answer"));
        if name == "kind" {
            arg = arg.value_parser(["decided", "escalated"]);
        }
        if name == "tier" {
            arg = arg.value_parser(["high", "medium", "low"]);
        }
        command = command.arg(arg);
    }
    let matches = command
        .try_get_matches_from(std::iter::once("persona-builder record".to_owned()).chain(args))
        .unwrap_or_else(|error| error.exit());
    let mut row = Map::new();
    row.insert(
        "timestamp".into(),
        Value::String(persona_builder_grow::store::utc_now()),
    );
    for name in ["work", "kind", "category", "tier", "question"] {
        row.insert(
            name.into(),
            Value::String(
                matches
                    .get_one::<String>(name)
                    .expect("required argument")
                    .clone(),
            ),
        );
    }
    row.insert(
        member.pick_field(),
        Value::String(
            matches
                .get_one::<String>("pick")
                .expect("required argument")
                .clone(),
        ),
    );
    for (key, name) in [
        ("rule".to_owned(), "rule"),
        (member.answer_field(), "answer"),
    ] {
        row.insert(
            key,
            matches
                .get_one::<String>(name)
                .cloned()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
    }
    let line = to_jsonl(&[row]);
    if line.len() > 64 * 1024 {
        return Err(Error::Invalid(
            "That is too long to keep (over 64 KiB). Shorten it, then record it again.".into(),
        ));
    }
    append_private(&home, &member.slug, "live.jsonl", line.as_bytes()).map_err(|error| {
        match error {
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => Error::Invalid(format!(
                "{} has nowhere to keep this yet: ~/.claude/state/{} isn't there. Open Persona Builder and give {} permission, then try again.", member.name, member.slug, member.name)),
            other => other,
        }
    })?;
    println!(
        "recorded a {} {} decision ({}) for {}",
        matches
            .get_one::<String>("kind")
            .expect("required argument"),
        matches
            .get_one::<String>("category")
            .expect("required argument"),
        matches
            .get_one::<String>("tier")
            .expect("required argument"),
        matches
            .get_one::<String>("work")
            .expect("required argument")
    );
    Ok(())
}
