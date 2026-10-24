use clap::{Parser, Subcommand};
use persona_builder::{error::Result, json::to_python_json, member::read_member};
use std::path::PathBuf;

/// The `persona-builder` command: a thin layer over the core library.
#[derive(Parser)]
#[command(
    name = "persona-builder",
    version = persona_builder::VERSION,
    about = "Build your AI assistant from your own Claude history"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Read a member's configuration.
    Member {
        #[command(subcommand)]
        command: MemberCommand,
    },
}

#[derive(Subcommand)]
enum MemberCommand {
    /// Show the member configuration, including private overrides.
    Show { path: PathBuf },
}

fn run(cli: Cli) -> Result<()> {
    if let Some(Command::Member {
        command: MemberCommand::Show { path },
    }) = cli.command
    {
        print!("{}", to_python_json(&read_member(&path)?)?);
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("persona-builder: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
