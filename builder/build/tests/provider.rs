use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, UNIX_EPOCH},
};
use voice_agent::{
    guard::{Guard, Sealed},
    ledger::{Cap, Session},
    provider::{Provider, Reach, Reply, Usage},
};
use voice_build::{
    build::{Asker, Misfit},
    interview::{self, Form},
};
use voice_core::{
    ceilings::{Ceilings, Source},
    error::Result,
    member::Member,
};
use voice_steps::book::book;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-provider-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, PartialEq)]
struct Recorded {
    step: String,
    system: String,
    prompt: String,
    model: String,
    schema: Value,
    sources: Vec<Source>,
}

struct Fake {
    briefs: Mutex<Vec<Recorded>>,
    replies: Mutex<VecDeque<Reply>>,
}
impl Fake {
    fn new(outputs: Vec<Value>) -> Self {
        Self {
            briefs: Mutex::new(Vec::new()),
            replies: Mutex::new(
                outputs
                    .into_iter()
                    .map(|output| Reply {
                        output,
                        usage: Usage::default(),
                        cost_usd: 0.0,
                    })
                    .collect(),
            ),
        }
    }
}
impl Provider for Fake {
    fn id(&self) -> &'static str {
        "fake"
    }
    fn name(&self) -> &'static str {
        "Fake provider"
    }
    fn reach(&self) -> Reach {
        Reach::OwnMachines
    }
    fn call(&self, _session: &Session, sealed: &Sealed) -> Result<Reply> {
        sealed.check(self.id(), self.reach())?;
        let brief = sealed.brief();
        self.briefs.lock().unwrap().push(Recorded {
            step: brief.step.to_owned(),
            system: brief.system.to_owned(),
            prompt: brief.prompt.to_owned(),
            model: brief.model.to_owned(),
            schema: brief.schema.clone(),
            sources: brief.sources.to_vec(),
        });
        Ok(self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("canned reply"))
    }
}

fn session(home: &Path) -> Session {
    Session::open_with_clock(
        home,
        "ada",
        Cap::default(),
        Box::new(|| UNIX_EPOCH + Duration::from_secs(1_793_000_000)),
    )
    .unwrap()
}

fn member() -> Member {
    Member::from_value(
        json!({"name": "Ada", "slug": "ada", "principal": "Sam", "principal_slug": "sam",
               "principal_pronouns": "they/them", "state_dir": "~/.claude/state/ada", "mark_word": "Ada",
               "filters": {"excluded_folders": "^-private-", "personal": "private",
                           "secret": "secret", "pasted_prefixes": ["You are "]},
               "codebook": {"categories": ["scope", "other"], "domains": ["garden", "other"],
                            "escalates": ["submit"], "signals": ["ruling"],
                            "groups": {"g1": ["scope", "other"]}}}),
        "member.json",
    )
    .unwrap()
}

fn questions(count: usize) -> Value {
    json!({"questions": (1..=count).map(|n| json!({
        "n": n, "header": "scope", "question": format!("Situation {n}: what first?"),
        "multiSelect": false, "recommended": null,
        "options": [{"label": "Cut it", "description": "Less work"},
                    {"label": "Keep it", "description": "More work"}]
    })).collect::<Vec<_>>()})
}

#[test]
fn interview_drafts_through_the_provider_with_the_forms_brief() {
    for form in [Form::Full, Form::Short] {
        let home = Scratch::new();
        let session = session(&home.0);
        let book = book(&member()).unwrap();
        let weights = vec![("scope".to_owned(), Some(0))];
        let count = form.count(&weights);
        let mut output = questions(count);
        // The caller must check and rebuild the output, dropping unrecognised keys.
        output["extra"] = json!("discard me");
        let fake = Fake::new(vec![output]);
        let guard = Guard::new(vec![&fake], Ceilings::defaults());
        let draft =
            interview::draft(form, &guard, &session, "test-model", &book, &weights).unwrap();
        assert_eq!(Value::Object(draft), questions(count));
        let briefs = fake.briefs.lock().unwrap();
        assert_eq!(
            *briefs,
            vec![Recorded {
                step: form.step().to_owned(),
                system: interview::SYSTEM.to_owned(),
                prompt: interview::prompt(form, &book, &weights),
                model: "test-model".to_owned(),
                schema: interview::schema(count),
                sources: vec![Source::CrewFolder],
            }]
        );
    }
}

#[test]
fn interview_retries_a_misfit_once_and_names_the_problem() {
    let home = Scratch::new();
    let session = session(&home.0);
    let book = book(&member()).unwrap();
    let weights = vec![("scope".to_owned(), None)];
    let good = questions(20);
    let mut bad = good.clone();
    bad["questions"][1]["header"] = json!("zebracorn");
    let fake = Fake::new(vec![bad, good.clone()]);
    let guard = Guard::new(vec![&fake], Ceilings::defaults());
    let draft =
        interview::draft(Form::Full, &guard, &session, "test-model", &book, &weights).unwrap();
    assert_eq!(Value::Object(draft), good);
    let briefs = fake.briefs.lock().unwrap();
    assert_eq!(briefs.len(), 2);
    assert!(
        briefs
            .iter()
            .all(|brief| brief.sources == [Source::CrewFolder])
    );
    assert_eq!(
        briefs[0].prompt,
        interview::prompt(Form::Full, &book, &weights)
    );
    assert_eq!(
        briefs[1].prompt,
        format!(
            "{}\nYour last draft didn't fit the form: question 2: the header must be one of the kinds of decision. Draft all 20 questions again, following every rule above.\n",
            briefs[0].prompt
        )
    );
    assert_eq!(briefs[1].step, briefs[0].step);
    assert_eq!(briefs[1].system, interview::SYSTEM);
    assert_eq!(briefs[1].model, "test-model");
    assert_eq!(briefs[1].schema, interview::schema(20));
    assert!(!briefs[1].prompt.contains("zebracorn"));
}

