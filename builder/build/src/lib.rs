//! The build's steps that call a model, through the person's own Claude Code (PB-D6, PB-D9): the
//! interview when the history is thin, and the build itself (label, distil, the blind backtest,
//! then the member's `decisions.md` and `measurements.md`).

pub mod build;
pub mod draft;
pub mod interview;
pub mod report;
pub mod text;

pub mod run;
