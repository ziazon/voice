//! The tool-less interface for model providers.
use crate::ledger::Session;
use persona_builder::error::Result;
use serde_json::Value;

/// Where a provider sends what it is given. Ordered from the narrowest to the widest, so VO8.2
/// can compare a provider's reach with a source's ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    /// Never leaves the person's own machines (this Mac, or one on their own network).
    OwnMachines,
    /// Goes to Anthropic.
    Anthropic,
    /// Goes to a third party.
    ThirdParty,
}

/// One brief: what it is for (`step`, kept in the ledger), the instructions, the person's records,
/// the model and the shape the reply must have. It has no tools field: no provider can be handed
/// a tool.
pub struct Brief<'a> {
    pub step: &'a str,
    pub system: &'a str,
    pub prompt: &'a str,
    pub model: &'a str,
    pub schema: &'a Value,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

/// A call's answer. `output` is untrusted: the caller checks it.
#[derive(Debug)]
pub struct Reply {
    pub output: Value,
    pub usage: Usage,
    pub cost_usd: f64,
}

/// A model provider. A call is tool-less, spends against the session's daily cap and leaves one
/// ledger line; the reply's `output` is untrusted and the caller checks it.
pub trait Provider: Send + Sync {
    /// Its id, for the CLI, the app and the ledger.
    fn id(&self) -> &'static str;
    /// Its name, for people.
    fn name(&self) -> &'static str;
    /// Where it sends a brief.
    fn reach(&self) -> Reach;
    /// One call.
    fn call(&self, session: &Session, brief: &Brief) -> Result<Reply>;
}
