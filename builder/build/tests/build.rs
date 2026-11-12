//! The build: packing the records, each reply's check, the whole build against a stub `claude`
//! (fixed instructions, no held-out answer in any prompt, one retry, the cache, the cap), the draft
//! in the template's shape, the measurements entry and the write.
use serde_json::{Map, Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, UNIX_EPOCH},
};
use voice_agent::guard::Guard;
use voice_agent::{
    claude::Claude,
    ledger::{Cap, Session},
};
use voice_build::{
    build::{self, Inputs, Misfit},
    draft,
};
use voice_core::ceilings::Ceilings;
use voice_core::member::Member;
use voice_ingest::{corpus::read_records, filters::Filters};
use voice_steps::{
    book::{Book, book},
    distil::Group,
    labels::Mode,
    score::Cell,
};

/// A word only the model or a held-out answer writes: it must never reach an error or a prompt.
const SENTINEL: &str = "zebracorn";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-build-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Scratch(path.canonicalize().unwrap())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn member() -> Member {
    Member::from_value(
        json!({"name": "Ada", "slug": "ada", "pronouns": "she/her", "principal": "Sam",
               "principal_slug": "sam", "principal_pronouns": "they/them",
               "state_dir": "~/.claude/state/ada", "mark_word": "Ada",
               "filters": {"excluded_folders": "^-private-", "personal": "\\bmy therapist\\b",
                           "secret": "\\b\\d{9,}\\b", "pasted_prefixes": ["You are "]},
               "codebook": {"categories": ["scope", "methods", "tooling", "other"],
                            "domains": ["teaching", "other"],
                            "escalates": ["submit"], "signals": ["ruling", "other"],
                            "groups": {"g1": ["scope", "methods"], "g2": ["tooling", "other"]}}}),
        "member.json",
    )
    .unwrap()
}
fn filters() -> Filters {
    Filters::compile(&member().filters).unwrap()
}
fn the_book() -> Book {
    book(&member()).unwrap()
}

const HEADERS: [&str; 3] = ["Scope", "Methods", "Tooling"];
const TOPICS: [&str; 6] = [
    "weekly planning notes",
    "seed catalogue order",
    "lesson handout layout",
    "shared drive folders",
    "parent newsletter draft",
    "greenhouse watering rota",
];

fn question(id: &str, n: usize, split: &str) -> Value {
    let topic = TOPICS[n % TOPICS.len()];
    json!({"id": id, "kind": "question", "timestamp": format!("2026-09-{:02}T10:00:00.000Z", 1 + n % 28),
           "project": "kit", "header": HEADERS[n % 3],
           "question": format!("Should the {topic} stay short this term, number {n}?"),
           "multiSelect": false,
           "options": [{"label": "Keep it short", "description": "Less to read"},
                       {"label": "Make it longer", "description": "More detail"}],
           "recommended": null, "picks": ["Keep it short"], "own_words": "",
           "notes": if split == "heldout" { format!("{SENTINEL} answer note") } else { String::new() },
           "outcome": "no_rec", "split": split, "ada_pick": null})
}
fn message(id: &str, n: usize) -> Value {
    json!({"id": id, "kind": "message", "timestamp": "2026-08-20T10:00:00.000Z", "project": "kit",
           "text": format!("Please always keep the {} tidy and easy to find later on", TOPICS[n % 6]),
           "split": "train"})
}

/// `train` training questions, two messages and `held` held-out questions.
fn corpus(train: usize, held: usize) -> String {
    let mut lines = Vec::new();
    for n in 0..train {
        lines.push(question(&format!("question:t{n:03}"), n, "train"));
    }
    for n in 0..2 {
        lines.push(message(&format!("message:m{n}"), n));
    }
    for n in 0..held {
        lines.push(question(&format!("question:h{n:03}"), n, "heldout"));
    }
    lines.iter().map(|l| format!("{l}\n")).collect()
}

fn records(text: &str) -> Vec<(usize, Map<String, Value>)> {
    read_records("decisions.jsonl", text.as_bytes()).unwrap()
}

fn python() -> Option<PathBuf> {
    let found = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()));
    if found.is_none() && std::env::var("VOICE_REQUIRE_PARITY").as_deref() == Ok("1") {
        panic!("python3 is needed for the stub claude");
    }
    found
}

/// The stub `claude` from `fixtures/claude_stub.py`, in a folder of its own.
struct Stub {
    folder: Scratch,
}
impl Stub {
    fn new(python: &Path) -> Self {
        let folder = Scratch::new();
        fs::create_dir(folder.0.join("profile")).unwrap();
        fs::set_permissions(folder.0.join("profile"), fs::Permissions::from_mode(0o700)).unwrap();
        let script = include_str!("fixtures/claude_stub.py");
        let program = folder.0.join("claude");
        fs::write(&program, format!("#!{} -I\n{script}", python.display())).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        Stub { folder }
    }
    fn plan(&self, plan: Value) {
        fs::write(self.folder.0.join("plan.json"), plan.to_string()).unwrap();
    }
    fn claude(&self) -> Claude {
        Claude::at(self.folder.0.join("claude"), self.folder.0.join("profile"))
    }
    fn calls(&self) -> usize {
        (0..)
            .take_while(|n| self.folder.0.join(format!("stdin{n}")).exists())
            .count()
    }
    fn stdin(&self, n: usize) -> String {
        fs::read_to_string(self.folder.0.join(format!("stdin{n}"))).unwrap()
    }
    fn argv(&self, n: usize) -> Vec<String> {
        serde_json::from_str(&fs::read_to_string(self.folder.0.join(format!("argv{n}"))).unwrap())
            .unwrap()
    }
    /// The first line of each call's prompt.
    fn steps(&self) -> Vec<String> {
        (0..self.calls())
            .map(|n| self.stdin(n).lines().next().unwrap_or_default().to_owned())
            .collect()
    }
}

fn session(home: &Path, cap: Cap) -> Session {
    Session::open_with_clock(
        home,
        "ada",
        cap,
        Box::new(|| UNIX_EPOCH + Duration::from_secs(1_793_000_000)),
    )
    .unwrap()
}

fn after(argv: &[String], flag: &str) -> String {
    let at = argv.iter().position(|a| a == flag).unwrap();
    argv[at + 1].clone()
}

const CODEBOOK: &str = "| category | use for |\n|---|---|\n| scope | what to include |\n";

fn run(
    stub: &Stub,
    home: &Path,
    corpus: &str,
    cap: Cap,
    lines: &mut Vec<String>,
) -> voice_core::error::Result<build::Built> {
    let member = member();
    let book = the_book();
    let filters = filters();
    let session = session(home, cap);
    build::build(
        &Guard::new(vec![&stub.claude()], Ceilings::defaults()),
        &session,
        home,
        &Inputs {
            member: &member,
            book: &book,
            codebook: CODEBOOK,
            corpus_name: "decisions.jsonl",
            corpus: corpus.as_bytes(),
            filters: &filters,
            model: "sonnet",
        },
        &mut |line| lines.push(line.to_owned()),
    )
}

