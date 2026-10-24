use clap::{Parser, Subcommand};
use persona_builder::{
    error::{Error, Result},
    install::{self, FileState, Places},
    json::to_python_json,
    member::read_member,
};
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
    /// Delete a member's private history and build drafts.
    Forget {
        member_dir: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// Make files for a claude.ai Project.
    Pack {
        member_dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Turn an assistant on in Claude Code.
    On {
        member_dir: PathBuf,
        #[arg(long)]
        claude_home: Option<PathBuf>,
    },
    /// Turn the assistant off in Claude Code.
    Off {
        #[arg(long)]
        claude_home: Option<PathBuf>,
    },
    /// Show the assistant turned on in Claude Code.
    Status {
        #[arg(long)]
        claude_home: Option<PathBuf>,
    },
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

fn places(claude_home: Option<PathBuf>) -> Result<Places> {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Invalid("HOME is not set".into()))?;
    let home = PathBuf::from(home);
    let claude_home = claude_home.unwrap_or_else(|| {
        std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude"))
    });
    Ok(Places { home, claude_home })
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Forget { member_dir, yes }) => {
            let home = places(None)?.home;
            let member = read_member(&member_dir.join("member.json"))?;
            let forgotten = persona_builder::forget::forget(&member_dir, &home, yes)?;
            if !forgotten.existed {
                println!(
                    "Nothing to forget: {} is not there.",
                    forgotten.path.display()
                );
            } else if yes {
                println!(
                    "Deleted {} files in {}. {}'s own files in {} are untouched.",
                    forgotten.files,
                    forgotten.path.display(),
                    member.name,
                    member_dir.display()
                );
            } else {
                println!(
                    "This deletes {} files in {}: the history {} learned from and the drafts of each build.",
                    forgotten.files,
                    forgotten.path.display(),
                    member.name
                );
                println!(
                    "{}'s own files in {} stay.",
                    member.name,
                    member_dir.display()
                );
                println!(
                    "To go ahead: persona-builder forget {} --yes",
                    member_dir.display()
                );
            }
        }
        Some(Command::Member {
            command: MemberCommand::Show { path },
        }) => {
            print!("{}", to_python_json(&read_member(&path)?)?);
        }
        Some(Command::Pack { member_dir, out }) => {
            let member = read_member(&member_dir.join("member.json"))?;
            let paths = persona_builder::pack::pack(&member_dir, &out)?;
            println!(
                "Made {}'s claude.ai Project pack in {}:",
                member.name,
                out.display()
            );
            for path in paths {
                println!(
                    "  {}",
                    path.file_name().expect("pack file has a name").display()
                );
            }
            println!(
                "To use it: in claude.ai, make a new Project, paste instructions.md into \"Set project"
            );
            println!(
                "instructions\", and add the other files to the Project's knowledge with the + button."
            );
        }
        Some(Command::On {
            member_dir,
            claude_home,
        }) => {
            let on = install::turn_on(&member_dir, &places(claude_home)?)?;
            println!("{} is on in Claude Code.", on.name);
            println!("Added the skill /{} in {}.", on.slug, on.skill.display());
            println!("Added one line to {}:", on.claude_md.display());
            println!("  {}", on.import_line);
            println!(
                "Start a new Claude Code session to use {0}. To turn {0} off: persona-builder off",
                on.name
            );
        }
        Some(Command::Off { claude_home }) => {
            let off = install::turn_off(&places(claude_home)?)?;
            match off.name {
                None => println!("No assistant is on in Claude Code."),
                Some(name) if off.left.is_empty() => {
                    println!("{name} is off. Everything Persona Builder added is removed.")
                }
                Some(name) => {
                    println!("{name} is off. These were left in place:");
                    for left in off.left {
                        println!("  {}: {}", left.path.display(), left.why);
                    }
                }
            }
        }
        Some(Command::Status { claude_home }) => match install::status(&places(claude_home)?)? {
            None => println!("No assistant is on in Claude Code."),
            Some(status) => {
                println!("{} is on in Claude Code.", status.name);
                println!(
                    "The line in CLAUDE.md: {}",
                    if status.line { "there" } else { "missing" }
                );
                for (path, state) in status.files {
                    let state = match state {
                        FileState::Present => "there",
                        FileState::Changed => "changed",
                        FileState::Missing => "missing",
                    };
                    println!("{}: {state}", path.display());
                }
                if status.out_of_date {
                    println!(
                        "{} has changed since it was turned on. To update it: persona-builder on {}",
                        status.name,
                        status.member_dir.display()
                    );
                }
            }
        },
        None => {}
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
