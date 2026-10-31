#![allow(dead_code)]
use persona_builder::member::Member;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub struct Fixture {
    pub home: PathBuf,
    pub member: Member,
}
impl Fixture {
    pub fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-grow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let template =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template/member.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(template).unwrap()).unwrap();
        value["slug"] = "robin".into();
        Self {
            home,
            member: serde_json::from_value(value).unwrap(),
        }
    }
    pub fn growth(&self) -> PathBuf {
        self.home.join(".claude/state/robin/growth")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
pub const NOW: &str = "2026-03-04T15:20:00Z";
pub fn input() -> serde_json::Value {
    serde_json::json!({"kind":"correction","area":"communication","situation":"A parent asks to move a student's quiz because of a family trip.","persona_said":"Decline politely and point to the late-work policy.","wanted":"Offer one make-up slot in the same week, then point to the policy for anything later.","source":"review-report"})
}

/// Test provider that records the sources actually delivered by the guard, then uses the stub.
pub struct RecordingProvider<'a> {
    pub inner: &'a dyn persona_builder_agent::provider::Provider,
    pub sources: std::sync::Mutex<Vec<Vec<persona_builder::ceilings::Source>>>,
}
impl<'a> RecordingProvider<'a> {
    pub fn new(inner: &'a dyn persona_builder_agent::provider::Provider) -> Self {
        Self {
            inner,
            sources: std::sync::Mutex::new(Vec::new()),
        }
    }
}
impl persona_builder_agent::provider::Provider for RecordingProvider<'_> {
    fn id(&self) -> &'static str {
        self.inner.id()
    }
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn reach(&self) -> persona_builder_agent::provider::Reach {
        self.inner.reach()
    }
    fn call(
        &self,
        session: &persona_builder_agent::ledger::Session,
        sealed: &persona_builder_agent::guard::Sealed,
    ) -> persona_builder::error::Result<persona_builder_agent::provider::Reply> {
        sealed.check(self.id(), self.reach())?;
        self.sources
            .lock()
            .unwrap()
            .push(sealed.brief().sources.to_vec());
        self.inner.call(session, sealed)
    }
}