#[test]
fn parts_hold_at_most_300_records_and_400_000_bytes() {
    let text = corpus(650, 3);
    let parts = build::parts("decisions.jsonl", &records(&text)).unwrap();
    let names: Vec<_> = parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["train-01", "train-02", "train-03", "heldout-01"]);
    let sizes: Vec<_> = parts.iter().map(|p| p.records.len()).collect();
    assert_eq!(sizes, [300, 300, 52, 3]);
    assert_eq!(parts[3].mode, Mode::Heldout);
    // A held-out question goes without its answer.
    assert!(parts[3].records[0].1.get("picks").is_none());
    assert!(parts[3].records[0].1.get("notes").is_none());

    let long = |id: &str| {
        let mut q = question(id, 1, "train");
        q["question"] = Value::from("word ".repeat(30_000));
        q
    };
    let text: String = (0..5)
        .map(|n| format!("{}\n", long(&format!("question:l{n}"))))
        .collect();
    let parts = build::parts("decisions.jsonl", &records(&text)).unwrap();
    // Each record is about 150,000 bytes: two fit in a part, a third does not.
    let sizes: Vec<_> = parts.iter().map(|p| p.records.len()).collect();
    assert_eq!(sizes, [2, 2, 1]);
}

fn part(mode: Mode) -> build::Part {
    let text = corpus(2, 1);
    let parts = build::parts("decisions.jsonl", &records(&text)).unwrap();
    parts.into_iter().find(|p| p.mode == mode).unwrap()
}

#[test]
fn label_replies_are_rebuilt_per_kind_and_mode() {
    let reply = json!({"labels": [
        {"id": "question:t000", "category": "scope", "domain": "teaching", "escalates": [], "signal": "ruling", "reason": "Small and clear."},
        {"id": "question:t001", "category": "methods", "domain": "teaching", "escalates": ["submit"], "signal": null, "reason": "It was the safer path."},
        {"id": "message:m0", "category": "tooling", "domain": "teaching", "escalates": ["submit"], "signal": "ruling", "reason": "A standing rule."},
        {"id": "message:m1", "category": "tooling", "domain": "other", "escalates": [], "signal": "other", "reason": "Asked for tidiness."}
    ]});
    let labels = build::check_label_reply(&the_book(), &part(Mode::Train), &reply).unwrap();
    assert_eq!(
        labels["question:t000"],
        json!({"category": "scope", "domain": "teaching", "escalates": [], "reason": "Small and clear."})
    );
    assert_eq!(
        labels["message:m0"],
        json!({"category": "tooling", "domain": "teaching", "signal": "ruling", "reason": "A standing rule."})
    );
    let held = json!({"labels": [{"id": "question:h000", "category": "scope", "domain": "teaching",
                                  "escalates": [], "signal": null, "reason": "Peeked at the answer."}]});
    let labels = build::check_label_reply(&the_book(), &part(Mode::Heldout), &held).unwrap();
    assert_eq!(
        labels["question:h000"],
        json!({"category": "scope", "domain": "teaching", "escalates": []})
    );
}

#[test]
fn label_replies_that_do_not_fit_are_refused_without_the_models_words() {
    let train = part(Mode::Train);
    let good = |edit: &dyn Fn(&mut Value)| {
        let mut reply = json!({"labels": [
            {"id": "question:t000", "category": "scope", "domain": "teaching", "escalates": [], "signal": null, "reason": "Small."},
            {"id": "question:t001", "category": "methods", "domain": "teaching", "escalates": [], "signal": null, "reason": "Safe."},
            {"id": "message:m0", "category": "tooling", "domain": "teaching", "escalates": [], "signal": "ruling", "reason": "Rule."},
            {"id": "message:m1", "category": "tooling", "domain": "teaching", "escalates": [], "signal": "ruling", "reason": "Rule."}
        ]});
        edit(&mut reply);
        build::check_label_reply(&the_book(), &train, &reply)
    };
    assert!(good(&|_| {}).is_ok());
    let wrong = good(&|r| r["labels"][0]["category"] = Value::from(SENTINEL)).unwrap_err();
    assert!(wrong.told.contains("question:t000 category"), "{wrong:?}");
    assert!(!wrong.shown.contains(SENTINEL) && !wrong.told.contains(SENTINEL));
    assert_eq!(wrong.shown, "1 label didn't fit the codebook");
    let fields = good(&|r| {
        r["labels"][0]["category"] = Value::from(SENTINEL);
        r["labels"][0]["domain"] = Value::from(SENTINEL);
    })
    .unwrap_err();
    assert_eq!(fields.shown, "1 label didn't fit the codebook");
    assert!(
        fields
            .told
            .contains("question:t000 category; question:t000 domain")
    );
    let missing = good(&|r| {
        r["labels"].as_array_mut().unwrap().pop();
    })
    .unwrap_err();
    assert_eq!(missing.told, "these records have no label: message:m1");
    assert_eq!(missing.shown, "1 record had no label");
    let twice = good(&|r| {
        let first = r["labels"][0].clone();
        r["labels"].as_array_mut().unwrap().push(first);
    })
    .unwrap_err();
    assert_eq!(
        twice,
        Misfit {
            told: "these records are labelled more than once: question:t000".into(),
            shown: "1 record was labelled more than once".into()
        }
    );
    let extra = good(&|r| {
        let mut label = r["labels"][0].clone();
        label["id"] = Value::from(SENTINEL);
        r["labels"].as_array_mut().unwrap().push(label);
    })
    .unwrap_err();
    assert_eq!(
        extra.told,
        "leave out the 1 labels whose id is not a record here"
    );
    assert_eq!(extra.shown, "1 label was for a record that isn't there");
    assert!(!extra.told.contains(SENTINEL) && !extra.shown.contains(SENTINEL));
    for (repeats, shown) in [
        (1, "1 label was for a record that isn't there"),
        (2, "2 labels were for records that aren't there"),
    ] {
        let repeated = good(&|r| {
            let mut label = r["labels"][0].clone();
            label["id"] = Value::from(SENTINEL);
            for _ in 0..=repeats {
                r["labels"].as_array_mut().unwrap().push(label.clone());
            }
        })
        .unwrap_err();
        assert_eq!(
            repeated.told,
            format!("leave out the {repeats} repeated labels whose id is not a record here")
        );
        assert_eq!(repeated.shown, shown);
    }
    let mixed = good(&|r| {
        let first = r["labels"][0].clone();
        r["labels"].as_array_mut().unwrap().push(first.clone());
        let mut extra = first;
        extra["id"] = Value::from(SENTINEL);
        r["labels"]
            .as_array_mut()
            .unwrap()
            .extend([extra.clone(), extra]);
    })
    .unwrap_err();
    assert_eq!(
        mixed.told,
        "these records are labelled more than once: question:t000; leave out the 1 repeated labels whose id is not a record here"
    );
    assert_eq!(
        mixed.shown,
        "1 record was labelled more than once, 1 label was for a record that isn't there"
    );
    let no_reason = good(&|r| r["labels"][1]["reason"] = Value::Null).unwrap_err();
    assert!(
        no_reason.told.contains("question:t001 reason"),
        "{no_reason:?}"
    );
    let not_a_list =
        build::check_label_reply(&the_book(), &train, &json!({"labels": 3})).unwrap_err();
    assert_eq!(not_a_list.shown, "the answer holds no list of labels");
}

#[test]
fn label_replies_count_a_repeated_record_id_as_missing_once() {
    let mut train = part(Mode::Train);
    train.records.push(train.records[0].clone());
    let reply = json!({"labels": [
        {"id": "question:t001", "category": "methods", "domain": "teaching", "escalates": [], "reason": "Safe."},
        {"id": "message:m0", "category": "tooling", "domain": "teaching", "signal": "ruling", "reason": "Rule."},
        {"id": "message:m1", "category": "tooling", "domain": "teaching", "signal": "ruling", "reason": "Rule."}
    ]});
    let misfit = build::check_label_reply(&the_book(), &train, &reply).unwrap_err();
    assert_eq!(misfit.told, "these records have no label: question:t000");
    assert_eq!(misfit.shown, "1 record had no label");
}

