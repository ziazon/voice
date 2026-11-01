//! The tool-less interface for model providers.
use crate::{guard::Sealed, ledger::Session};
use serde_json::Value;
use voice_core::ceilings::Source;
use voice_core::error::{Error, Result};

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
    pub sources: &'a [Source],
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
    /// The model this provider runs for a brief that asks for `asked`. Claude Code runs the one
    /// asked for; a model server runs the one it was set up with.
    fn model<'b>(&'b self, asked: &'b str) -> &'b str {
        asked
    }
    /// One call. The provider re-checks the seal before it sends anything.
    fn call(&self, session: &Session, sealed: &Sealed) -> Result<Reply>;
}

/// The largest prompt, under the CLI's 10 MB stdin cap.
pub const MAX_PROMPT: usize = 8 * 1024 * 1024;
pub const MAX_SYSTEM: usize = 32 * 1024;

/// A step's name for the ledger: a lower-case letter, then up to 39 letters, digits or dashes.
fn valid_step(step: &str) -> bool {
    let mut chars = step.chars();
    step.len() <= 40
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
/// The checks every provider makes on a brief before it spends anything.
pub(crate) fn check_brief(brief: &Brief) -> Result<()> {
    let invalid = |message: &str| Error::Invalid(message.into());
    if !valid_step(brief.step) {
        return Err(invalid(
            "a step's name must be a lower-case letter, then up to 39 letters, digits or dashes",
        ));
    }
    if brief.prompt.trim().is_empty() || brief.prompt.len() > MAX_PROMPT {
        return Err(invalid("a step's records must be there and at most 8 MiB"));
    }
    if brief.system.trim().is_empty() || brief.system.len() > MAX_SYSTEM {
        return Err(invalid(
            "a step's instructions must be there and at most 32 KiB",
        ));
    }
    if !brief.schema.is_object() {
        return Err(invalid("a step's answer shape must be a JSON object"));
    }
    Ok(())
}
