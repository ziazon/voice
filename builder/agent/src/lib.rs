//! The providers Voice drives for its model calls, behind one tool-less trait: today
//! Claude Code and a model server on the person's own machines. Later providers join this list.
//! Every call spends against a daily cap and leaves one ledger line.

pub mod agents;
mod child;
pub mod claude;
pub mod guard;
pub mod ledger;
pub mod provider;
pub mod self_hosted;
pub mod text;