#[test]
fn a_record_id_starting_with_ids_differ_keeps_its_field_problem() {
    let mut train = part(Mode::Train);
    let id = "ids differ: invented record";
    train.records[0].1["id"] = Value::from(id);
    let reply = json!({"labels": train.records.iter().map(|(_, r)| {
        json!({"id": r["id"], "category": if r["id"] == id { SENTINEL } else { "scope" },
               "domain": "teaching", "escalates": [], "signal": "ruling", "reason": "Small."})
    }).collect::<Vec<_>>()});
    let misfit = build::check_label_reply(&the_book(), &train, &reply).unwrap_err();
    assert!(
        misfit.told.contains(&format!("{id} category")),
        "{misfit:?}"
    );
    assert_eq!(misfit.shown, "1 label didn't fit the codebook");
}

fn group(rows: usize) -> (Group, Vec<(String, usize)>) {
    let rows: Vec<Map<String, Value>> = (0..rows)
        .map(|n| {
            json!({"category": if n % 2 == 0 { "methods" } else { "scope" }, "q": "x"})
                .as_object()
                .unwrap()
                .clone()
        })
        .collect();
    let group = Group {
        name: "g1".into(),
        rows,
    };
    let categories = build::group_categories(&the_book(), &group);
    (group, categories)
}

fn sections(edit: &dyn Fn(&mut Value)) -> Value {
    let mut reply = json!({"sections": [
        {"category": "methods", "lean": ["They try the simplest method first."], "flips": [],
         "own_answer": [], "rules": ["2026-09-01: always test on a copy."],
         "precedents": ["2026-09-02 · kit · chose the plain approach"]},
        {"category": "scope", "lean": ["They cut to what the reader needs.", "They keep lists short."],
         "flips": ["A deadline makes them cut more."], "own_answer": [], "rules": [],
         "precedents": []}
    ]});
    edit(&mut reply);
    reply
}

#[test]
fn distil_replies_render_in_the_briefs_shape_in_codebook_order() {
    let (_, categories) = group(12);
    assert_eq!(
        categories,
        vec![("scope".to_owned(), 6), ("methods".to_owned(), 6)]
    );
    let text = corpus(4, 0);
    let rendered = build::check_distil_reply(
        &categories,
        "decisions.jsonl",
        &records(&text),
        &filters(),
        &sections(&|_| {}),
    )
    .unwrap();
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0].0, "scope");
    assert_eq!(
        rendered[0].1,
        "### scope\n\n**The lean:**\n- They cut to what the reader needs.\n- They keep lists short.\n\n**What flips it:**\n- A deadline makes them cut more.\n\n**When they write their own answer:**\n- None yet.\n\n**Standing rules:**\n- None yet.\n\n**Precedents:**\n- None yet.\n\n"
    );
    let (_, thin) = group(3);
    let rendered = build::check_distil_reply(
        &thin,
        "decisions.jsonl",
        &records(&text),
        &filters(),
        &sections(&|_| {}),
    )
    .unwrap();
    assert!(
        rendered[0]
            .1
            .ends_with("\n_Thin: 1 records; treat as Low until more history exists._\n\n"),
        "{}",
        rendered[0].1
    );
}

#[test]
fn distil_replies_that_do_not_fit_are_refused() {
    let (_, categories) = group(12);
    let text = corpus(4, 0);
    let corpus = records(&text);
    let check = |edit: &dyn Fn(&mut Value)| {
        build::check_distil_reply(
            &categories,
            "decisions.jsonl",
            &corpus,
            &filters(),
            &sections(edit),
        )
        .unwrap_err()
    };
    let shown = |edit: &dyn Fn(&mut Value)| check(edit).shown;
    assert_eq!(
        shown(&|r| {
            r["sections"].as_array_mut().unwrap().remove(0);
        }),
        "there is no section for methods"
    );
    assert_eq!(
        shown(&|r| r["sections"][0]["category"] = Value::from(SENTINEL)),
        "a section is about a kind of decision that wasn't asked for"
    );
    assert_eq!(
        shown(&|r| {
            let first = r["sections"][0].clone();
            r["sections"].as_array_mut().unwrap().push(first);
        }),
        "there are two sections for methods"
    );
    assert_eq!(
        shown(&|r| r["sections"][0]["lean"] = json!([])),
        "methods: lean needs 1 to 6 points"
    );
    assert_eq!(
        shown(&|r| r["sections"][0]["flips"] = json!(["a", "b", "c", "d", "e", "f"])),
        "methods: flips needs 0 to 5 points"
    );
    assert_eq!(
        shown(&|r| r["sections"][0]["lean"][0] = Value::from(format!("{SENTINEL}\n### other"))),
        "methods: lean point 1 must be one line of text"
    );
    assert_eq!(
        shown(&|r| r["sections"][0]["lean"][0] = Value::from("x".repeat(401))),
        "methods: lean point 1 is longer than 400 characters"
    );
    assert_eq!(
        shown(&|r| r["sections"][0]["precedents"][0] = Value::from("kit · chose the plain one")),
        "methods: precedent 1 must start with its date, YYYY-MM-DD · "
    );
    assert_eq!(
        shown(&|r| r["sections"][0]["lean"] =
            Value::from(vec!["a ".repeat(199).trim().to_owned(); 6])),
        "methods: the section is longer than 900 words"
    );
    let private = check(&|r| {
        r["sections"][1]["flips"][0] =
            Value::from(format!("They ask {SENTINEL} my therapist first."))
    });
    assert_eq!(private.shown, "1 point looked personal or secret");
    let private_many = check(&|r| {
        r["sections"][0]["lean"][0] = Value::from("They ask my therapist first.");
        r["sections"][1]["flips"][0] = Value::from("They ask my therapist next.");
    });
    assert_eq!(private_many.shown, "2 points looked personal or secret");
    assert!(private.told.contains("scope flips point 1"), "{private:?}");
    assert!(!private.told.contains(SENTINEL));
    let own = "Should the weekly planning notes stay short this term";
    let quoted = check(&|r| r["sections"][0]["lean"][0] = Value::from(own));
    assert_eq!(quoted.shown, "4 runs of 6 or more of your own words");
    let one_quote = check(&|r| {
        r["sections"][0]["lean"][0] = Value::from("Should the weekly planning notes stay");
    });
    assert_eq!(one_quote.shown, "1 run of 6 or more of your own words");
    assert!(
        quoted
            .told
            .contains("\"should the weekly planning notes stay\""),
        "{quoted:?}"
    );
}

#[test]
fn backtest_batches_are_near_equal_and_about_forty() {
    let sizes = |n: usize| {
        let questions: Vec<Map<String, Value>> = (0..n).map(|_| Map::new()).collect();
        build::batches(&questions)
            .iter()
            .map(|b| b.len())
            .collect::<Vec<_>>()
    };
    assert_eq!(sizes(30), [30]);
    assert_eq!(sizes(40), [40]);
    assert_eq!(sizes(41), [21, 20]);
    assert_eq!(sizes(81), [27, 27, 27]);
    assert!(sizes(0).is_empty());
}

