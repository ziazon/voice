//! The Persona Builder core library: everything that builds a person's AI assistant from their own
//! Claude history. It does no I/O beyond the local filesystem; the CLI and the app are thin layers
//! over it.

/// The version every Persona Builder surface reports, from this crate's `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod assistants;
pub mod autonomy;
pub mod block;
pub mod ceilings;
pub mod cli_command;
pub mod consent;
pub mod crew;
pub mod deny;
pub mod error;
pub mod fields;
pub mod forget;
pub mod install;
mod install_fs;
mod install_record;
pub mod json;
pub mod member;
pub mod pack;
pub mod template;

#[cfg(test)]
mod test_support;
