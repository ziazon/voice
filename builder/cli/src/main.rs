use clap::{Parser, Subcommand};
use persona_builder::{
    error::{Error, Result},
    install::{self, FileState, Places},
    json::to_python_json,
    member::read_member,
};
use persona_builder_save::{
    flow::{self, Choice, GithubRemotes, MemberPaths, RemoteCheck, Saved, Saver},
    git::Git,
    github::{self, Endpoints, Github, UreqHttp},
    keychain::{Keychain, TokenStore},
    record::SaveRecord,
    secret::Secret,
    text,
};
use persona_builder_steps::input::RecordList as Records;
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
    /// Check an assistant's folder for anything private before it is shared.
    PrivacyCheck { folder: PathBuf },
    /// Read your Claude Code history into your assistant's private folder.
    Mine {
        member_dir: PathBuf,
        #[arg(long)]
        projects: Option<PathBuf>,
        #[arg(long, default_value = "decisions-claude-code.jsonl")]
        out: String,
        #[arg(long, conflicts_with = "cutoff", allow_hyphen_values = true)]
        holdout: Option<String>,
        #[arg(long)]
        cutoff: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Combine your histories into one set of decisions, and set aside the newest to test with.
    Merge {
        member_dir: PathBuf,
        #[arg(long = "in", num_args = 1.., action = clap::ArgAction::Append)]
        inputs: Vec<String>,
        #[arg(long, num_args = 1.., action = clap::ArgAction::Append)]
        train: Vec<String>,
        #[arg(long, num_args = 1.., action = clap::ArgAction::Append)]
        rounds: Vec<String>,
        #[arg(long, default_value = "decisions.jsonl")]
        out: String,
        #[arg(long, conflicts_with = "cutoff", allow_hyphen_values = true)]
        holdout: Option<String>,
        #[arg(long)]
        cutoff: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Set aside the held-out questions and their answers for the test.
    Split {
        member_dir: PathBuf,
        #[arg(long, default_value = "decisions.jsonl")]
        corpus: String,
        #[arg(long, default_value = "heldout-questions.jsonl")]
        questions: String,
        #[arg(long, default_value = "heldout-answers.jsonl")]
        answers: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Cut your history into parts small enough to label.
    Chunk {
        member_dir: PathBuf,
        #[arg(long, default_value = "decisions.jsonl")]
        corpus: String,
        #[arg(long, default_value = "chunks")]
        out_dir: String,
        #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..))]
        size: u64,
    },
    /// Check one part's labels against the assistant's kinds of decision.
    CheckLabels {
        member_dir: PathBuf,
        #[arg(long)]
        chunk: String,
        #[arg(long)]
        labels: String,
        #[arg(long, value_parser = ["train", "heldout"])]
        mode: String,
    },
    /// Count how you decided, for each kind of decision.
    Tally {
        member_dir: PathBuf,
        #[arg(long, default_value = "decisions.jsonl")]
        corpus: String,
        #[arg(long)]
        labels: String,
        #[arg(long)]
        json: bool,
    },
    /// Group your labelled decisions, ready to be written up.
    DistilIn {
        member_dir: PathBuf,
        #[arg(long, default_value = "decisions.jsonl")]
        corpus: String,
        #[arg(long)]
        labels: String,
        #[arg(long, default_value = "distil")]
        out_dir: String,
    },
    /// Score the test's guesses against what you really chose.
    Score {
        member_dir: PathBuf,
        #[arg(long, default_value = "heldout-answers.jsonl")]
        answers: String,
        #[arg(long)]
        labels: String,
        #[arg(long)]
        predictions: String,
        #[arg(long)]
        json: bool,
    },
    /// Find the past decisions closest to a question.
    Similar {
        member_dir: PathBuf,
        #[arg(long, default_value = "decisions.jsonl")]
        corpus: String,
        #[arg(long)]
        query: String,
        #[arg(long, default_value_t = 6)]
        k: usize,
        #[arg(long)]
        json: bool,
    },
    /// Check that a draft repeats no run of words from your history.
    Shingle {
        member_dir: PathBuf,
        #[arg(long)]
        doc: PathBuf,
        #[arg(long = "corpus", num_args = 1.., required = true)]
        corpora: Vec<String>,
        #[arg(long, default_value_t = 6, value_parser = clap::value_parser!(u64).range(1..))]
        n: u64,
    },
    /// Add a round of questions you answered outside Claude.
    Round {
        member_dir: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        questions: String,
        #[arg(long)]
        answers: String,
        #[arg(long)]
        heldout: bool,
        #[arg(long)]
        out: Option<String>,
    },
    /// Read your claude.ai chats: check the export, list the chats, mark which are yours, and read them.
    Claudeai {
        #[command(subcommand)]
        command: ClaudeaiCommand,
    },
    /// List the Claude Code project folders and which are left out.
    Folders {
        member_dir: PathBuf,
        #[arg(long)]
        projects: Option<PathBuf>,
    },
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
    /// List the agents Persona Builder can build with.
    Agents,
    /// Check your own Claude Code.
    Claude {
        #[command(subcommand)]
        command: ClaudeCommand,
    },
    /// Interview yourself when your history is thin: twenty questions, then your answers.
    Interview {
        #[command(subcommand)]
        command: InterviewCommand,
    },
    /// Show what building this assistant has spent today.
    Spend {
        member_dir: PathBuf,
        /// The daily limit, in US dollars as Claude Code estimates them.
        #[arg(long, default_value_t = persona_builder_agent::ledger::Cap::DEFAULT_DAILY_USD)]
        daily_cap_usd: f64,
    },
}

#[derive(Subcommand)]
enum InterviewCommand {
    /// Draft twenty questions from your areas of work, more of them on the kinds of decision your history holds least of.
    Draft {
        member_dir: PathBuf,
        #[arg(long, default_value = "decisions.jsonl")]
        corpus: String,
        /// Labels from an earlier build, to find the kinds of decision your history holds least of.
        #[arg(long)]
        labels: Option<String>,
        #[arg(long, default_value = persona_builder_agent::claude::DEFAULT_MODEL)]
        model: String,
        /// The daily limit, in US dollars as Claude Code estimates them.
        #[arg(long, default_value_t = persona_builder_agent::ledger::Cap::DEFAULT_DAILY_USD)]
        daily_cap_usd: f64,
        /// The most one call may spend, in US dollars as Claude Code estimates them.
        #[arg(long, default_value_t = persona_builder_agent::ledger::Cap::DEFAULT_CALL_USD)]
        call_cap_usd: f64,
        /// Draft new questions in place of the ones waiting for your answers.
        #[arg(long)]
        replace: bool,
    },
    /// Keep your answers to the interview as training for your assistant.
    Answer {
        member_dir: PathBuf,
        #[arg(long)]
        answers: String,
        #[arg(long, default_value = persona_builder_build::interview::QUESTIONS_FILE)]
        questions: String,
    },
}

#[derive(Subcommand)]
enum ClaudeCommand {
    /// Check that Claude Code is installed and signed in.
    Check,
}

#[derive(Subcommand)]
enum ClaudeaiCommand {
    /// Check the export has what Persona Builder reads (shows field names only, never your words).
    Inspect {
        #[arg(long)]
        export: PathBuf,
    },
    /// List your claude.ai chats in a review file, so you can mark which are yours.
    Titles {
        member_dir: PathBuf,
        #[arg(long)]
        export: PathBuf,
        #[arg(long, default_value = "claudeai-review.tsv")]
        review: String,
        #[arg(long)]
        project_field: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Mark chats in the review file as yours (y) or not (n).
    Mark {
        member_dir: PathBuf,
        #[arg(long = "as", value_parser = ["y", "n"])]
        mark: String,
        #[arg(long, default_value = "claudeai-review.tsv")]
        review: String,
        #[arg(long = "uuid", action = clap::ArgAction::Append)]
        uuids: Vec<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        title: Option<String>,
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        unmarked: bool,
        #[arg(long)]
        all: bool,
    },
    /// Read the chats you marked as yours into your assistant's private folder.
    Mine {
        member_dir: PathBuf,
        #[arg(long)]
        export: PathBuf,
        #[arg(long, default_value = "claudeai-review.tsv")]
        review: String,
        #[arg(long, default_value = "decisions-claudeai.jsonl")]
        out: String,
        #[arg(long, conflicts_with = "cutoff", allow_hyphen_values = true)]
        holdout: Option<String>,
        #[arg(long)]
        cutoff: Option<String>,
        #[arg(long)]
        dry_run: bool,
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
fn expand_home(path: PathBuf, home: &std::path::Path) -> PathBuf {
    if path.as_os_str().as_encoded_bytes().starts_with(b"~/") {
        home.join(path.strip_prefix("~").expect("leading ~/"))
    } else {
        path
    }
}
fn projects_path(projects: Option<PathBuf>, home: &std::path::Path) -> Result<PathBuf> {
    let path = expand_home(
        projects.unwrap_or_else(|| home.join(".claude/projects")),
        home,
    );
    if !path.exists() {
        return Err(Error::Invalid(format!(
            "{} doesn't exist: there is no Claude Code history there",
            path.display()
        )));
    }
    Ok(path)
}
fn split_choice(
    holdout: Option<String>,
    cutoff: Option<String>,
) -> Result<persona_builder_ingest::mine::Split> {
    use persona_builder_ingest::mine;
    let split = match cutoff {
        Some(cutoff) => mine::Split::Cutoff(cutoff),
        None => {
            let holdout = holdout.as_deref().unwrap_or("0.15").parse::<f64>();
            let holdout = holdout
                .ok()
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .ok_or_else(|| Error::Invalid("--holdout must be a number from 0 to 1".into()))?;
            mine::Split::Holdout(holdout)
        }
    };
    Ok(split)
}
fn checked_member(
    member_dir: &std::path::Path,
    home: &std::path::Path,
) -> Result<persona_builder::member::Member> {
    let member = read_member(&member_dir.join("member.json"))?;
    persona_builder::member::private_state_dir(&member, home).ok_or_else(|| {
        Error::Invalid(format!(
            "Persona Builder keeps your history only in ~/.claude/state/<slug>. This assistant's member.json says {}: change it to ~/.claude/state/<slug> and try again",
            member.state_dir
        ))
    })?;
    Ok(member)
}
fn mine(
    member_dir: PathBuf,
    projects: Option<PathBuf>,
    out: String,
    holdout: Option<String>,
    cutoff: Option<String>,
    dry_run: bool,
) -> Result<()> {
    use persona_builder_ingest::{filters::Filters, mine, private::write_private};
    persona_builder_ingest::private::validate_name(&out)?;
    let split = split_choice(holdout, cutoff)?;
    let home = home()?;
    let member = checked_member(&member_dir, &home)?;
    let projects = projects_path(projects, &home)?;
    let filters = Filters::compile(&member.filters)?;
    let mined = mine::mine(&member, &filters, &projects, &home, &split)?;
    if dry_run {
        println!("{}dry run: nothing written", mine::summary(&mined));
    } else {
        write_private(
            &home,
            &member.slug,
            &out,
            mine::to_jsonl(&mined.records).as_bytes(),
        )?;
        println!(
            "{}saved to ~/.claude/state/{}/{out}",
            mine::summary(&mined),
            member.slug
        );
    }
    Ok(())
}
fn claudeai_inspect(export: PathBuf) -> Result<()> {
    use persona_builder_ingest::claudeai;
    let export = claudeai::read_export(&expand_home(export, &home()?))?;
    let inspection = claudeai::inspect(&export)?;
    print!("{}", inspection.text);
    if !inspection.missing.is_empty() {
        return Err(Error::Invalid(format!(
            "the export is missing fields Persona Builder reads: {}",
            inspection.missing.join(", ")
        )));
    }
    println!("ok: every field the adapter reads is present");
    Ok(())
}
fn existing_review(
    home: &std::path::Path,
    slug: &str,
    review: &str,
) -> Result<Vec<persona_builder_ingest::claudeai::ReviewRow>> {
    use persona_builder_ingest::{claudeai::read_review, private::read_private};
    let bytes = read_private(home, slug, review).map_err(|error| match error {
        Error::Invalid(message) if message.ends_with("doesn't exist") => Error::Invalid(
            "there's no review file yet: run persona-builder claudeai titles first".into(),
        ),
        other => other,
    })?;
    read_review(review, &bytes)
}
fn claudeai_titles(
    member_dir: PathBuf,
    export: PathBuf,
    review: String,
    project_field: Option<String>,
    dry_run: bool,
) -> Result<()> {
    use persona_builder_ingest::{
        claudeai::{self, review_text},
        filters::Filters,
        private::{validate_review_name, write_private},
    };
    validate_review_name(&review)?;
    let home = home()?;
    let member = checked_member(&member_dir, &home)?;
    let filters = Filters::compile(&member.filters)?;
    let export = claudeai::read_export(&expand_home(export, &home))?;
    let existing = match existing_review(&home, &member.slug, &review) {
        Ok(rows) => rows,
        Err(Error::Invalid(message)) if message.starts_with("there's no review file yet") => {
            vec![]
        }
        Err(other) => return Err(other),
    };
    let titles = claudeai::titles(&filters, &export, &existing, project_field.as_deref())?;
    if !dry_run {
        write_private(
            &home,
            &member.slug,
            &review,
            review_text(&titles.rows).as_bytes(),
        )?;
    }
    print!(
        "{}",
        claudeai::titles_summary(
            &titles,
            &format!("~/.claude/state/{}/{review}", member.slug)
        )
    );
    if dry_run {
        println!("dry run: nothing written");
    }
    Ok(())
}
struct MarkArgs {
    mark: String,
    review: String,
    uuids: Vec<String>,
    project: Option<String>,
    title: Option<String>,
    before: Option<String>,
    after: Option<String>,
    unmarked: bool,
    all: bool,
}
fn claudeai_mark(member_dir: PathBuf, args: MarkArgs) -> Result<()> {
    use persona_builder_ingest::{
        claudeai::{self, Selector, review_text},
        private::{validate_review_name, write_private},
    };
    validate_review_name(&args.review)?;
    let given = |value: Option<String>| value.filter(|v| !v.is_empty());
    let title = given(args.title);
    let mut selector = Selector {
        uuids: args.uuids,
        project: given(args.project),
        title: None,
        before: given(args.before),
        after: given(args.after),
        unmarked: args.unmarked,
        all: args.all,
    };
    if !selector.is_given() && title.is_none() {
        return Err(Error::Invalid(
            "mark needs a selector: --uuid, --project, --title, --before, --after, --unmarked or --all".into(),
        ));
    }
    selector.title = title
        .map(|pattern| claudeai::title_pattern(&pattern))
        .transpose()?;
    let home = home()?;
    let member = checked_member(&member_dir, &home)?;
    let mut rows = existing_review(&home, &member.slug, &args.review)?;
    let changed = claudeai::mark(&mut rows, &selector, &args.mark);
    write_private(
        &home,
        &member.slug,
        &args.review,
        review_text(&rows).as_bytes(),
    )?;
    println!("marked {changed} conversations {}", args.mark);
    Ok(())
}
fn claudeai_mine(
    member_dir: PathBuf,
    export: PathBuf,
    review: String,
    out: String,
    holdout: Option<String>,
    cutoff: Option<String>,
    dry_run: bool,
) -> Result<()> {
    use persona_builder_ingest::{
        claudeai,
        filters::Filters,
        mine::to_jsonl,
        private::{validate_name, validate_review_name, write_private},
    };
    validate_review_name(&review)?;
    validate_name(&out)?;
    let split = split_choice(holdout, cutoff)?;
    let home = home()?;
    let member = checked_member(&member_dir, &home)?;
    let filters = Filters::compile(&member.filters)?;
    let export = claudeai::read_export(&expand_home(export, &home))?;
    let rows = existing_review(&home, &member.slug, &review)?;
    let mined = claudeai::mine(&filters, &export, &rows, &split)?;
    if !dry_run {
        write_private(
            &home,
            &member.slug,
            &out,
            to_jsonl(&mined.records).as_bytes(),
        )?;
    }
    print!("{}", claudeai::mine_summary(&mined));
    if dry_run {
        println!("dry run: nothing written");
    } else {
        println!("saved to ~/.claude/state/{}/{out}", member.slug);
    }
    Ok(())
}
fn merge(
    member_dir: PathBuf,
    mut inputs: Vec<(String, persona_builder_ingest::corpus::Mode)>,
    out: String,
    holdout: Option<String>,
    cutoff: Option<String>,
    dry_run: bool,
) -> Result<()> {
    use persona_builder_ingest::{
        corpus::{self, Input, Mode},
        filters::Filters,
        mine::to_jsonl,
        private::{read_private, validate_name, write_private},
    };
    validate_name(&out)?;
    for (name, _) in &inputs {
        validate_name(name)?;
    }
    let split = split_choice(holdout, cutoff)?;
    let home = home()?;
    let member = checked_member(&member_dir, &home)?;
    let filters = Filters::compile(&member.filters)?;
    if inputs.is_empty() {
        let folder = persona_builder::member::private_state_dir(&member, &home)
            .expect("checked state folder");
        let names = match std::fs::read_dir(&folder) {
            Ok(entries) => entries
                .map(|entry| entry.map(|e| e.file_name().into_string().ok()))
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(|source| Error::Io {
                    path: folder.clone(),
                    source,
                })?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(source) => {
                return Err(Error::Io {
                    path: folder,
                    source,
                });
            }
        };
        inputs = corpus::discover(&names, &out);
        if inputs.is_empty() {
            return Err(Error::Invalid(format!(
                "nothing to merge yet: ~/.claude/state/{} has no decisions-*.jsonl",
                member.slug
            )));
        }
    }
    if inputs.iter().any(|(name, _)| name == &out) {
        return Err(Error::Invalid(
            "the output can't also be one of the inputs".into(),
        ));
    }
    let bytes = inputs
        .iter()
        .map(|(name, _)| read_private(&home, &member.slug, name))
        .collect::<Result<Vec<_>>>()?;
    let borrowed: Vec<_> = inputs
        .iter()
        .zip(&bytes)
        .map(|((name, mode), bytes)| Input {
            name,
            mode: *mode,
            bytes,
        })
        .collect();
    let merged = corpus::merge(&filters, &borrowed, &split)?;
    if !dry_run {
        write_private(
            &home,
            &member.slug,
            &out,
            to_jsonl(&merged.records).as_bytes(),
        )?;
    }
    let names: Vec<_> = inputs
        .iter()
        .map(|(name, mode)| match mode {
            Mode::History => name.clone(),
            Mode::Train => format!("{name} (always training)"),
            Mode::Round => format!("{name} (keeps its split)"),
        })
        .collect();
    println!("merging: {}", names.join(", "));
    print!("{}", corpus::merge_summary(&merged));
    if dry_run {
        println!("dry run: nothing written");
    } else {
        println!("saved to ~/.claude/state/{}/{out}", member.slug);
    }
    Ok(())
}
fn split_corpus(
    member_dir: PathBuf,
    corpus: String,
    questions: String,
    answers: String,
    dry_run: bool,
) -> Result<()> {
    use persona_builder_ingest::{
        corpus as corpus_ops,
        mine::to_jsonl,
        private::{read_private, validate_name, write_private},
    };
    for name in [&corpus, &questions, &answers] {
        validate_name(name)?;
    }
    if corpus == questions || corpus == answers || questions == answers {
        return Err(Error::Invalid(
            "--corpus, --questions and --answers must be three different files".into(),
        ));
    }
    let home = home()?;
    let member = checked_member(&member_dir, &home)?;
    let held = corpus_ops::split(
        &member,
        &corpus,
        &read_private(&home, &member.slug, &corpus)?,
    )?;
    if !dry_run {
        write_private(
            &home,
            &member.slug,
            &questions,
            to_jsonl(&held.questions).as_bytes(),
        )?;
        write_private(
            &home,
            &member.slug,
            &answers,
            to_jsonl(&held.answers).as_bytes(),
        )?;
    }
    print!("{}", corpus_ops::split_summary(&held));
    if dry_run {
        println!("dry run: nothing written");
    } else {
        println!(
            "saved to ~/.claude/state/{}/{questions} and {answers}",
            member.slug
        );
    }
    Ok(())
}
/// A member whose state folder is the private one, with the home it lives under.
fn member_and_home(
    member_dir: &std::path::Path,
) -> Result<(persona_builder::member::Member, PathBuf)> {
    let home = home()?;
    let member = checked_member(member_dir, &home)?;
    Ok((member, home))
}
/// A corpus file, `NAME` or `FOLDER/NAME` inside the state folder.
fn corpus_at(home: &std::path::Path, slug: &str, argument: &str) -> Result<Records> {
    use persona_builder_ingest::{corpus::read_records, private};
    let (folder, name) = private::place(argument)?;
    private::validate_name(name)?;
    read_records(
        argument,
        &private::read_private_at(home, slug, folder, name)?,
    )
}
/// A JSON file, `NAME` or `FOLDER/NAME` inside the state folder.
fn json_at(home: &std::path::Path, slug: &str, argument: &str) -> Result<Vec<u8>> {
    use persona_builder_ingest::private;
    let (folder, name) = private::place(argument)?;
    private::validate_json_name(name)?;
    private::read_private_at(home, slug, folder, name)
}
fn chunk(member_dir: PathBuf, corpus: String, out_dir: String, size: u64) -> Result<()> {
    use persona_builder_ingest::private::{private_folder, write_private_at};
    let (member, home) = member_and_home(&member_dir)?;
    let records = corpus_at(&home, &member.slug, &corpus)?;
    let size = usize::try_from(size).map_err(|_| Error::Invalid("--size is too large".into()))?;
    let chunks = persona_builder_steps::chunk::chunk(&corpus, &records, size)?;
    private_folder(&home, &member.slug, &out_dir)?;
    for (name, text) in chunks.files() {
        write_private_at(&home, &member.slug, Some(&out_dir), &name, text.as_bytes())?;
    }
    print!("{}", chunks.summary());
    println!("saved to ~/.claude/state/{}/{out_dir}/", member.slug);
    Ok(())
}
fn check_labels(
    member_dir: PathBuf,
    chunk: String,
    labels: String,
    mode: String,
) -> Result<std::process::ExitCode> {
    use persona_builder_steps::{book::book, input::read_object, labels as steps};
    let (member, home) = member_and_home(&member_dir)?;
    let book = book(&member)?;
    let records = corpus_at(&home, &member.slug, &chunk)?;
    let given = read_object(&labels, &json_at(&home, &member.slug, &labels)?)?;
    let mode = if mode == "train" {
        steps::Mode::Train
    } else {
        steps::Mode::Heldout
    };
    let problems = steps::label_problems(&book, &chunk, &records, &given, mode)?;
    print!("{}", steps::render_problems(&problems));
    Ok(if problems.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    })
}
fn tally(member_dir: PathBuf, corpus: String, labels: String, json: bool) -> Result<()> {
    use persona_builder_steps::{input::read_objects, labels as steps};
    let (member, home) = member_and_home(&member_dir)?;
    let records = corpus_at(&home, &member.slug, &corpus)?;
    let given = read_objects(&labels, &json_at(&home, &member.slug, &labels)?)?;
    let rows = steps::tally(&corpus, &records, &labels, &given)?;
    print!(
        "{}",
        if json {
            steps::tally_json(&rows)
        } else {
            steps::tally_markdown(&rows)
        }
    );
    Ok(())
}
fn distil_in(member_dir: PathBuf, corpus: String, labels: String, out_dir: String) -> Result<()> {
    use persona_builder_ingest::{
        mine::to_jsonl,
        private::{private_folder, write_private_at},
    };
    use persona_builder_steps::{book::book, distil, input::read_objects};
    let (member, home) = member_and_home(&member_dir)?;
    let book = book(&member)?;
    let records = corpus_at(&home, &member.slug, &corpus)?;
    let given = read_objects(&labels, &json_at(&home, &member.slug, &labels)?)?;
    let groups = distil::distil_in(&book, &corpus, &records, &labels, &given)?;
    private_folder(&home, &member.slug, &out_dir)?;
    for group in &groups {
        write_private_at(
            &home,
            &member.slug,
            Some(&out_dir),
            &format!("{}.jsonl", group.name),
            to_jsonl(&group.rows).as_bytes(),
        )?;
    }
    print!("{}", distil::summary(&groups));
    println!("saved to ~/.claude/state/{}/{out_dir}/", member.slug);
    Ok(())
}
fn score(
    member_dir: PathBuf,
    answers: String,
    labels: String,
    predictions: String,
    json: bool,
) -> Result<()> {
    use persona_builder_steps::{input::read_objects, score as steps};
    let (member, home) = member_and_home(&member_dir)?;
    let records = corpus_at(&home, &member.slug, &answers)?;
    let given = read_objects(&labels, &json_at(&home, &member.slug, &labels)?)?;
    let guesses = read_objects(&predictions, &json_at(&home, &member.slug, &predictions)?)?;
    let result = steps::score(&member, &answers, &records, &labels, &given, &guesses)?;
    print!(
        "{}",
        if json {
            steps::score_json(&member, &result)?
        } else {
            steps::score_markdown(&member, &result)
        }
    );
    Ok(())
}
fn similar(member_dir: PathBuf, corpus: String, query: String, k: usize, json: bool) -> Result<()> {
    use persona_builder_steps::similar as steps;
    let (member, home) = member_and_home(&member_dir)?;
    let records = corpus_at(&home, &member.slug, &corpus)?;
    let hits = steps::similar(&member, &corpus, &records, &query, k)?;
    print!(
        "{}",
        if json {
            steps::similar_json(hits)
        } else {
            steps::similar_text(&member, &hits)
        }
    );
    Ok(())
}
fn shingle(
    member_dir: PathBuf,
    doc: PathBuf,
    corpora: Vec<String>,
    n: u64,
) -> Result<std::process::ExitCode> {
    use persona_builder_steps::shingle;
    let (member, home) = member_and_home(&member_dir)?;
    let doc = expand_home(doc, &home);
    let bytes = std::fs::read(&doc).map_err(|source| Error::Io {
        path: doc.clone(),
        source,
    })?;
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::Invalid(format!("{} is not UTF-8 text", doc.display())))?;
    let read = corpora
        .iter()
        .map(|name| corpus_at(&home, &member.slug, name))
        .collect::<Result<Vec<_>>>()?;
    let pairs: Vec<_> = corpora
        .iter()
        .map(String::as_str)
        .zip(read.iter().map(Vec::as_slice))
        .collect();
    let n = usize::try_from(n).map_err(|_| Error::Invalid("--n is too large".into()))?;
    let hits = shingle::overlaps(&text, &pairs, n)?;
    print!("{}", shingle::render(&hits));
    Ok(if hits.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    })
}
fn round(
    member_dir: PathBuf,
    name: String,
    questions: String,
    answers: String,
    heldout: bool,
    out: Option<String>,
) -> Result<()> {
    use persona_builder_ingest::{
        filters::Filters,
        mine::to_jsonl,
        private::{validate_name, write_private},
    };
    use persona_builder_steps::{input::read_object, round as steps};
    steps::validate_name(&name)?;
    let out = out.unwrap_or_else(|| steps::default_out(&name));
    validate_name(&out)?;
    let (member, home) = member_and_home(&member_dir)?;
    let filters = Filters::compile(&member.filters)?;
    let asked = read_object(&questions, &json_at(&home, &member.slug, &questions)?)?;
    let answered = read_object(&answers, &json_at(&home, &member.slug, &answers)?)?;
    let imported = steps::round(&member, &filters, &name, &asked, &answered, heldout)?;
    write_private(
        &home,
        &member.slug,
        &out,
        to_jsonl(&imported.records).as_bytes(),
    )?;
    print!("{}", steps::summary(&imported));
    println!("saved to ~/.claude/state/{}/{out}", member.slug);
    Ok(())
}
fn folders(member_dir: PathBuf, projects: Option<PathBuf>) -> Result<()> {
    use persona_builder_ingest::{filters::Filters, mine};
    let home = home()?;
    let member = read_member(&member_dir.join("member.json"))?;
    let filters = Filters::compile(&member.filters)?;
    let projects = projects_path(projects, &home)?;
    print!(
        "{}",
        mine::render_folders(&mine::folders(&filters, &projects, &home)?)
    );
    Ok(())
}
fn agents() -> Result<()> {
    for agent in persona_builder_agent::agents::AGENTS {
        println!("{}  {}", agent.id, agent.name);
    }
    Ok(())
}
fn claude_check() -> Result<()> {
    use persona_builder_agent::{claude, text};
    let home = home()?;
    let path = std::env::var_os("PATH");
    let ready = claude::find(path.as_deref(), &home)?.check(&home)?;
    println!("{}", text::ready(&ready.version));
    if ready.api_key_set {
        println!("{}", text::API_KEY_SET);
    }
    Ok(())
}
fn spend(member_dir: PathBuf, daily_cap_usd: f64) -> Result<()> {
    use persona_builder_agent::{
        ledger::{Cap, Session},
        text,
    };
    let home = home()?;
    let member = checked_member(&member_dir, &home)?;
    let cap = Cap::new(daily_cap_usd, Cap::DEFAULT_CALL_USD.min(daily_cap_usd))?;
    let session = Session::open(&home, &member.slug, cap)?;
    println!("{}", text::spend(session.spent_today()?, cap.daily_usd()));
    Ok(())
}
struct DraftArgs {
    corpus: String,
    labels: Option<String>,
    model: String,
    daily_cap_usd: f64,
    call_cap_usd: f64,
    replace: bool,
}
fn interview_draft(member_dir: PathBuf, args: DraftArgs) -> Result<()> {
    use persona_builder_agent::{
        claude,
        ledger::{Cap, Session},
        text as spend_text,
    };
    use persona_builder_build::{interview, text};
    use persona_builder_ingest::{
        corpus::read_records,
        private::{read_private_optional, write_private},
    };
    use persona_builder_steps::{book::book, input::pretty, input::read_objects, labels::tally};
    let (member, home) = member_and_home(&member_dir)?;
    let book = book(&member)?;
    let records = match read_private_optional(&home, &member.slug, &args.corpus)? {
        Some(bytes) => read_records(&args.corpus, &bytes)?,
        None => Vec::new(),
    };
    let answered = interview::answered(&records);
    if answered >= interview::THIN_BELOW {
        println!("{}", text::enough(answered));
        return Ok(());
    }
    if !args.replace
        && read_private_optional(&home, &member.slug, interview::QUESTIONS_FILE)?.is_some()
    {
        return Err(Error::Invalid(text::ALREADY_DRAFTED.into()));
    }
    let rows = match &args.labels {
        Some(labels) => {
            let given = read_objects(labels, &json_at(&home, &member.slug, labels)?)?;
            Some(tally(&args.corpus, &records, labels, &given)?)
        }
        None => None,
    };
    let weights = interview::weights(&book, rows.as_deref())?;
    let cap = Cap::new(args.daily_cap_usd, args.call_cap_usd)?;
    let path = std::env::var_os("PATH");
    let claude = claude::find(path.as_deref(), &home)?;
    claude.check(&home)?;
    let session = Session::open(&home, &member.slug, cap)?;
    let drafted = interview::draft(&claude, &session, &args.model, &book, &weights)?;
    write_private(
        &home,
        &member.slug,
        interview::QUESTIONS_FILE,
        pretty(&serde_json::Value::Object(drafted)).as_bytes(),
    )?;
    println!("{}", text::drafted(interview::COUNT, rows.is_some()));
    println!(
        "saved to ~/.claude/state/{}/{}",
        member.slug,
        interview::QUESTIONS_FILE
    );
    println!("{}", text::swap(&member_dir.display().to_string()));
    println!(
        "{}",
        spend_text::spend(session.spent_today()?, cap.daily_usd())
    );
    Ok(())
}
fn interview_answer(member_dir: PathBuf, answers: String, questions: String) -> Result<()> {
    use persona_builder_build::{interview, text};
    use persona_builder_ingest::{filters::Filters, mine::to_jsonl, private::write_private};
    use persona_builder_steps::input::read_object;
    let (member, home) = member_and_home(&member_dir)?;
    let filters = Filters::compile(&member.filters)?;
    let asked = read_object(&questions, &json_at(&home, &member.slug, &questions)?)?;
    let answered = read_object(&answers, &json_at(&home, &member.slug, &answers)?)?;
    let kept = interview::answer(&member, &filters, &asked, &answered)?;
    write_private(
        &home,
        &member.slug,
        interview::OUT,
        to_jsonl(&kept.records).as_bytes(),
    )?;
    print!("{}", interview::summary(&kept));
    println!(
        "saved to ~/.claude/state/{}/{}",
        member.slug,
        interview::OUT
    );
    println!("{}", text::merge_next(&member_dir.display().to_string()));
    Ok(())
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
struct FolderPrivacy;
impl flow::PrivacyCheck for FolderPrivacy {
    fn hits(&self, folder: &std::path::Path) -> Result<Vec<String>> {
        let files = persona_builder_save::git::folder_files(folder)?;
        let member = read_member(&folder.join("member.json"))?;
        let filters = persona_builder_ingest::filters::Filters::compile(&member.filters)?;
        Ok(persona_builder_ingest::check::scan(folder, &files, &filters)?.hits)
    }
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
    .save_now(&member, &home, &FolderPrivacy, check)?;
    match saved {
        Saved::KeptLocal => println!("{}", text::KEPT_LOCAL),
        Saved::Pushed { message } => println!("{message}"),
    }
    Ok(())
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
fn forget(member_dir: PathBuf, yes: bool) -> Result<()> {
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
    Ok(())
}
fn pack(member_dir: PathBuf, out: PathBuf) -> Result<()> {
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
    Ok(())
}
fn turn_on(member_dir: PathBuf, claude_home: Option<PathBuf>) -> Result<()> {
    let on = install::turn_on(&member_dir, &places(claude_home)?)?;
    println!("{} is on in Claude Code.", on.name);
    println!("Added the skill /{} in {}.", on.slug, on.skill.display());
    println!("Added one line to {}:", on.claude_md.display());
    println!("  {}", on.import_line);
    println!(
        "Start a new Claude Code session to use {0}. To turn {0} off: persona-builder off",
        on.name
    );
    Ok(())
}
fn turn_off(claude_home: Option<PathBuf>) -> Result<()> {
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
    Ok(())
}
fn status(claude_home: Option<PathBuf>) -> Result<()> {
    match install::status(&places(claude_home)?)? {
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
    }
    Ok(())
}

fn run(cli: Cli) -> Result<std::process::ExitCode> {
    let result = match cli.command {
        Some(Command::PrivacyCheck { folder }) => {
            let report = persona_builder_ingest::check::privacy_check(&folder)?;
            print!("{}", persona_builder_ingest::check::render(&report));
            return Ok(if report.hits.is_empty() {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            });
        }
        Some(Command::CheckLabels {
            member_dir,
            chunk,
            labels,
            mode,
        }) => return check_labels(member_dir, chunk, labels, mode),
        Some(Command::Shingle {
            member_dir,
            doc,
            corpora,
            n,
        }) => return shingle(member_dir, doc, corpora, n),
        Some(Command::Chunk {
            member_dir,
            corpus,
            out_dir,
            size,
        }) => chunk(member_dir, corpus, out_dir, size),
        Some(Command::Tally {
            member_dir,
            corpus,
            labels,
            json,
        }) => tally(member_dir, corpus, labels, json),
        Some(Command::DistilIn {
            member_dir,
            corpus,
            labels,
            out_dir,
        }) => distil_in(member_dir, corpus, labels, out_dir),
        Some(Command::Score {
            member_dir,
            answers,
            labels,
            predictions,
            json,
        }) => score(member_dir, answers, labels, predictions, json),
        Some(Command::Similar {
            member_dir,
            corpus,
            query,
            k,
            json,
        }) => similar(member_dir, corpus, query, k, json),
        Some(Command::Round {
            member_dir,
            name,
            questions,
            answers,
            heldout,
            out,
        }) => round(member_dir, name, questions, answers, heldout, out),
        Some(Command::Forget { member_dir, yes }) => forget(member_dir, yes),
        Some(Command::Merge {
            member_dir,
            inputs,
            train,
            rounds,
            out,
            holdout,
            cutoff,
            dry_run,
        }) => {
            use persona_builder_ingest::corpus::Mode;
            let inputs = inputs
                .into_iter()
                .map(|n| (n, Mode::History))
                .chain(train.into_iter().map(|n| (n, Mode::Train)))
                .chain(rounds.into_iter().map(|n| (n, Mode::Round)))
                .collect();
            merge(member_dir, inputs, out, holdout, cutoff, dry_run)
        }
        Some(Command::Split {
            member_dir,
            corpus,
            questions,
            answers,
            dry_run,
        }) => split_corpus(member_dir, corpus, questions, answers, dry_run),
        Some(Command::Mine {
            member_dir,
            projects,
            out,
            holdout,
            cutoff,
            dry_run,
        }) => mine(member_dir, projects, out, holdout, cutoff, dry_run),
        Some(Command::Claudeai { command }) => match command {
            ClaudeaiCommand::Inspect { export } => claudeai_inspect(export),
            ClaudeaiCommand::Titles {
                member_dir,
                export,
                review,
                project_field,
                dry_run,
            } => claudeai_titles(member_dir, export, review, project_field, dry_run),
            ClaudeaiCommand::Mark {
                member_dir,
                mark,
                review,
                uuids,
                project,
                title,
                before,
                after,
                unmarked,
                all,
            } => claudeai_mark(
                member_dir,
                MarkArgs {
                    mark,
                    review,
                    uuids,
                    project,
                    title,
                    before,
                    after,
                    unmarked,
                    all,
                },
            ),
            ClaudeaiCommand::Mine {
                member_dir,
                export,
                review,
                out,
                holdout,
                cutoff,
                dry_run,
            } => claudeai_mine(member_dir, export, review, out, holdout, cutoff, dry_run),
        },
        Some(Command::Folders {
            member_dir,
            projects,
        }) => folders(member_dir, projects),
        Some(Command::Pack { member_dir, out }) => pack(member_dir, out),
        Some(Command::On {
            member_dir,
            claude_home,
        }) => turn_on(member_dir, claude_home),
        Some(Command::Off { claude_home }) => turn_off(claude_home),
        Some(Command::Status { claude_home }) => status(claude_home),
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
        Some(Command::Agents) => agents(),
        Some(Command::Claude {
            command: ClaudeCommand::Check,
        }) => claude_check(),
        Some(Command::Interview { command }) => match command {
            InterviewCommand::Draft {
                member_dir,
                corpus,
                labels,
                model,
                daily_cap_usd,
                call_cap_usd,
                replace,
            } => interview_draft(
                member_dir,
                DraftArgs {
                    corpus,
                    labels,
                    model,
                    daily_cap_usd,
                    call_cap_usd,
                    replace,
                },
            ),
            InterviewCommand::Answer {
                member_dir,
                answers,
                questions,
            } => interview_answer(member_dir, answers, questions),
        },
        Some(Command::Spend {
            member_dir,
            daily_cap_usd,
        }) => spend(member_dir, daily_cap_usd),
        None => Ok(()),
    };
    result.map(|()| std::process::ExitCode::SUCCESS)
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("persona-builder: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
