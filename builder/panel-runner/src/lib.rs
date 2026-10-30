//! Runs a panel of the person's own assistants on this computer.
//! Every post is untrusted.
//! No tools are available in any turn.

pub mod brief;
pub mod guard;
pub mod room;
pub mod run;
pub mod seats;
pub mod text;
pub mod turns;

pub use run::*;
pub use turns::{ClaudeTurns, Turn, TurnUsage, Turns};
