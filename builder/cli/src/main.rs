use clap::{Parser, Subcommand};
use persona_builder::error::Error;
use persona_builder::{error::Result, json::to_python_json, member::read_member};
use persona_builder_save::{
    flow::{self, Choice, GithubRemotes, MemberPaths, NotBuiltYet, RemoteCheck, Saved, Saver},
    git::Git,
    github::{self, Endpoints, Github, UreqHttp},
    keychain::{Keychain, TokenStore},
    record::SaveRecord,
    secret::Secret,
    text,
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
    /// Choose where to keep an assistant and save it.
    Save {
        #[command(subcommand)]
        command: SaveCommand,
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

#[derive(Subcommand)]
enum SaveCommand {
    /// Show where your assistant can be kept.
    #[command(verbatim_doc_comment)]
    Options { path: PathBuf },
    /// Sign in to GitHub, so your assistant can be saved there.
    #[command(verbatim_doc_comment)]
    SignIn,
    /// Sign out of GitHub on this Mac.
    #[command(verbatim_doc_comment)]
    SignOut,
    /// Choose where your assistant is kept.
    #[command(verbatim_doc_comment)]
    Choose {
        path: PathBuf,
        #[command(subcommand)]
        choice: SaveChoice,
    },
    /// Save your assistant where you chose.
    #[command(verbatim_doc_comment)]
    Push { path: PathBuf },
}

#[derive(Subcommand)]
enum SaveChoice {
    /// Keep it on this Mac only.
    #[command(verbatim_doc_comment)]
    Local,
    /// Save it in a GitHub repo your AI setup already uses.
    #[command(verbatim_doc_comment)]
    Here { folder: PathBuf },
    /// Save it to a new private GitHub repo with this name.
    #[command(verbatim_doc_comment)]
    Github { name: String },
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| Error::Io {
            path: "~".into(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "HOME"),
        })
}
fn github_client(http: &UreqHttp) -> Result<Github<'_>> {
    Github::new(
        http,
        Endpoints::github(),
        github::client_id().ok_or_else(|| Error::Invalid(text::NOT_SET_UP.into()))?,
    )
}
fn save_options(path: PathBuf) -> Result<()> {
    let home = home()?;
    let member = MemberPaths::load(&path, &home)?;
    let options = flow::options(&flow::detect(&Git::system(), &home, &member.dir)?, &home);
    for (index, choice) in options.choices.iter().enumerate() {
        let label = match choice {
            Choice::Existing { label, .. } | Choice::Github { label } | Choice::Local { label } => {
                label
            }
        };
        let recommended = if index == options.selected {
            " (recommended)"
        } else {
            ""
        };
        println!("{}. {label}{recommended}", index + 1);
    }
    if let Some(note) = options.note {
        println!("{note}");
    }
    Ok(())
}
fn save_sign_in() -> Result<()> {
    let http = UreqHttp::default();
    let github = github_client(&http)?;
    let code = github.start_sign_in()?;
    println!(
        "{}",
        text::sign_in_prompt(&code.verification_uri, &code.user_code)
    );
    println!("{}", text::SIGN_IN_WAITING);
    let token = github.finish_sign_in(&code, &mut std::thread::sleep)?;
    let account = github.account(&token)?;
    Keychain::github().save(&token)?;
    println!("{}", text::signed_in(&account.login));
    Ok(())
}
fn save_sign_out() -> Result<()> {
    Keychain::github().forget()?;
    println!("{}", text::SIGNED_OUT);
    Ok(())
}
fn save_choose_local(path: PathBuf) -> Result<()> {
    let member = MemberPaths::load(&path, &home()?)?;
    Saver {
        git: &Git::system(),
        github: None,
        tokens: &Keychain::github(),
    }
    .choose_local(&member)?;
    println!("{}", text::CHOSE_LOCAL);
    Ok(())
}
fn save_choose_existing(path: PathBuf, folder: PathBuf) -> Result<()> {
    let http = UreqHttp::default();
    let github = github_client(&http)?;
    let home = home()?;
    let member = MemberPaths::load(&path, &home)?;
    let record = Saver {
        git: &Git::system(),
        github: Some(&github),
        tokens: &Keychain::github(),
    }
    .choose_existing(&member, &home, &folder, &GithubRemotes { github: &github })?;
    let SaveRecord::Existing { repo, .. } = record else {
        unreachable!("choose_existing returns an existing save record");
    };
    println!("{}", text::chose_existing(&flow::shown(&repo, &home)));
    Ok(())
}
fn save_choose_github(path: PathBuf, name: String) -> Result<()> {
    let http = UreqHttp::default();
    let github = github_client(&http)?;
    let member = MemberPaths::load(&path, &home()?)?;
    let repo = Saver {
        git: &Git::system(),
        github: Some(&github),
        tokens: &Keychain::github(),
    }
    .choose_github(&member, &name)?;
    println!("{}", text::chose_github(&repo.full()));
    Ok(())
}
struct UnavailableRemotes;
impl RemoteCheck for UnavailableRemotes {
    fn confirm(&self, _token: &Secret, _urls: &[String]) -> Result<()> {
        Err(Error::Invalid(text::NOT_SET_UP.into()))
    }
}
fn save_push(path: PathBuf) -> Result<()> {
    let home = home()?;
    let member = MemberPaths::load(&path, &home)?;
    let http = UreqHttp::default();
    let github = github::client_id()
        .map(|id| Github::new(&http, Endpoints::github(), id))
        .transpose()?;
    let remotes = github.as_ref().map(|github| GithubRemotes { github });
    let check: &dyn RemoteCheck = match &remotes {
        Some(remotes) => remotes,
        None => &UnavailableRemotes,
    };
    let saved = Saver {
        git: &Git::system(),
        github: github.as_ref(),
        tokens: &Keychain::github(),
    }
    .save_now(&member, &home, &NotBuiltYet, check)?;
    match saved {
        Saved::KeptLocal => println!("{}", text::KEPT_LOCAL),
        Saved::Pushed { message } => println!("{message}"),
    }
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Member {
            command: MemberCommand::Show { path },
        }) => {
            print!("{}", to_python_json(&read_member(&path)?)?);
            Ok(())
        }
        Some(Command::Save { command }) => match command {
            SaveCommand::Options { path } => save_options(path),
            SaveCommand::SignIn => save_sign_in(),
            SaveCommand::SignOut => save_sign_out(),
            SaveCommand::Choose { path, choice } => match choice {
                SaveChoice::Local => save_choose_local(path),
                SaveChoice::Here { folder } => save_choose_existing(path, folder),
                SaveChoice::Github { name } => save_choose_github(path, name),
            },
            SaveCommand::Push { path } => save_push(path),
        },
        None => Ok(()),
    }
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
