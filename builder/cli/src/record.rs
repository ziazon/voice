//! Keep live decisions in the assistant's existing private state folder.
use clap::{Arg, ArgAction, Command};
use persona_builder::error::{Error, Result};
use persona_builder_ingest::{mine::to_jsonl, private::append_private};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordInput {
    work: String,
    kind: String,
    category: String,
    tier: String,
    question: String,
    pick: String,
    rule: Option<String>,
    answer: Option<String>,
}

pub fn run(member_dir: PathBuf, args: Vec<String>) -> Result<()> {
    if member_dir == std::path::Path::new("--help") || member_dir == std::path::Path::new("-h") {
        record_command("<assistant>-pick", "<you>-answer")
            .print_help()
            .map_err(|source| Error::Io {
                path: member_dir,
                source,
            })?;
        println!();
        return Ok(());
    }
    let (member, home) = crate::member_and_home(&member_dir)?;
    let pick_flag = format!("{}-pick", member.slug);
    let answer_flag = format!("{}-answer", member.principal_slug);
    let mut command = record_command(&pick_flag, &answer_flag);
    let argv: Vec<_> = std::iter::once("persona-builder record".to_owned())
        .chain(args)
        .collect();
    let matches = command
        .try_get_matches_from_mut(&argv)
        .unwrap_or_else(|error| error.exit());
    let mut values: Vec<_> = command
        .get_arguments()
        .filter(|arg| arg.get_id() != "stdin")
        .filter_map(|arg| {
            matches
                .index_of(arg.get_id().as_str())
                .map(|index| (index, arg))
        })
        .collect();
    values.sort_by_key(|(index, _)| *index);
    // clap counts --flag=value as two indices; argv keeps it in one token.
    let mut joined = 0;
    for (index, arg) in values {
        let flag = arg.get_long().expect("record flag");
        let explicit = argv[index - joined - 1].starts_with(&format!("--{flag}="));
        if explicit {
            joined += 1;
        } else if option_like_value(
            matches
                .get_one::<String>(arg.get_id().as_str())
                .expect("record value"),
        ) {
            command
                .clone()
                .error(
                    clap::error::ErrorKind::InvalidValue,
                    format!("a value is required for --{flag}"),
                )
                .exit();
        }
    }
    let input = if matches.get_flag("stdin") {
        let input: RecordInput = crate::stdin_json::read(&command);
        for (valid, message) in [
            (
                ["decided", "escalated"].contains(&input.kind.as_str()),
                "kind must be decided or escalated",
            ),
            (
                ["high", "medium", "low"].contains(&input.tier.as_str()),
                "tier must be high, medium or low",
            ),
        ] {
            if !valid {
                command
                    .clone()
                    .error(clap::error::ErrorKind::ValueValidation, message)
                    .exit();
            }
        }
        input
    } else {
        let required = |name: &str| {
            matches
                .get_one::<String>(name)
                .expect("required argument")
                .clone()
        };
        RecordInput {
            work: required("work"),
            kind: required("kind"),
            category: required("category"),
            tier: required("tier"),
            question: required("question"),
            pick: required("pick"),
            rule: matches.get_one::<String>("rule").cloned(),
            answer: matches.get_one::<String>("answer").cloned(),
        }
    };
    let mut row = Map::new();
    row.insert(
        "timestamp".into(),
        Value::String(persona_builder_grow::store::utc_now()),
    );
    for (name, value) in [
        ("work", &input.work),
        ("kind", &input.kind),
        ("category", &input.category),
        ("tier", &input.tier),
        ("question", &input.question),
    ] {
        row.insert(name.into(), Value::String(value.clone()));
    }
    row.insert(member.pick_field(), Value::String(input.pick));
    for (key, value) in [
        ("rule".to_owned(), input.rule),
        (member.answer_field(), input.answer),
    ] {
        row.insert(key, value.map(Value::String).unwrap_or(Value::Null));
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
        input.kind, input.category, input.tier, input.work
    );
    Ok(())
}

/// Stricter than argparse on purpose: a value starting with two dashes must be
/// given as --flag=value. Otherwise, a leading dash is option-like only when the
/// value is longer than one character, has no space, and is not a negative number
/// matching ^-\d+$|^-\d*\.\d+$.
fn option_like_value(value: &str) -> bool {
    if value.starts_with("--") {
        return true;
    }
    let Some(number) = value.strip_prefix('-') else {
        return false;
    };
    let digits = |s: &str| s.bytes().all(|byte| byte.is_ascii_digit());
    let negative = !number.is_empty() && digits(number)
        || number.split_once('.').is_some_and(|(whole, fraction)| {
            digits(whole) && !fraction.is_empty() && digits(fraction)
        });
    !number.is_empty() && !value.contains(' ') && !negative
}

fn record_command(pick_flag: &str, answer_flag: &str) -> Command {
    // Member-specific names belong to this invocation of the command.
    let mut command = Command::new("persona-builder record")
        .about("Keep one decision the assistant made, or your answer to one it handed back.")
        .arg(
            Arg::new("stdin")
                .long("stdin")
                .action(ArgAction::SetTrue)
                .help("Read the texts as one JSON object from stdin"),
        );
    for name in [
        "work", "kind", "category", "tier", "question", "pick", "rule", "answer",
    ] {
        let flag = match name {
            "pick" => pick_flag,
            "answer" => answer_flag,
            other => other,
        };
        let mut arg = Arg::new(name)
            .long(flag.to_owned())
            .allow_hyphen_values(true)
            .conflicts_with("stdin");
        if !matches!(name, "rule" | "answer") {
            arg = arg.required_unless_present("stdin");
        }
        if name == "kind" {
            arg = arg.value_parser(["decided", "escalated"]);
        }
        if name == "tier" {
            arg = arg.value_parser(["high", "medium", "low"]);
        }
        command = command.arg(arg);
    }
    command
}
