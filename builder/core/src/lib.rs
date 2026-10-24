//! The Persona Builder core library: everything that builds a person's AI assistant from their own
//! Claude history. It does no I/O beyond the local filesystem; the CLI and the app are thin layers
//! over it.

/// The version every Persona Builder surface reports, from this crate's `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
