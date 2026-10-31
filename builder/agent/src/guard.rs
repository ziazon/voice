//! Privacy routing and provider-bound briefs.
use crate::{
    ledger::Session,
    provider::{Brief, Provider, Reach, Reply},
    text,
};
use persona_builder::{
    ceilings::{Ceiling, Ceilings, Source},
    error::{Error, Result},
};

/// Whether data under this ceiling may go where a provider of this reach sends it.
/// Third parties are never allowed yet: no third-party provider or consent record exists.
/// A later slice adds the person's per-provider consent here.
pub fn allows(ceiling: Ceiling, reach: Reach) -> bool {
    match reach {
        Reach::OwnMachines => true,
        Reach::Anthropic => ceiling >= Ceiling::Anthropic,
        Reach::ThirdParty => false,
    }
}
/// The ceiling needed for a reach, subject to the person's per-provider consent.
pub fn needed(reach: Reach) -> Ceiling {
    match reach {
        Reach::OwnMachines => Ceiling::LocalOnly,
        Reach::Anthropic => Ceiling::Anthropic,
        Reach::ThirdParty => Ceiling::AnyConsented,
    }
}

/// A brief the guard checked for one provider. Only this module makes one.
pub struct Sealed<'a> {
    brief: &'a Brief<'a>,
    provider: &'static str,
    provider_name: &'static str,
    ceilings: Vec<(Source, Ceiling)>,
}
impl<'a> Sealed<'a> {
    pub fn brief(&self) -> &Brief<'a> {
        self.brief
    }
    /// The provider's own re-check before it sends anything: the seal must be for this provider
    /// and every ceiling in it must allow this reach.
    pub fn check(&self, provider: &str, reach: Reach) -> Result<()> {
        if provider != self.provider {
            return Err(Error::Invalid(
                "this brief wasn't sealed for this provider, so nothing was sent".into(),
            ));
        }
        if let Some(&(source, ceiling)) = self
            .ceilings
            .iter()
            .find(|(_, ceiling)| !allows(*ceiling, reach))
        {
            return Err(Error::Invalid(text::kept_home(
                source,
                ceiling,
                self.provider_name,
                reach,
            )));
        }
        Ok(())
    }
}

/// The privacy guard: every model call goes through it.
pub struct Guard<'a> {
    providers: Vec<&'a dyn Provider>,
    ceilings: Ceilings,
}
impl<'a> Guard<'a> {
    pub fn new(providers: Vec<&'a dyn Provider>, ceilings: Ceilings) -> Self {
        Self {
            providers,
            ceilings,
        }
    }
    /// The first provider, in order, whose reach every source's ceiling allows.
    pub fn route(&self, sources: &[Source]) -> Result<&'a dyn Provider> {
        if sources.is_empty() {
            return Err(Error::Invalid(
                "a step must name the sources of what it sends".into(),
            ));
        }
        let first = self
            .providers
            .first()
            .ok_or_else(|| Error::Invalid("no model provider is set up".into()))?;
        for provider in &self.providers {
            if sources
                .iter()
                .all(|source| allows(self.ceilings.get(*source), provider.reach()))
            {
                return Ok(*provider);
            }
        }
        let source = *sources
            .iter()
            .find(|source| !allows(self.ceilings.get(**source), first.reach()))
            .expect("first provider refused a source");
        Err(Error::Invalid(text::kept_home(
            source,
            self.ceilings.get(source),
            first.name(),
            first.reach(),
        )))
    }
    /// Route, seal, and hand the sealed brief to that provider.
    pub fn call(&self, session: &Session, brief: &Brief) -> Result<Reply> {
        let provider = self.route(brief.sources)?;
        let sealed = Sealed {
            brief,
            provider: provider.id(),
            provider_name: provider.name(),
            ceilings: brief
                .sources
                .iter()
                .map(|source| (*source, self.ceilings.get(*source)))
                .collect(),
        };
        provider.call(session, &sealed)
    }
}