#[test]
fn backtest_replies_count_a_repeated_question_id_as_missing_once() {
    let batch: Vec<Map<String, Value>> = [
        question("question:h000", 0, "heldout"),
        question("question:h000", 0, "heldout"),
        question("question:h001", 1, "heldout"),
    ]
    .iter()
    .map(|q| q.as_object().unwrap().clone())
    .collect();
    let reply = json!({"predictions": [
        {"id": "question:h001", "pick": "Keep it short", "confidence": "high"}
    ]});
    let misfit = build::check_backtest_reply(&batch, &reply).unwrap_err();
    assert_eq!(
        misfit.told,
        "these questions have no prediction: question:h000"
    );
    assert_eq!(misfit.shown, "1 question had no prediction");
}

#[test]
fn backtest_replies_need_one_fitting_pick_per_question() {
    let mut multi = question("question:h001", 1, "heldout");
    multi["multiSelect"] = Value::Bool(true);
    let batch: Vec<Map<String, Value>> = [question("question:h000", 0, "heldout"), multi]
        .iter()
        .map(|q| q.as_object().unwrap().clone())
        .collect();
    let check = |predictions: Value| {
        build::check_backtest_reply(&batch, &json!({"predictions": predictions}))
    };
    let good = check(json!([
        {"id": "question:h000", "pick": "Keep it short", "confidence": "high"},
        {"id": "question:h001", "pick": ["Keep it short", "Make it longer"], "confidence": "low"}
    ]))
    .unwrap();
    assert_eq!(
        good["question:h001"],
        json!({"pick": ["Keep it short", "Make it longer"], "confidence": "low"})
    );
    assert!(
        check(json!([
            {"id": "question:h000", "pick": "own", "confidence": "medium"},
            {"id": "question:h001", "pick": "own", "confidence": "medium"}
        ]))
        .is_ok()
    );
    let fields = check(json!([
        {"id": "question:h000", "pick": SENTINEL, "confidence": SENTINEL},
        {"id": "question:h001", "pick": "own", "confidence": "high"}
    ]))
    .unwrap_err();
    assert_eq!(fields.shown, "1 prediction didn't fit");
    assert!(
        fields
            .told
            .contains("question:h000 pick; question:h000 confidence")
    );
    for (predictions, told) in [
        (
            json!([{"id": "question:h000", "pick": SENTINEL, "confidence": "high"},
                {"id": "question:h001", "pick": ["Keep it short"], "confidence": "high"}]),
            "question:h000 pick",
        ),
        (
            json!([{"id": "question:h000", "pick": ["Keep it short"], "confidence": "high"},
                {"id": "question:h001", "pick": ["Keep it short"], "confidence": "high"}]),
            "question:h000 pick",
        ),
        (
            json!([{"id": "question:h000", "pick": "Keep it short", "confidence": "high"},
                {"id": "question:h001", "pick": "Keep it short", "confidence": "high"}]),
            "question:h001 pick",
        ),
        (
            json!([{"id": "question:h000", "pick": "Keep it short", "confidence": "high"},
                {"id": "question:h001", "pick": ["Keep it short", "Keep it short"], "confidence": "high"}]),
            "question:h001 pick",
        ),
        (
            json!([{"id": "question:h000", "pick": "Keep it short", "confidence": "sure"},
                {"id": "question:h001", "pick": "own", "confidence": "high"}]),
            "question:h000 confidence",
        ),
        (
            json!([{"id": "question:h000", "pick": "Keep it short", "confidence": "high"},
                {"id": SENTINEL, "pick": "own", "confidence": "high"}]),
            "these questions have no prediction: question:h001; leave out the 1 predictions whose id is not a question here",
        ),
    ] {
        let misfit = check(predictions).unwrap_err();
        assert!(misfit.told.contains(told), "{misfit:?}");
        assert!(!misfit.told.contains(SENTINEL) && !misfit.shown.contains(SENTINEL));
        if told.starts_with("these questions have no prediction:") {
            assert_eq!(
                misfit.shown,
                "1 question had no prediction, 1 prediction was for a question that wasn't asked"
            );
        } else {
            assert_eq!(misfit.shown, "1 prediction didn't fit");
        }
    }
}

#[test]
fn a_category_is_live_at_five_high_picks_and_ninety_percent() {
    let at = |n, hits| build::live(Some(&Cell { n, hits }));
    assert!(at(5, 5));
    assert!(!at(4, 4));
    assert!(at(10, 9));
    assert!(!at(10, 8));
    assert!(!build::live(None));
}

#[test]
fn the_build_labels_distils_and_tests_blind_with_fixed_instructions() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    let text = corpus(12, 32);
    let mut lines = Vec::new();
    let built = run(&stub, &home.0, &text, Cap::default(), &mut lines).unwrap();
    assert_eq!(
        stub.steps(),
        [
            "Label every record below for the person's assistant.",
            "Label every record below for the person's assistant.",
            "Write how the person decides, one section for each kind of decision below.",
            "Write how the person decides, one section for each kind of decision below.",
            "Predict the person's answer to each question below."
        ]
    );
    for n in 0..stub.calls() {
        // No held-out answer in any prompt, and nothing of the person's in the argv.
        assert!(!stub.stdin(n).contains(SENTINEL), "call {n}");
        let argv = stub.argv(n).join("\n");
        assert!(
            !argv.contains("weekly planning") && !argv.contains("Sam"),
            "call {n}"
        );
    }
    let (systems, schemas) = (
        [
            build::LABEL_SYSTEM,
            build::LABEL_SYSTEM,
            build::DISTIL_SYSTEM,
            build::DISTIL_SYSTEM,
            build::BACKTEST_SYSTEM,
        ],
        [
            build::label_schema(),
            build::label_schema(),
            build::distil_schema(),
            build::distil_schema(),
            build::backtest_schema(),
        ],
    );
    for n in 0..5 {
        assert_eq!(after(&stub.argv(n), "--system-prompt"), systems[n]);
        assert_eq!(
            after(&stub.argv(n), "--json-schema"),
            schemas[n].to_string()
        );
    }
    assert!(stub.stdin(0).contains("Mode: train") && stub.stdin(1).contains("Mode: heldout"));
    assert!(stub.stdin(0).contains(CODEBOOK));
    assert!(
        stub.stdin(4).contains("### scope")
            && stub.stdin(4).contains("Closest past decisions:\n- ")
    );
    assert_eq!(built.calls, 5);
    assert_eq!(built.reused, 0);
    assert_eq!(built.held, 32);
    let score = &built.backtest.as_ref().unwrap().score;
    assert_eq!((score.overall.hits, score.overall.n), (32, 32));
    assert!(built.sections.starts_with("### scope\n"));
    let order: Vec<_> = built
        .sections
        .lines()
        .filter(|l| l.starts_with("### "))
        .collect();
    assert_eq!(order, ["### scope", "### methods", "### tooling"]);
    let ledger = fs::read_to_string(home.0.join(".claude/state/ada/runs.jsonl")).unwrap();
    for step in ["label", "distil", "backtest"] {
        assert!(ledger.contains(&format!("\"step\":\"{step}\"")), "{ledger}");
    }
    assert_eq!(
        lines,
        [
            "Reading your history: 44 answered questions, 32 of them kept back to test your assistant.",
            "Sorting your decisions by kind, part 1 of 2: done.",
            "Sorting your decisions by kind, part 2 of 2: done.",
            "Learning how you decide, group 1 of 2: done.",
            "Learning how you decide, group 2 of 2: done.",
            "Testing your assistant on 32 questions it hasn't seen, batch 1 of 1: done."
        ]
    );
}

