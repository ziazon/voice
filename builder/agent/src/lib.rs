//! The providers Persona Builder drives for its model calls, behind one tool-less trait: today
//! one, the person's own Claude Code (PB-D6, PB-D9). Every call spends against a daily cap and
//! leaves one ledger line.

pub mod agents;
mod child;
pub mod claude;
pub mod guard;
pub mod ledger;
pub mod provider;
pub mod text;
