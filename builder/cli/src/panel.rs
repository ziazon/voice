//! Arguments for asking a panel of your assistants.
use clap::Subcommand;
use persona_builder::error::{Error, Result};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PanelCommand {
    /// Ask your assistants a question together
    Convene {
        #[arg(long)]
        question_file: PathBuf,
        #[arg(long, value_parser = kind)]
        kind: String,
        #[arg(long = "seat", required = true, action = clap::ArgAction::Append)]
        seats: Vec<String>,
        #[arg(long, value_parser = ["15m", "1h", "deadlock"])]
        time_box: Option<String>,
    },
    /// Carry on a panel, or ask it to reconsider
    Resume {
        id: String,
        #[arg(long)]
        steer_file: Option<PathBuf>,
        #[arg(long, value_parser = ["15m", "1h", "deadlock"])]
        time_box: Option<String>,
    },
    /// Show how a panel is doing, with its rounds and result
    Status {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// List your panels
    List,
}
fn kind(value: &str) -> std::result::Result<String, String> {
    if value == "text"
        || value
            .strip_prefix("list:")
            .filter(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=12).contains(&n))
    {
        Ok(value.into())
    } else {
        Err(persona_builder_api::panels::BAD_KIND.into())
    }
}
fn read(path: &std::path::Path) -> Result<String> {
    persona_builder_build::draft::read_text(path, 8 * 1024 * 1024)
        .map_err(|_| Error::Invalid(persona_builder_api::panels::text::READ_INPUT.into()))?
        .ok_or_else(|| Error::Invalid(persona_builder_api::panels::text::READ_INPUT.into()))
}
fn question(path: &std::path::Path) -> Result<(String, String)> {
    let text = read(path)?;
    let mut lines = text.lines().skip_while(|line| line.trim().is_empty());
    let question = lines.next().unwrap_or_default().trim().to_owned();
    Ok((
        question,
        lines.collect::<Vec<_>>().join("\n").trim().to_owned(),
    ))
}
fn finish(
    result: persona_builder_api::dto::PanelRan,
    printed: bool,
) -> Result<std::process::ExitCode> {
    use persona_builder_api::panels::text;
    if !printed {
        println!("{}", result.id);
    }
    let code = match result.status.as_str() {
        "closed" => {
            println!(
                "{}",
                text::cli_closed(result.outcome.as_deref().unwrap_or_default())
            );
            let home =
                std::env::var_os("HOME").ok_or_else(|| Error::Invalid(text::READ_FAILED.into()))?;
            println!(
                "{}",
                PathBuf::from(home)
                    .join(".claude/voice/panels")
                    .join(result.id)
                    .join("result.md")
                    .display()
            );
            0
        }
        "paused" => {
            println!("{}", text::cli_paused());
            3
        }
        "stopped" => {
            println!("{}", text::cli_stopped());
            3
        }
        _ => {
            eprintln!("{}", text::FAILED);
            1
        }
    };
    Ok(std::process::ExitCode::from(code))
}
fn execute(command: PanelCommand) -> Result<std::process::ExitCode> {
    use persona_builder_api::{self as api, dto::*, panels::text};
    let mut printed = false;
    let mut progress = |progress: PanelProgress| {
        if !printed {
            println!("{}", progress.id);
            printed = true;
        }
        println!("{}", progress.line);
    };
    let result = match command {
        PanelCommand::Convene {
            question_file,
            kind,
            seats,
            time_box,
        } => {
            let (question, background) = question(&question_file)?;
            api::convene_panel(
                ConvenePanelRequest {
                    question,
                    background,
                    kind,
                    seats,
                    time_box,
                },
                &mut progress,
            )
        }
        PanelCommand::Resume {
            id,
            steer_file,
            time_box,
        } => {
            let steer = steer_file.as_deref().map(read).transpose()?;
            api::resume_panel(
                ResumePanelRequest {
                    id,
                    steer,
                    time_box,
                },
                &mut progress,
            )
        }
        PanelCommand::Status { id, json } => {
            let detail = api::panel(PanelIdRequest { id })
                .map_err(|failure| Error::Invalid(failure.message))?;
            if json {
                let value = serde_json::to_string_pretty(&detail)
                    .map_err(|_| Error::Invalid(text::READ_FAILED.into()))?;
                println!("{value}");
            } else {
                let row = detail.summary;
                println!(
                    "{}",
                    text::cli_status(&row.status, row.round, row.outcome.as_deref())
                );
            }
            return Ok(std::process::ExitCode::SUCCESS);
        }
        PanelCommand::List => {
            for row in api::list_panels().map_err(|failure| Error::Invalid(failure.message))? {
                println!("{}", text::cli_summary(&row.id, &row.status, &row.question));
            }
            return Ok(std::process::ExitCode::SUCCESS);
        }
    };
    match result {
        Ok(result) => finish(result, printed),
        Err(failure) => {
            eprintln!("{}", failure.message);
            Ok(std::process::ExitCode::from(failure_code(&failure.message)))
        }
    }
}

pub fn run(command: PanelCommand) -> Result<std::process::ExitCode> {
    match execute(command) {
        Err(Error::Invalid(message))
            if [
                persona_builder_api::panels::text::READ_INPUT,
                persona_builder_api::panels::text::BAD_ID,
            ]
            .contains(&message.as_str()) =>
        {
            eprintln!("{message}");
            Ok(std::process::ExitCode::from(2))
        }
        result => result,
    }
}

fn failure_code(message: &str) -> u8 {
    use persona_builder_api::panels::text;
    if [text::BUSY, text::ANSWER_FIRST].contains(&message)
        || message.ends_with(text::ASSISTANT_BUSY)
    {
        3
    } else if [
        text::BAD_ID,
        text::BAD_KIND,
        text::BAD_SEATS,
        text::BAD_QUESTION,
        text::BAD_BACKGROUND,
        text::BAD_TIME,
        text::NO_ASK,
        text::NEED_STEER,
        text::READ_INPUT,
    ]
    .contains(&message)
        || [
            text::NO_ASSISTANT,
            text::NOT_BUILT_ASSISTANT,
            text::PERMISSION,
        ]
        .iter()
        .any(|reason| message.ends_with(reason))
        || message.starts_with(text::INPUT_PREFIX)
    {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::failure_code;
    use persona_builder_api::panels::text;

    #[test]
    fn failure_codes_separate_waiting_bad_input_and_a_failed_call() {
        assert_eq!(failure_code(text::BUSY), 3);
        assert_eq!(failure_code(text::ANSWER_FIRST), 3);
        assert_eq!(failure_code(text::BAD_KIND), 2);
        assert_eq!(failure_code(text::NEED_STEER), 2);
        assert_eq!(failure_code(text::FAILED), 1);
        assert_eq!(failure_code(persona_builder_agent::text::DOES_NOT_RUN), 1);
    }
}