#[test]
fn a_reply_that_does_not_fit_is_asked_for_once_more() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    stub.plan(json!({"0": "bad"}));
    let built = run(
        &stub,
        &home.0,
        &corpus(12, 0),
        Cap::default(),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(built.calls, 4);
    let retry = stub.stdin(1);
    assert!(retry.starts_with(&stub.stdin(0)));
    assert!(
        retry.contains("\nYour last answer didn't fit: these labels are wrong"),
        "{retry}"
    );
    assert!(retry.ends_with(PARTIAL_RETRY_END));
}

#[test]
fn a_distil_retry_keeps_the_full_reply_instruction() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    stub.plan(json!({"1": "bad"}));
    run(
        &stub,
        &home.0,
        &corpus(12, 0),
        Cap::default(),
        &mut Vec::new(),
    )
    .unwrap();
    let retry = stub.stdin(2);
    assert!(
        retry.ends_with("Answer again in full, following every rule above.\n"),
        "{retry}"
    );
    assert!(!retry.contains("You may give only"), "{retry}");
}

#[test]
fn two_replies_that_do_not_fit_stop_the_build() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    stub.plan(json!({"1": "bad", "2": "bad"}));
    let error = run(
        &stub,
        &home.0,
        &corpus(12, 0),
        Cap::default(),
        &mut Vec::new(),
    )
    .err()
    .unwrap()
    .to_string();
    assert_eq!(
        error,
        "Learning how you decide, group 1 of 2: Claude's answer didn't fit twice, so the build stopped (the last problem: there is no section for scope). What it already finished is kept: run the build again to carry on."
    );
    assert_eq!(stub.calls(), 3);
    let quoted = Stub::new(&python);
    quoted.plan(json!({"1": "quote", "2": "quote"}));
    let error = run(
        &quoted,
        &Scratch::new().0,
        &corpus(12, 0),
        Cap::default(),
        &mut Vec::new(),
    )
    .err()
    .unwrap()
    .to_string();
    assert!(
        error.contains("runs of 6 or more of your own words"),
        "{error}"
    );
    assert!(!error.contains("planning notes"), "{error}");
}

#[test]
fn a_rerun_reuses_every_reply_already_paid_for() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let text = corpus(12, 32);
    let first = Stub::new(&python);
    first.plan(json!({"4": "error"}));
    let error = run(&first, &home.0, &text, Cap::default(), &mut Vec::new())
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("Claude Code couldn't finish"),
        "{error}"
    );
    assert_eq!(first.calls(), 5);
    let second = Stub::new(&python);
    let mut lines = Vec::new();
    let built = run(&second, &home.0, &text, Cap::default(), &mut lines).unwrap();
    assert_eq!(
        second.steps(),
        ["Predict the person's answer to each question below."]
    );
    assert_eq!((built.calls, built.reused), (1, 4));
    assert!(
        lines.contains(&"Sorting your decisions by kind, part 1 of 2: already done.".to_owned())
    );
    // A kept reply that no longer fits its check is asked for again.
    let kept = home.0.join(".claude/state/ada/build");
    for entry in fs::read_dir(&kept).unwrap() {
        let path = entry.unwrap().path();
        if path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("distil-")
        {
            fs::write(&path, "{\"sections\": []}").unwrap();
        }
    }
    let third = Stub::new(&python);
    let built = run(&third, &home.0, &text, Cap::default(), &mut Vec::new()).unwrap();
    assert_eq!((built.calls, built.reused), (2, 3));
}

#[test]
fn the_daily_cap_stops_the_build_before_it_spends_more() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    let error = run(
        &stub,
        &home.0,
        &corpus(12, 0),
        Cap::new(5.0, 5.0).unwrap(),
        &mut Vec::new(),
    )
    .err()
    .unwrap()
    .to_string();
    assert!(
        error.contains("spending limit for this assistant is reached"),
        "{error}"
    );
    assert_eq!(stub.calls(), 1);
}

#[test]
fn too_few_held_out_questions_skip_the_backtest() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    let mut lines = Vec::new();
    let built = run(&stub, &home.0, &corpus(12, 29), Cap::default(), &mut lines).unwrap();
    assert!(built.backtest.is_none());
    assert_eq!(built.held, 29);
    assert!(!stub.steps().iter().any(|s| s.starts_with("Predict")));
    // Held-out labels serve only the test, so none is paid for without one.
    assert_eq!(stub.calls(), 3);
    assert!((0..3).all(|n| !stub.stdin(n).contains("Mode: heldout")));
    assert!(
        lines
            .last()
            .unwrap()
            .starts_with("Only 29 questions were kept back")
    );
}

#[test]
fn nothing_to_train_on_is_refused_before_any_call() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    let held_only: String = (0..3)
        .map(|n| format!("{}\n", question(&format!("question:h{n}"), n, "heldout")))
        .collect();
    let error = run(&stub, &home.0, &held_only, Cap::default(), &mut Vec::new())
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.starts_with("There's no history to build from yet."),
        "{error}"
    );
    assert_eq!(stub.calls(), 0);
}

fn built(home: &Path, held: usize) -> build::Built {
    let python = python().expect("python3");
    let stub = Stub::new(&python);
    let mut text = corpus(12, held);
    text.push_str(&format!("{}\n", {
        let mut q = question("question:interview-01", 3, "train");
        q["project"] = Value::from("interview");
        q
    }));
    run(&stub, home, &text, Cap::default(), &mut Vec::new()).unwrap()
}

#[test]
fn the_draft_keeps_every_line_of_the_template() {
    if python().is_none() {
        return;
    }
    let home = Scratch::new();
    let built = built(&home.0, 32);
    let text = draft::decisions(&member(), &the_book(), &built);
    let mut rest = text.as_str();
    let mut placeholder = false;
    for line in draft::filled_template(&member()).lines() {
        if placeholder || line.starts_with('<') {
            placeholder = !line.ends_with('>');
            continue;
        }
        if ["mined_through: ", "sources: ", "generated_by: "]
            .iter()
            .any(|p| line.starts_with(p))
        {
            continue;
        }
        let at = rest
            .find(line)
            .unwrap_or_else(|| panic!("missing or out of order: {line}"));
        rest = &rest[at + line.len()..];
    }
    assert!(text.contains("\nmined_through: 2026-09-12\n"), "{text}");
    assert!(text.contains(
        "\nsources: 44 answered questions and 2 typed messages in Claude Code, 2026-08-20 → 2026-09-28; 1 interview answers\n"
    ), "{text}");
    assert!(
        text.contains("\n| scope | 11/11 | — | — | yes |\n"),
        "{text}"
    );
    assert!(
        text.contains("\n| Category | Questions | Took rec |"),
        "{text}"
    );
    assert!(text.contains("Overall 32 of 32 (100%)"), "{text}");
    assert!(text.contains("A category is **live** at 5 or more High picks with at least 90% right: scope, methods, tooling. Ada decides alone"), "{text}");
    assert!(
        text.contains("\n_Thin: 4 records; treat as Low until more history exists._\n"),
        "{text}"
    );
    assert!(!text.contains("{{") && !text.contains("<Filled"), "{text}");

    let skipped = built_without_test(&home);
    let text = draft::decisions(&member(), &the_book(), &skipped);
    assert!(text.contains("There were too few held-out questions to backtest (29, fewer than 30), so no category is live."), "{text}");
    assert!(text.contains("\n| scope | — | — | — | no |\n"), "{text}");
}

