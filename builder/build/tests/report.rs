//! The plain report uses only counts, checked test results and the codebook's areas.
use persona_builder_build::{
    build::{Backtest, Built, Counts},
    report::{self, Area, Level, REPORT, REPORT_MOST, Report, SCHEMA, Strengthen, Test},
};
use persona_builder_ingest::private;
use persona_builder_steps::{
    book::Book,
    labels::Row,
    score::{Cell, Score},
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

fn book() -> Book {
    Book {
        categories: ["scope", "methods", "tooling", "other"]
            .map(String::from)
            .into(),
        domains: vec![],
        escalates: vec![],
        signals: vec![],
        groups: vec![],
    }
}
fn built() -> Built {
    Built {
        sections: String::new(),
        tally: vec![],
        backtest: None,
        held: 10,
        counts: Counts::default(),
        calls: 0,
        reused: 0,
    }
}
fn row(questions: usize, messages: usize) -> Row {
    Row {
        questions,
        messages,
        ..Row::default()
    }
}
fn tested(built: &mut Built, live: &[&str]) {
    built.held = 32;
    built.backtest = Some(Backtest {
        first: String::new(),
        last: String::new(),
        score: Score {
            overall: Cell { n: 32, hits: 30 },
            baseline: 0,
            led: Cell::default(),
            took_member: 0,
            tiers: [None; 3],
            categories: live
                .iter()
                .map(|s| (s.to_string(), [Some(Cell { n: 5, hits: 5 }), None, None]))
                .collect(),
        },
    });
}
fn make(built: &Built, answered: usize) -> Report {
    report::report(&book(), built, answered, "2026-10-25")
}

#[test]
fn untested_thin_history_strengthens_every_area() {
    let mut b = built();
    b.tally = vec![("scope".into(), row(4, 4)), ("other".into(), row(1, 0))];
    let r = make(&b, 44);
    assert_eq!(r.overall, Level::Thin);
    assert_eq!(
        (r.schema.as_str(), r.built.as_str(), r.answered, r.held),
        (SCHEMA, "2026-10-25", 44, 10)
    );
    assert_eq!(r.test, None);
    assert_eq!(r.areas.len(), 2);
    for a in r.areas {
        assert_ne!(a.level, Level::Strong);
        assert_eq!(
            a.strengthen,
            [
                Strengthen::Interview,
                Strengthen::Claudeai,
                Strengthen::Time
            ]
        );
    }
}

#[test]
fn enough_history_uses_chats_and_time_without_an_interview() {
    let mut b = built();
    b.tally = vec![("scope".into(), row(5, 3))];
    let r = make(&b, 160);
    assert_eq!(r.areas.len(), 1);
    assert_eq!(r.overall, Level::GettingThere);
    assert_eq!(
        (r.areas[0].level, r.areas[0].examples),
        (Level::GettingThere, 8)
    );
    assert_eq!(
        r.areas[0].strengthen,
        [Strengthen::Claudeai, Strengthen::Time]
    );
    b.counts.claudeai = 1;
    assert_eq!(make(&b, 160).areas[0].strengthen, [Strengthen::Time]);
    b.tally[0].1 = row(4, 0);
    assert_eq!(
        make(&b, 150).areas[0].strengthen,
        [Strengthen::Interview, Strengthen::Time]
    );
}

#[test]
fn at_150_answers_enough_examples_need_chats_and_time_only() {
    let mut b = built();
    b.tally = vec![("scope".into(), row(5, 0))];
    assert_eq!(
        make(&b, 150).areas[0].strengthen,
        [Strengthen::Claudeai, Strengthen::Time]
    );
}

#[test]
fn untested_empty_tally_with_enough_history_is_getting_there() {
    assert_eq!(make(&built(), 200).overall, Level::GettingThere);
}

#[test]
fn area_levels_use_example_and_high_pick_boundaries() {
    let mut b = built();
    b.tally = vec![
        ("scope".into(), row(4, 0)),
        ("methods".into(), row(4, 1)),
        ("tooling".into(), row(1, 0)),
        ("other".into(), row(5, 0)),
    ];
    tested(&mut b, &["tooling", "other"]);
    b.backtest
        .as_mut()
        .unwrap()
        .score
        .categories
        .get_mut("other")
        .unwrap()[0]
        .as_mut()
        .unwrap()
        .hits = 4;
    let r = make(&b, 200);
    assert_eq!(r.areas.len(), 4);
    assert_eq!(
        r.areas.iter().map(|a| a.level).collect::<Vec<_>>(),
        [
            Level::Thin,
            Level::GettingThere,
            Level::Strong,
            Level::GettingThere
        ]
    );
    assert!(r.areas[2].strengthen.is_empty());
    let high = &mut b.backtest.as_mut().unwrap().score.categories;
    high.insert("scope".into(), [Some(Cell { n: 4, hits: 4 }), None, None]);
    high.insert(
        "methods".into(),
        [Some(Cell { n: 10, hits: 9 }), None, None],
    );
    let r = make(&b, 200);
    assert_eq!(r.areas[0].level, Level::Thin);
    assert_eq!(r.areas[1].level, Level::Strong);
    b.backtest = None;
    assert!(make(&b, 200).areas.iter().all(|a| a.level != Level::Strong));
}

#[test]
fn overall_is_strong_at_half_the_areas_after_enough_history() {
    let mut b = built();
    b.tally = book()
        .categories
        .into_iter()
        .map(|s| (s, row(5, 0)))
        .collect();
    tested(&mut b, &["scope", "methods"]);
    let r = make(&b, 200);
    assert_eq!(r.test, Some(Test { right: 30, of: 32 }));
    assert_eq!(r.overall, Level::Strong);
    assert_eq!(make(&b, 150).overall, Level::Strong);
    assert_eq!(make(&b, 149).overall, Level::Thin);
    b.backtest
        .as_mut()
        .unwrap()
        .score
        .categories
        .remove("methods");
    assert_eq!(make(&b, 200).overall, Level::GettingThere);
    b.backtest = None;
    b.tally.clear();
    assert_eq!(make(&b, 200).overall, Level::GettingThere);
}

#[test]
fn live_categories_without_training_rows_are_appended_and_count_toward_half() {
    let mut b = built();
    b.tally = vec![("other".into(), row(5, 0)), ("extra".into(), row(5, 0))];
    tested(&mut b, &["methods", "scope"]);
    let r = make(&b, 200);
    assert_eq!(
        r.areas,
        vec![
            Area {
                area: "other".into(),
                level: Level::GettingThere,
                examples: 5,
                strengthen: vec![Strengthen::Claudeai, Strengthen::Time]
            },
            Area {
                area: "extra".into(),
                level: Level::GettingThere,
                examples: 5,
                strengthen: vec![Strengthen::Claudeai, Strengthen::Time]
            },
            Area {
                area: "scope".into(),
                level: Level::Strong,
                examples: 0,
                strengthen: vec![]
            },
            Area {
                area: "methods".into(),
                level: Level::Strong,
                examples: 0,
                strengthen: vec![]
            },
        ]
    );
    assert_eq!(r.overall, Level::Strong);
}

#[test]
fn areas_follow_the_codebook_then_the_remaining_tally_order() {
    let mut b = built();
    b.tally = ["extra-z", "other", "tooling", "extra-a", "scope"]
        .map(|s| (s.into(), row(5, 0)))
        .into();
    assert_eq!(
        make(&b, 200)
            .areas
            .iter()
            .map(|a| a.area.as_str())
            .collect::<Vec<_>>(),
        ["scope", "tooling", "other", "extra-z", "extra-a"]
    );
}

fn plain() -> Report {
    Report {
        schema: SCHEMA.into(),
        built: "2026-10-25".into(),
        overall: Level::Thin,
        answered: 44,
        held: 10,
        test: None,
        areas: vec![Area {
            area: "scope".into(),
            level: Level::Thin,
            examples: 1,
            strengthen: vec![
                Strengthen::Interview,
                Strengthen::Claudeai,
                Strengthen::Time,
            ],
        }],
    }
}

#[test]
fn render_untested_words_are_exact_and_interview_is_only_for_thin_history() {
    let mut r = plain();
    assert_eq!(
        report::render(&r),
        [
            "How well your assistant knows you: Thin.",
            "Your history holds 44 answered questions. It takes about 150 to know you well.",
            "Only 10 questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Until it can be tested, it decides nothing on its own.",
            "The interview fills out a thin history: persona-builder interview draft",
            "By kind of decision:",
            "- scope: Thin, 1 example so far. To strengthen it: answer the interview (persona-builder interview draft); add your claude.ai chats (persona-builder claudeai); keep using Claude, since it learns from every answer.",
        ]
    );
    r.answered = 150;
    r.overall = Level::GettingThere;
    r.areas.clear();
    assert_eq!(
        report::render(&r),
        [
            "How well your assistant knows you: Getting there.",
            "There's enough history to learn from, but too little to test yet.",
            "Only 10 questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Until it can be tested, it decides nothing on its own.",
            "By kind of decision: none yet.",
        ]
    );
}

#[test]
fn render_tested_words_are_exact() {
    let r = Report {
        overall: Level::Strong,
        answered: 200,
        held: 32,
        test: Some(Test { right: 30, of: 32 }),
        areas: vec![
            Area {
                area: "scope".into(),
                level: Level::Strong,
                examples: 5,
                strengthen: vec![],
            },
            Area {
                area: "methods".into(),
                level: Level::GettingThere,
                examples: 8,
                strengthen: vec![Strengthen::Time],
            },
            Area {
                area: "other".into(),
                level: Level::Thin,
                examples: 2,
                strengthen: vec![Strengthen::Time],
            },
        ],
        ..plain()
    };
    assert_eq!(
        report::render(&r),
        [
            "How well your assistant knows you: Strong.",
            "It passed the test on 1 of 3 kinds of decision, and got 30 of 32 test questions right.",
            "By kind of decision:",
            "- scope: Strong.",
            "- methods: Getting there. To strengthen it: keep using Claude, since it learns from every answer.",
            "- other: Thin, 2 examples so far. To strengthen it: keep using Claude, since it learns from every answer.",
        ]
    );
    let r = Report {
        overall: Level::GettingThere,
        ..r
    };
    assert_eq!(
        report::render(&r)[0],
        "How well your assistant knows you: Getting there."
    );
}

#[test]
fn check_refuses_hidden_area_names_without_echoing_them_and_accepts_plain() {
    assert!(report::check(&plain()).is_ok());
    let mut r = plain();
    r.areas[0].area = "scope\u{1b}[2J".into();
    let result = report::check(&r);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "A kind of decision in your assistant's settings (codebook.categories) has a name that is empty, longer than 100 characters, or holds a character that can't be shown, so nothing was saved. Rename it, then try again."
    );
}

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-report-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn private_report_round_trips_with_private_permissions() {
    let home = Home::new();
    assert_eq!(report::read(&home.0, "wren").unwrap(), None);
    let r = plain();
    let path = report::write(&home.0, "wren", &r).unwrap();
    assert_eq!(report::read(&home.0, "wren").unwrap(), Some(r.clone()));
    assert_eq!(path, home.0.join(".claude/state/wren/report.json"));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let bytes = fs::read(&path).unwrap();
    let mut expected = serde_json::to_vec_pretty(&r).unwrap();
    expected.push(b'\n');
    assert_eq!(bytes, expected);
    let r = Report {
        test: Some(Test { right: 5, of: 5 }),
        overall: Level::GettingThere,
        ..r
    };
    report::write(&home.0, "wren", &r).unwrap();
    assert_eq!(report::read(&home.0, "wren").unwrap(), Some(r));
}