fn checked_label(output: &Value) -> std::result::Result<String, Misfit> {
    output["label"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Misfit::plain("there is no label"))
}

#[test]
fn a_build_step_sends_its_brief_and_a_new_asker_reuses_the_checked_cache() {
    let home = Scratch::new();
    let session = session(&home.0);
    let fake = Fake::new(vec![json!({"label": "scope"})]);
    let schema = json!({"type": "object", "required": ["label"],
                        "properties": {"label": {"type": "string"}}});
    let guard = Guard::new(vec![&fake], Ceilings::defaults());
    assert_eq!(
        voice_build::build::SOURCES,
        &[Source::ClaudeHistory, Source::Typed, Source::CrewFolder]
    );
    let mut asker = Asker::new(
        &guard,
        voice_build::build::SOURCES,
        &session,
        &home.0,
        "ada",
        "test-model",
    );
    let (output, reused) = asker
        .ask(
            "label",
            "Label the record.",
            &schema,
            "record one\n",
            "Labelling",
            checked_label,
        )
        .unwrap();
    assert_eq!(output, "scope");
    assert!(!reused);
    assert_eq!(
        *fake.briefs.lock().unwrap(),
        vec![Recorded {
            step: "label".to_owned(),
            system: "Label the record.".to_owned(),
            prompt: "record one\n".to_owned(),
            model: "test-model".to_owned(),
            schema: schema.clone(),
            sources: voice_build::build::SOURCES.to_vec(),
        }]
    );
    let mut resumed = Asker::new(
        &guard,
        voice_build::build::SOURCES,
        &session,
        &home.0,
        "ada",
        "test-model",
    );
    let (output, reused) = resumed
        .ask(
            "label",
            "Label the record.",
            &schema,
            "record one\n",
            "Labelling",
            checked_label,
        )
        .unwrap();
    assert_eq!(output, "scope");
    assert!(reused);
    assert_eq!(fake.briefs.lock().unwrap().len(), 1);
}

#[test]
fn claude_cache_key_is_pinned() {
    let name =
        voice_build::build::cache_name("label", "sonnet", "S", &json!({"type":"object"}), "P");
    assert_eq!(name, "label-a81454259719d691af2c1d509da0bd64977c5e8e.json");
}

struct Named {
    id: &'static str,
    model: &'static str,
    fake: Fake,
}
impl Provider for Named {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        "Test provider"
    }
    fn reach(&self) -> Reach {
        Reach::OwnMachines
    }
    fn model<'b>(&'b self, _asked: &'b str) -> &'b str {
        self.model
    }
    fn call(&self, _session: &Session, sealed: &Sealed) -> Result<Reply> {
        sealed.check(self.id(), self.reach())?;
        Ok(self
            .fake
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("canned reply"))
    }
}

#[test]
fn a_self_hosted_step_does_not_reuse_a_claude_reply() {
    let home = Scratch::new();
    let session = session(&home.0);
    let schema = json!({"type":"object"});
    let claude = Named {
        id: "claude-code",
        model: "sonnet",
        fake: Fake::new(vec![json!({"label":"claude"})]),
    };
    let server = Named {
        id: "self-hosted",
        model: "sonnet",
        fake: Fake::new(vec![json!({"label":"server"})]),
    };
    let ask = |provider: &dyn Provider| {
        let guard = Guard::new(vec![provider], Ceilings::defaults());
        let mut asker = Asker::new(
            &guard,
            &[Source::CrewFolder],
            &session,
            &home.0,
            "ada",
            "sonnet",
        );
        asker
            .ask("label", "S", &schema, "P", "Labelling", checked_label)
            .unwrap()
    };
    assert_eq!(ask(&claude), ("claude".into(), false));
    assert_eq!(ask(&server), ("server".into(), false));
    assert_eq!(ask(&server), ("server".into(), true));
    let folder = home.0.join(".claude/state/ada/build");
    assert!(
        folder
            .join("label-a81454259719d691af2c1d509da0bd64977c5e8e.json")
            .exists()
    );
    assert!(
        folder
            .join(voice_build::build::cache_name(
                "label",
                "self-hosted/sonnet",
                "S",
                &schema,
                "P"
            ))
            .exists()
    );
}

#[test]
fn asker_records_the_answering_provider_and_model_once_even_when_reused() {
    let home = Scratch::new();
    let session = session(&home.0);
    let server = Named {
        id: "self-hosted",
        model: "qwen3:8b",
        fake: Fake::new(vec![json!({"label":"server"})]),
    };
    let guard = Guard::new(vec![&server], Ceilings::defaults());
    let schema = json!({"type":"object"});
    let mut asker = Asker::new(&guard, &[Source::Mail], &session, &home.0, "ada", "sonnet");
    for _ in 0..2 {
        asker
            .ask("label", "S", &schema, "P", "Labelling", checked_label)
            .unwrap();
    }
    assert_eq!(asker.by(), &[("self-hosted".into(), "qwen3:8b".into())]);
    let mut resumed = Asker::new(&guard, &[Source::Mail], &session, &home.0, "ada", "sonnet");
    assert!(
        resumed
            .ask("label", "S", &schema, "P", "Labelling", checked_label)
            .unwrap()
            .1
    );
    assert_eq!(resumed.by(), asker.by());
}