fn built_without_test(home: &Scratch) -> build::Built {
    let fresh = Scratch::new();
    let _ = home;
    built(&fresh.0, 29)
}

#[test]
fn the_measurements_entry_replaces_the_template_examples_before_the_reports() {
    if python().is_none() {
        return;
    }
    let home = Scratch::new();
    let built = built(&home.0, 32);
    let entries = draft::entries(&the_book(), &built, "sonnet", "2026-10-25");
    assert!(entries.starts_with("## 2026-10-25: build backtest\n\n- **Set:** 32 held-out questions, 2026-09-01 → 2026-09-28:"), "{entries}");
    assert!(entries.contains("- **Result:** 32 of 32 (100%); always-take-the-recommendation baseline 0%. High 32/32, Medium none, Low none.\n"), "{entries}");
    assert!(
        entries.contains("- **Read:** live: scope, methods, tooling.\n"),
        "{entries}"
    );
    assert!(
        entries.contains("\n## 2026-10-25: calibration interview\n\n- 1 interview answers,"),
        "{entries}"
    );
    let template = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template/measurements.md"),
    )
    .unwrap();
    let text = draft::measurements(Some(&template), &member(), &entries);
    assert!(!text.contains("YYYY-MM-DD"), "{text}");
    let entry = text.find("## 2026-10-25: build backtest").unwrap();
    let reports = text.find("## Reports").unwrap();
    assert!(
        entry < reports && text.starts_with("# {{NAME}} measurements\n\nEvery test"),
        "{text}"
    );
    assert!(
        text.contains("training data, never test data).\n\n## Reports\n"),
        "{text}"
    );
    let again = draft::measurements(
        Some(&text),
        &member(),
        "## 2026-10-26: build backtest\n\n- later\n",
    );
    assert!(again.contains("(training data, never test data).\n\n## 2026-10-26: build backtest\n\n- later\n\n## Reports"), "{again}");
    let fresh = draft::measurements(None, &member(), &entries);
    assert!(fresh.starts_with("# Ada measurements\n\nEvery test of Ada, newest last. Paraphrase only: never paste one of Sam's answers here.\n\n## 2026-10-25: build backtest\n"), "{fresh}");
    let skipped = draft::entries(
        &the_book(),
        &built_without_test(&home),
        "sonnet",
        "2026-10-25",
    );
    assert!(
        skipped.starts_with(
            "## 2026-10-25: build (no backtest)\n\n- **Set:** 29 held-out questions, fewer than 30"
        ),
        "{skipped}"
    );
}

#[test]
fn a_draft_a_filter_marks_is_refused_by_place() {
    let marked = "# How Sam decides\n\nThey ask my therapist.\n";
    let error = draft::check_private(&filters(), marked, "- fine\n- 1234567890\n")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("decisions.md:3 (personal), the new measurements entry:2 (secret)"),
        "{error}"
    );
    assert!(!error.contains("therapist") && !error.contains("1234567890"));
    assert!(draft::check_private(&filters(), "# fine\n", "- fine\n").is_ok());
}