#[test]
fn read_refuses_changed_or_oversized_reports_without_echoing_them() {
    let home = Home::new();
    let original = serde_json::to_value(plain()).unwrap();
    let mut bad = Vec::new();
    for key in [
        "unknown",
        "schema",
        "built",
        "area",
        "area_long",
        "area_empty",
        "nested",
        "test",
    ] {
        let mut v = original.clone();
        match key {
            "unknown" => v["unknown"] = json!(1),
            "schema" => v["schema"] = json!("wrong"),
            "built" => v["built"] = json!("2026-1x-25"),
            "area" => v["areas"][0]["area"] = json!("scope\u{1b}[2J"),
            "area_long" => v["areas"][0]["area"] = json!("é".repeat(101)),
            "area_empty" => v["areas"][0]["area"] = json!(""),
            "nested" => v["areas"][0]["unknown"] = json!(1),
            "test" => v["test"] = json!({"right": 1, "of": 1, "unknown": true}),
            _ => unreachable!(),
        }
        bad.push(serde_json::to_vec(&v).unwrap());
    }
    let mut oversized = serde_json::to_vec(&original).unwrap();
    oversized.resize(REPORT_MOST + 1, b' ');
    bad.extend([b"not json".to_vec(), oversized]);
    for bytes in bad {
        private::write_private(&home.0, "wren", REPORT, &bytes).unwrap();
        let result = report::read(&home.0, "wren");
        assert!(
            result.is_err(),
            "accepted {}",
            String::from_utf8_lossy(&bytes)
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "Your last report couldn't be read, so it isn't shown. Build your assistant again to make a new one."
        );
    }
    // The limit counts bytes, while area names count characters.
    let mut v: Value = original;
    v["areas"][0]["area"] = json!("é".repeat(100));
    let mut bytes = serde_json::to_vec(&v).unwrap();
    bytes.resize(REPORT_MOST, b' ');
    private::write_private(&home.0, "wren", REPORT, &bytes).unwrap();
    assert!(report::read(&home.0, "wren").unwrap().is_some());
}
