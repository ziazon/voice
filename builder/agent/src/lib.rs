//! The agents Persona Builder drives for its model calls: today one, the person's own Claude Code
//! (PB-D6, PB-D9). Every call is tool-less, spends against a daily cap and leaves one ledger line.

pub mod agents;
mod child;
pub mod claude;
pub mod ledger;
pub mod text;