#[test]
fn writing_changes_the_folder_only_when_both_files_are_ready() {
    let dir = Scratch::new();
    fs::write(dir.0.join("decisions.md"), "old decisions\n").unwrap();
    fs::set_permissions(
        dir.0.join("decisions.md"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    draft::write(&dir.0, "new decisions\n", "new measurements\n").unwrap();
    assert_eq!(
        fs::read_to_string(dir.0.join("decisions.md")).unwrap(),
        "new decisions\n"
    );
    assert_eq!(
        fs::read_to_string(dir.0.join("measurements.md")).unwrap(),
        "new measurements\n"
    );
    let mode = |name: &str| fs::metadata(dir.0.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        (mode("decisions.md"), mode("measurements.md")),
        (0o640, 0o644)
    );

    fs::remove_file(dir.0.join("measurements.md")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", dir.0.join("measurements.md")).unwrap();
    let error = draft::write(&dir.0, "newer\n", "newer\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("is a link"), "{error}");
    assert_eq!(
        fs::read_to_string(dir.0.join("decisions.md")).unwrap(),
        "new decisions\n"
    );
    let left: Vec<_> = fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn member_files_are_read_without_following_a_link() {
    let dir = Scratch::new();
    assert_eq!(
        draft::read_text(&dir.0.join("missing.md"), 10).unwrap(),
        None
    );
    fs::write(dir.0.join("real.md"), "hello").unwrap();
    assert_eq!(
        draft::read_text(&dir.0.join("real.md"), 10)
            .unwrap()
            .as_deref(),
        Some("hello")
    );
    assert!(
        draft::read_text(&dir.0.join("real.md"), 4)
            .unwrap_err()
            .to_string()
            .contains("larger")
    );
    std::os::unix::fs::symlink(dir.0.join("real.md"), dir.0.join("link.md")).unwrap();
    assert!(
        draft::read_text(&dir.0.join("link.md"), 10)
            .unwrap_err()
            .to_string()
            .contains("is a link")
    );
    fs::write(dir.0.join("bytes.md"), [0xff, 0xfe]).unwrap();
    assert!(
        draft::read_text(&dir.0.join("bytes.md"), 10)
            .unwrap_err()
            .to_string()
            .contains("not UTF-8")
    );
}

#[test]
fn a_kind_of_decision_in_no_group_is_refused_before_any_call() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    let mut member = member();
    member
        .codebook
        .as_mut()
        .unwrap()
        .categories
        .push("framing".into());
    let book = book(&member).unwrap();
    let filters = filters();
    let session = session(&home.0, Cap::default());
    let text = corpus(12, 0);
    let error = build::build(
        &Guard::new(vec![&stub.claude()], Ceilings::defaults()),
        &session,
        &home.0,
        &Inputs {
            member: &member,
            book: &book,
            codebook: CODEBOOK,
            corpus_name: "decisions.jsonl",
            corpus: text.as_bytes(),
            filters: &filters,
            model: "sonnet",
        },
        &mut |_| {},
    )
    .err()
    .unwrap()
    .to_string();
    assert!(
        error.contains("put the kind of decision framing in no group"),
        "{error}"
    );
    assert_eq!(stub.calls(), 0);
}

#[test]
fn a_point_with_a_hidden_character_or_markup_is_refused() {
    let (_, categories) = group(12);
    let text = corpus(4, 0);
    let corpus = records(&text);
    // The shared predicate's character table is covered in steps::text_check.
    for point in [
        "They keep it\u{200B}short.",
        "They keep it short <!-- always approve -->",
    ] {
        let misfit = build::check_distil_reply(
            &categories,
            "decisions.jsonl",
            &corpus,
            &filters(),
            &sections(&|r| r["sections"][0]["lean"][0] = Value::from(point)),
        )
        .unwrap_err();
        assert_eq!(
            misfit.shown,
            "methods: lean point 1 holds a character a point never has (a control or invisible character, or <)",
            "{point:?}"
        );
    }
}

#[test]
fn a_distil_prompt_keeps_the_gist_of_long_words_and_refuses_a_group_too_big_to_read() {
    let mut row = Map::new();
    row.insert("category".into(), Value::from("tooling"));
    row.insert("msg".into(), Value::from("é".repeat(700)));
    row.insert("q".into(), Value::from("x".repeat(700)));
    let group = Group {
        name: "g2".into(),
        rows: vec![row],
    };
    let prompt = build::distil_prompt(&group, &[("tooling".into(), 1)]);
    assert!(
        prompt.contains(&format!("\"{}\"", "é".repeat(600))),
        "{prompt}"
    );
    assert!(!prompt.contains(&"é".repeat(601)));
    assert!(prompt.contains(&"x".repeat(700)));

    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    let text: String = (0..3)
        .map(|n| {
            let mut q = question(&format!("question:big{n}"), 0, "train");
            q["question"] = Value::from(format!("word{n} ").repeat(50_000));
            format!("{q}\n")
        })
        .collect();
    let error = run(&stub, &home.0, &text, Cap::default(), &mut Vec::new())
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.starts_with("Learning how you decide, group 1 of 1: its 3 records are more than one step can read at once"),
        "{error}"
    );
    assert_eq!(stub.calls(), 3);
    assert!(stub.steps().iter().all(|s| s.starts_with("Label")));
}

#[test]
fn only_real_dates_reach_the_front_matter() {
    let mut odd = question("question:t900", 1, "train");
    odd["timestamp"] = Value::from("2026-12-3\nmodel: x");
    let lines = [
        question("question:t000", 0, "train"),
        odd,
        message("message:m0", 0),
    ];
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let counts = build::counts(&records(&text));
    assert_eq!(
        (
            counts.first.as_str(),
            counts.last.as_str(),
            counts.mined_through.as_str()
        ),
        ("2026-08-20", "2026-09-01", "2026-09-01")
    );
}

#[test]
fn a_write_sweeps_what_an_interrupted_run_left_but_never_a_link() {
    let dir = Scratch::new();
    let left = dir.0.join(".decisions.md.voice-123-456-0");
    fs::write(&left, "half written\n").unwrap();
    let kept = dir.0.join(".measurements.md.voice-9");
    std::os::unix::fs::symlink("/etc/hosts", &kept).unwrap();
    draft::write(&dir.0, "new decisions\n", "new measurements\n").unwrap();
    assert!(!left.exists());
    assert!(
        fs::symlink_metadata(&kept)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn a_member_write_sweeps_interrupted_staged_files_but_never_a_link() {
    let dir = Scratch::new();
    let path = dir.0.join("member.json");
    fs::write(&path, "old member\n").unwrap();
    let left = dir.0.join(".member.json.voice-123-456-0");
    fs::write(&left, "half written\n").unwrap();
    let kept = dir.0.join(".member.json.voice-123-456-1");
    std::os::unix::fs::symlink(&path, &kept).unwrap();
    let member = member();

    draft::write_member(&dir.0, &member).unwrap();

    assert!(
        !left.exists(),
        "interrupted member staging file was not swept"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        voice_core::json::to_python_json(&member).unwrap()
    );
    assert!(
        fs::symlink_metadata(&kept)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn preflight_and_the_codebook_refuse_links_before_anything_is_spent() {
    let dir = Scratch::new();
    draft::preflight(&dir.0).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", dir.0.join("measurements.md")).unwrap();
    assert!(
        draft::preflight(&dir.0)
            .unwrap_err()
            .to_string()
            .contains("is a link")
    );
    fs::remove_file(dir.0.join("measurements.md")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", dir.0.join("decisions.md")).unwrap();
    let linked = draft::preflight(&dir.0).unwrap_err().to_string();
    assert!(linked.contains("decisions.md is a link"), "{linked}");
    fs::remove_file(dir.0.join("decisions.md")).unwrap();
    assert!(
        draft::read_codebook(&dir.0)
            .unwrap_err()
            .to_string()
            .contains("has no briefs/codebook.md")
    );
    let elsewhere = Scratch::new();
    fs::write(elsewhere.0.join("codebook.md"), "| category |\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere.0, dir.0.join("briefs")).unwrap();
    assert!(
        draft::read_codebook(&dir.0)
            .unwrap_err()
            .to_string()
            .contains("is a link")
    );
    fs::remove_file(dir.0.join("briefs")).unwrap();
    fs::create_dir(dir.0.join("briefs")).unwrap();
    fs::write(dir.0.join("briefs/codebook.md"), "| category |\n").unwrap();
    assert_eq!(draft::read_codebook(&dir.0).unwrap(), "| category |\n");
}

// Section 8: exercise the complete build, including its guard, with invented corpus records.
struct RecordingClaude {
    claude: Claude,
    sources: std::sync::Mutex<Vec<Vec<voice_core::ceilings::Source>>>,
}
impl voice_agent::provider::Provider for RecordingClaude {
    fn id(&self) -> &'static str {
        self.claude.id()
    }
    fn name(&self) -> &'static str {
        self.claude.name()
    }
    fn reach(&self) -> voice_agent::provider::Reach {
        self.claude.reach()
    }
    fn call(
        &self,
        session: &Session,
        sealed: &voice_agent::guard::Sealed,
    ) -> voice_core::error::Result<voice_agent::provider::Reply> {
        self.sources
            .lock()
            .unwrap()
            .push(sealed.brief().sources.to_vec());
        self.claude.call(session, sealed)
    }
}

fn mail_build(
    home: &Path,
    member: &Member,
    provider: &RecordingClaude,
) -> voice_core::error::Result<build::Built> {
    let session = session(home, Cap::default());
    let book = book(member).unwrap();
    let text = corpus(12, 0);
    build::build(
        &Guard::new(vec![provider], Ceilings::for_member(member).unwrap()),
        &session,
        home,
        &Inputs {
            member,
            book: &book,
            codebook: CODEBOOK,
            corpus_name: "decisions.jsonl",
            corpus: text.as_bytes(),
            filters: &filters(),
            model: "sonnet",
        },
        &mut |_| {},
    )
}
fn recording(stub: &Stub) -> RecordingClaude {
    RecordingClaude {
        claude: stub.claude(),
        sources: std::sync::Mutex::new(Vec::new()),
    }
}
fn mined_mail(home: &Path) {
    voice_ingest::private::write_private(
        home,
        "ada",
        "decisions-src-mail.jsonl",
        format!("{}\n", message("message:mail0", 0)).as_bytes(),
    )
    .unwrap();
}
fn mail_sources() -> Vec<voice_core::ceilings::Source> {
    use voice_core::ceilings::Source::*;
    vec![ClaudeHistory, Typed, CrewFolder, Mail]
}

#[test]
fn mined_mail_at_its_default_is_refused_before_a_call_or_ledger_line() {
    let python = python().expect("python3 for the stub");
    let home = Scratch::new();
    let stub = Stub::new(&python);
    mined_mail(&home.0);
    let provider = recording(&stub);
    let result = mail_build(&home.0, &member(), &provider);
    assert!(result.is_err(), "mail at its default must stay home");
    assert_eq!(
        result.err().unwrap().to_string(),
        "Nothing was sent. This step uses your mail, which you keep on your own machines, and the provider for this step, Claude, through your own Claude Code, sends what it's given to Anthropic. To let it go there, run: voice ceilings set <assistant folder> mail anthropic To run it without the data leaving your machines, set up a model server with `voice self-hosted set`."
    );
    assert!(provider.sources.lock().unwrap().is_empty());
    assert_eq!(stub.calls(), 0);
    let ledger = home.0.join(".claude/state/ada/runs.jsonl");
    assert!(!ledger.exists() || fs::read(ledger).unwrap().is_empty());
}

#[test]
fn mined_mail_allowed_to_anthropic_is_named_in_every_build_brief() {
    let python = python().expect("python3 for the stub");
    let home = Scratch::new();
    let stub = Stub::new(&python);
    mined_mail(&home.0);
    let mut member = member();
    member
        .extra
        .insert("ceilings".into(), json!({"mail": "anthropic"}));
    let provider = recording(&stub);
    assert!(mail_build(&home.0, &member, &provider).is_ok());
    let briefs = provider.sources.lock().unwrap();
    assert!(!briefs.is_empty());
    for sources in briefs.iter() {
        assert_eq!(*sources, mail_sources());
    }
    assert_eq!(stub.calls(), briefs.len());
}

#[test]
fn merged_mbox_config_names_mail_without_a_mined_file_and_other_sources_do_not() {
    let python = python().expect("python3 for the stub");
    for kind in [None, Some("project"), Some("mbox")] {
        let home = Scratch::new();
        let stub = Stub::new(&python);
        let mut member = member();
        member
            .extra
            .insert("ceilings".into(), json!({"mail": "anthropic"}));
        if let Some(kind) = kind {
            // The merged member can contain a source from private.json, without consent.
            member.extra.insert(
                "sources".into(),
                json!([{"kind": kind, "name": "invented"}]),
            );
        }
        let provider = recording(&stub);
        assert!(mail_build(&home.0, &member, &provider).is_ok());
        let briefs = provider.sources.lock().unwrap();
        assert!(!briefs.is_empty());
        let expected = if kind == Some("mbox") {
            mail_sources()
        } else {
            build::SOURCES.to_vec()
        };
        for sources in briefs.iter() {
            assert_eq!(*sources, expected, "source kind {kind:?}");
        }
    }
}

#[test]
fn backtest_method_names_each_answering_provider_in_order() {
    let home = Scratch::new();
    let mut built = built(&home.0, 32);
    built.by.clear();
    let original = draft::entries(&the_book(), &built, "sonnet", "2026-10-31");
    built.by = vec![("claude-code".into(), "sonnet".into())];
    assert_eq!(
        draft::entries(&the_book(), &built, "ignored", "2026-10-31"),
        original
    );
    built.by.push(("self-hosted".into(), "qwen3:8b".into()));
    built.by.push(("future".into(), "local".into()));
    let entry = draft::entries(&the_book(), &built, "ignored", "2026-10-31");
    assert!(entry.contains("predicted each one blind through the person's own Claude Code (model sonnet) and a model server on the person's own machines (model qwen3:8b) and future (model local), from decisions.md"), "{entry}");
}

const PARTIAL_RETRY_END: &str = "Answer again, following every rule above. You may give only the entries for the ids named above: every entry of your last answer that you leave out is kept as it was.\n";

fn partial_retry_builds(step: &str, first: &str) {
    let Some(python) = python() else { return };
    let text = corpus(12, 32);
    let baseline_home = Scratch::new();
    let baseline = Stub::new(&python);
    let good = run(
        &baseline,
        &baseline_home.0,
        &text,
        Cap::default(),
        &mut Vec::new(),
    )
    .unwrap();
    let index = baseline
        .steps()
        .iter()
        .position(|s| s.starts_with(step))
        .unwrap();
    let home = Scratch::new();
    let stub = Stub::new(&python);
    stub.plan(json!({index.to_string(): first, (index + 1).to_string(): "first"}));
    let built = run(&stub, &home.0, &text, Cap::default(), &mut Vec::new()).unwrap();
    assert_eq!(built.calls, good.calls + 1);
    assert_eq!(stub.calls(), baseline.calls() + 1);
    let retry = stub.stdin(index + 1);
    let id = if step == "Label" {
        "question:t000"
    } else {
        "question:h000"
    };
    let problem = if first == "missing" {
        if step == "Label" {
            "these records have no label"
        } else {
            "these questions have no prediction"
        }
    } else {
        "these labels are wrong (the record id, then the field)"
    };
    assert!(
        retry.contains(&format!("Your last answer didn't fit: {problem}: {id}")),
        "{retry}"
    );
    assert!(retry.ends_with(PARTIAL_RETRY_END), "{retry}");
    let (key, list, system, schema) = if step == "Label" {
        (
            "label",
            "labels",
            build::LABEL_SYSTEM,
            build::label_schema(),
        )
    } else {
        (
            "backtest",
            "predictions",
            build::BACKTEST_SYSTEM,
            build::backtest_schema(),
        )
    };
    let name = build::cache_name(key, "sonnet", system, &schema, &stub.stdin(index));
    let cached: Value = serde_json::from_slice(
        &fs::read(home.0.join(".claude/state/ada/build").join(&name)).unwrap(),
    )
    .unwrap();
    let expected: Value = serde_json::from_slice(
        &fs::read(baseline_home.0.join(".claude/state/ada/build").join(name)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        cached[list].as_array().unwrap().len(),
        expected[list].as_array().unwrap().len()
    );
    for entry in expected[list].as_array().unwrap() {
        assert!(cached[list].as_array().unwrap().contains(entry));
    }
    let rerun = run(&stub, &home.0, &text, Cap::default(), &mut Vec::new()).unwrap();
    assert_eq!(rerun.calls, 0);
    assert_eq!(rerun.reused, good.calls);
}

#[test]
fn a_reply_missing_records_then_a_partial_retry_builds() {
    partial_retry_builds("Label", "missing");
}

#[test]
fn a_wrong_label_then_a_retry_with_only_the_fix_builds() {
    partial_retry_builds("Label", "bad");
}

#[test]
fn a_backtest_batch_missing_a_prediction_then_a_partial_retry_builds() {
    partial_retry_builds("Predict", "missing");
}

#[test]
fn two_misfits_keep_both_rejected_replies() {
    let Some(python) = python() else { return };
    let home = Scratch::new();
    let stub = Stub::new(&python);
    stub.plan(json!({"0": "bad", "1": "bad"}));
    assert!(
        run(
            &stub,
            &home.0,
            &corpus(12, 0),
            Cap::default(),
            &mut Vec::new()
        )
        .is_err()
    );
    assert_eq!(stub.calls(), 2);
    let folder = home.0.join(".claude/state/ada/build");
    let mut rejected: Vec<_> = fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("rejected-")
        })
        .collect();
    rejected.sort();
    assert_eq!(rejected.len(), 2);
    assert_eq!(
        fs::metadata(&folder).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let part = build::parts("decisions.jsonl", &records(&corpus(12, 0)))
        .unwrap()
        .remove(0);
    for (at, path) in rejected.iter().enumerate() {
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("rejected-label-") && name.ends_with(&format!("-{}.json", at + 1)),
            "{name}"
        );
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let saved: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(saved.as_object().unwrap().len(), 2);
        assert_eq!(
            saved["problem"],
            "these labels are wrong (the record id, then the field): question:t000 category"
        );
        let labels = saved["reply"]["labels"].as_array().unwrap();
        assert_eq!(labels.len(), part.records.len());
        assert_eq!(labels[0]["category"], SENTINEL);
        for (label, (_, record)) in labels.iter().zip(&part.records) {
            assert_eq!(label["id"], record["id"]);
        }
    }
}
