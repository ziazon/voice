use persona_builder::{error::Result, member::Member};
use persona_builder_agent::{
    ledger::{Cap, Session},
    provider::{Brief, Provider, Reach, Reply, Usage},
};
use persona_builder_build::{
    build::{Asker, Misfit},
    interview::{self, Form},
};
use persona_builder_steps::book::book;
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
    fn call(&self, _session: &Session, brief: &Brief) -> Result<Reply> {
        self.briefs.lock().unwrap().push(Recorded {
            step: brief.step.to_owned(),
            system: brief.system.to_owned(),
            prompt: brief.prompt.to_owned(),
            model: brief.model.to_owned(),
            schema: brief.schema.clone(),
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
        let draft = interview::draft(form, &fake, &session, "test-model", &book, &weights).unwrap();
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
    let draft =
        interview::draft(Form::Full, &fake, &session, "test-model", &book, &weights).unwrap();
    assert_eq!(Value::Object(draft), good);
    let briefs = fake.briefs.lock().unwrap();
    assert_eq!(briefs.len(), 2);
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
    let mut asker = Asker::new(&fake, &session, &home.0, "ada", "test-model");
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
        }]
    );
    let mut resumed = Asker::new(&fake, &session, &home.0, "ada", "test-model");
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
