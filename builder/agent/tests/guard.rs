mod common;
use common::*;
use persona_builder::{
    ceilings::{Ceiling, Ceilings, Source},
    error::Result,
    member::Member,
};
use persona_builder_agent::{
    guard::{self, Guard, Sealed},
    ledger::Session,
    provider::{Brief, Provider, Reach, Reply, Usage},
};
use serde_json::json;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, PartialEq)]
struct Recorded {
    step: String,
    system: String,
    prompt: String,
    model: String,
    schema: serde_json::Value,
    sources: Vec<Source>,
}
struct Fake {
    id: &'static str,
    reach: Reach,
    check_id: &'static str,
    check_reach: Reach,
    calls: AtomicUsize,
    briefs: Mutex<Vec<Recorded>>,
}
impl Fake {
    fn new(id: &'static str, reach: Reach) -> Self {
        Self {
            id,
            reach,
            check_id: id,
            check_reach: reach,
            calls: AtomicUsize::new(0),
            briefs: Mutex::new(Vec::new()),
        }
    }
    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl Provider for Fake {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        "Fake"
    }
    fn reach(&self) -> Reach {
        self.reach
    }
    fn call(&self, _session: &Session, sealed: &Sealed) -> Result<Reply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let brief = sealed.brief();
        self.briefs.lock().unwrap().push(Recorded {
            step: brief.step.into(),
            system: brief.system.into(),
            prompt: brief.prompt.into(),
            model: brief.model.into(),
            schema: brief.schema.clone(),
            sources: brief.sources.to_vec(),
        });
        sealed.check(self.check_id, self.check_reach)?;
        Ok(Reply {
            output: json!({"labels": []}),
            usage: Usage::default(),
            cost_usd: 0.0,
        })
    }
}
fn settings(source: &str, ceiling: &str) -> Ceilings {
    let mut member: Member =
        serde_json::from_str(include_str!("../../../crew/_template/member.json")).unwrap();
    member
        .extra
        .insert("ceilings".into(), json!({source: ceiling}));
    Ceilings::for_member(&member).unwrap()
}
fn refused(guard: &Guard, sources: &[Source]) -> String {
    let home = Scratch::new("guard-refusal");
    let session = session(&home.0);
    let schema = schema();
    let error = guard
        .call(
            &session,
            &Brief {
                sources,
                ..call("records", &schema)
            },
        )
        .unwrap_err();
    assert!(ledger_lines(&home.0).is_empty());
    error.to_string()
}

