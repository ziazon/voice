// Test-only harness: do not export or implement the production module in the red dispatch.
pub use persona_builder::{ceilings, member};
#[path = "../src/ceilings.rs"]
mod ceilings_contract;
