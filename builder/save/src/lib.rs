//! Saving an assistant to a private GitHub repo, an existing repo, or keeping it on this computer.
//! Network, git and password-store code lives here so the core library keeps to the filesystem.

pub mod github;
pub mod keychain;
pub mod secret;
pub mod text;

pub mod flow;
pub mod git;
pub mod record;
