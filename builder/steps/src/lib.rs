//! The build's steps that need no model: a port of `scripts/crew_decisions.py`'s `chunk`,
//! `check-labels`, `tally`, `distil-in`, `score`, `similar`, `shingle` and `round`, at parity with
//! the Python on every input it accepts and finishes on.
pub mod book;
pub mod chunk;
pub mod distil;
pub mod input;
pub mod labels;
pub mod round;
pub mod score;
pub mod share;
pub mod shingle;
pub mod similar;
pub mod text_check;