#[test]
fn allows_checks_all_nine_ceiling_reach_pairs() {
    use Ceiling::{AnyConsented, LocalOnly};
    use Reach::{OwnMachines, ThirdParty};
    for (ceiling, reach, expected) in [
        (LocalOnly, OwnMachines, true),
        (LocalOnly, Reach::Anthropic, false),
        (LocalOnly, ThirdParty, false),
        (Ceiling::Anthropic, OwnMachines, true),
        (Ceiling::Anthropic, Reach::Anthropic, true),
        (Ceiling::Anthropic, ThirdParty, false),
        (AnyConsented, OwnMachines, true),
        (AnyConsented, Reach::Anthropic, true),
        (AnyConsented, ThirdParty, false),
    ] {
        assert_eq!(
            guard::allows(ceiling, reach),
            expected,
            "{ceiling:?}, {reach:?}"
        );
    }
}
#[test]
fn needed_names_the_narrowest_ceiling_for_each_reach() {
    assert_eq!(guard::needed(Reach::OwnMachines), Ceiling::LocalOnly);
    assert_eq!(guard::needed(Reach::Anthropic), Ceiling::Anthropic);
    assert_eq!(guard::needed(Reach::ThirdParty), Ceiling::AnyConsented);
}
fn routes(sources: &[Source], expected: &str) {
    let third = Fake::new("third", Reach::ThirdParty);
    let anthropic = Fake::new("anthropic", Reach::Anthropic);
    let own = Fake::new("own", Reach::OwnMachines);
    let guard = Guard::new(vec![&third, &anthropic, &own], Ceilings::defaults());
    assert_eq!(guard.route(sources).unwrap().id(), expected);
    let home = Scratch::new("guard-route");
    let schema = schema();
    guard
        .call(
            &session(&home.0),
            &Brief {
                sources,
                ..call("records", &schema)
            },
        )
        .unwrap();
    assert_eq!(third.count(), 0);
    assert_eq!(anthropic.count(), usize::from(expected == "anthropic"));
    assert_eq!(own.count(), usize::from(expected == "own"));
    let selected = if expected == "anthropic" {
        &anthropic
    } else {
        &own
    };
    assert_eq!(
        *selected.briefs.lock().unwrap(),
        vec![Recorded {
            step: "label".into(),
            system: "Label each record.".into(),
            prompt: "records".into(),
            model: "sonnet".into(),
            schema,
            sources: sources.to_vec()
        }]
    );
}
#[test]
fn history_routes_to_the_first_allowed_anthropic_provider() {
    routes(&[Source::ClaudeHistory], "anthropic");
}
#[test]
fn mail_routes_to_own_machines() {
    routes(&[Source::Mail], "own");
}
#[test]
fn mixed_history_and_mail_route_to_own_machines() {
    routes(&[Source::ClaudeHistory, Source::Mail], "own");
}
#[test]
fn mail_refusal_names_the_blocked_source_without_a_call_or_ledger_line() {
    let fake = Fake::new("fake", Reach::Anthropic);
    let guard = Guard::new(vec![&fake], Ceilings::defaults());
    assert_eq!(
        refused(&guard, &[Source::CrewFolder, Source::Mail]),
        "Nothing was sent. This step uses your mail, which you keep on your own machines, and Fake sends what it's given to Anthropic. To let it go there, run: persona-builder ceilings set <assistant folder> mail anthropic"
    );
    assert_eq!(fake.count(), 0);
    assert!(fake.briefs.lock().unwrap().is_empty());
}
#[test]
fn the_persons_local_history_setting_refuses_anthropic() {
    let fake = Fake::new("fake", Reach::Anthropic);
    let guard = Guard::new(vec![&fake], settings("claude-history", "local-only"));
    assert_eq!(
        refused(&guard, &[Source::ClaudeHistory]),
        "Nothing was sent. This step uses your Claude history (your Claude Code sessions and your claude.ai export), which you keep on your own machines, and Fake sends what it's given to Anthropic. To let it go there, run: persona-builder ceilings set <assistant folder> claude-history anthropic"
    );
    assert_eq!(fake.count(), 0);
}
#[test]
fn empty_sources_are_refused_before_the_provider_runs() {
    let fake = Fake::new("fake", Reach::Anthropic);
    let guard = Guard::new(vec![&fake], Ceilings::defaults());
    assert_eq!(
        refused(&guard, &[]),
        "a step must name the sources of what it sends"
    );
    assert_eq!(fake.count(), 0);
}
#[test]
fn any_consented_still_refuses_a_third_party() {
    let fake = Fake::new("fake", Reach::ThirdParty);
    let guard = Guard::new(vec![&fake], settings("mail", "any-consented"));
    assert_eq!(
        refused(&guard, &[Source::Mail]),
        "Nothing was sent. This step uses your mail, and Fake sends what it's given to another company, which you haven't agreed to."
    );
    assert_eq!(fake.count(), 0);
}
#[test]
fn the_seal_rechecks_a_provider_that_changes_its_reach() {
    let mut fake = Fake::new("fake", Reach::Anthropic);
    fake.check_reach = Reach::ThirdParty;
    let guard = Guard::new(vec![&fake], Ceilings::defaults());
    assert!(refused(&guard, &[Source::ClaudeHistory]).starts_with("Nothing was sent."));
    assert_eq!(fake.count(), 1);
}
#[test]
fn the_seal_refuses_a_different_provider_id() {
    let mut fake = Fake::new("fake", Reach::Anthropic);
    fake.check_id = "another-provider";
    let guard = Guard::new(vec![&fake], Ceilings::defaults());
    assert_eq!(
        refused(&guard, &[Source::ClaudeHistory]),
        "this brief wasn't sealed for this provider, so nothing was sent"
    );
    assert_eq!(fake.count(), 1);
}
