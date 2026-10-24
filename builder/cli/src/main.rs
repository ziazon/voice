use clap::Parser;

/// The `persona-builder` command: a thin layer over the core library.
#[derive(Parser)]
#[command(
    name = "persona-builder",
    version = persona_builder::VERSION,
    about = "Build your AI assistant from your own Claude history"
)]
struct Cli {}

fn main() {
    Cli::parse();
}
