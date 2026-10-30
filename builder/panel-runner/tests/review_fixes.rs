//! Public-API regressions for forwarded posts and saved member names.
#![cfg(unix)]

use persona_builder::crew::{Decisions, NewMember, new_member_embedded};
use persona_builder_agent::ledger::{Cap, Session};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use voice_panel_runner::{Clock, Convene, Resume, Turn, Turns, convene, detail, resume, status};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "voice-review-fixes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let home = Self(path);
        for name in ["Pebble", "Quill"] {
            let slug = name.to_lowercase();
            let dir = home.0.join(".claude/crew").join(&slug);
            new_member_embedded(
                &dir,
                &NewMember {
                    name,
                    pronouns: "she/her",
                    principal: "Rowan Vale",
                    principal_pronouns: "they/them",
                    repo_shown: "~/.claude",
                    date: "2026-10-30",
                    decisions: Decisions::Cli("persona-builder"),
                },
            )
            .unwrap();
            fs::write(dir.join("decisions.md"), "Prefer small trials.").unwrap();
            let member = dir.join("member.json");
            let mut value = read(&member);
            value["filters"]["personal"] = json!(r"Rowan Vale|\borchidhouse\b");
            fs::write(member, serde_json::to_vec(&value).unwrap()).unwrap();
            drop(Session::open(&home.0, &slug, Cap::default()).unwrap());
        }
        home
    }

    fn start(&self, turns: &Script) -> String {
        let ran = convene(
            &self.0,
            &Convene {
                question: "Which small trial should we try?".into(),
                background: String::new(),
                kind: "list:1".into(),
                seats: vec!["pebble".into(), "quill".into()],
                time_box: Some("deadlock".into()),
            },
            &|_, _| true,
            turns,
            &FakeClock,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(ran.status, "closed");
        ran.id
    }

    fn room(&self, id: &str) -> PathBuf {
        self.0.join(".claude/voice/panels").join(id)
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

struct FakeClock;
impl Clock for FakeClock {
    fn now_iso(&self) -> String {
        "2026-10-30T12:00:00Z".into()
    }
    fn seconds(&self) -> f64 {
        0.0
    }
}

fn post(prose: &str) -> String {
    format!(
        "{prose}\n```deliberation\nposition: \"propose\"\nitems: [\"A small trial\"]\nobjections: \"\"\n```"
    )
}
struct Script {
    first: String,
    prompts: Mutex<Vec<String>>,
}
impl Script {
    fn new(first: String) -> Self {
        Self {
            first,
            prompts: Mutex::new(Vec::new()),
        }
    }
}
impl Turns for Script {
    fn turn(&self, seat: &str, _: &str, prompt: &str) -> Turn {
        self.prompts.lock().unwrap().push(prompt.into());
        Turn {
            post: Some(if seat == "pebble" && prompt.starts_with("Round 1.") {
                self.first.clone()
            } else {
                post("A modest trial.")
            }),
            ..Turn::default()
        }
    }
}

fn withheld(prose: &str, category: &str, forbidden: &str) {
    let home = Home::new();
    let first = post(prose);
    // The leak must be the only reason to withhold this otherwise valid post.
    assert!(
        voice_panels::parse_block(
            &first,
            voice_panels::Kind::List,
            Some(1),
            1,
            "pebble",
            &["pebble".into(), "quill".into()],
            &Default::default(),
        )
        .is_some()
    );
    let turns = Script::new(first.clone());
    let id = home.start(&turns);
    let panel = detail(&home.0, &id).unwrap();
    assert_eq!(
        panel.rounds[0].withheld.get("pebble"),
        Some(&vec![category.to_owned()]),
        "round-1 post must be withheld as {category}"
    );
    let room = home.room(&id);
    assert_eq!(read(&room.join("withheld/r1-pebble.json"))["text"], first);
    let transcript = fs::read_to_string(room.join("transcript.jsonl")).unwrap();
    let entry: Value = transcript
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|entry| entry["round"] == 1 && entry["seat"] == "pebble")
        .unwrap();
    assert_eq!(entry["text"], "");
    assert!(entry["block"].is_null());
    let prompts = turns.prompts.lock().unwrap();
    let later: Vec<_> = prompts
        .iter()
        .filter(|prompt| !prompt.starts_with("Round 1."))
        .collect();
    assert!(!later.is_empty());
    for prompt in later {
        assert!(!prompt.contains(prose));
        assert!(!prompt.contains(forbidden));
    }
}

#[test]
fn forwarded_control_path_is_withheld() {
    for control in ['\u{1}', '\u{7f}', '\u{80}', '\u{9f}'] {
        withheld(
            &format!("See{control}/Users/rowan/notes/x.md"),
            "path",
            "/Users/rowan/notes/x.md",
        );
    }
}

#[test]
fn forwarded_control_email_is_withheld() {
    for control in ['\u{1}', '\u{7f}', '\u{80}', '\u{9f}'] {
        withheld(
            &format!("rowan{control}@{control}example.com"),
            "email",
            "rowan @ example.com",
        );
    }
}

#[test]
fn forwarded_control_private_phrase_is_withheld() {
    for control in ['\u{1}', '\u{7f}', '\u{80}', '\u{9f}'] {
        withheld(&format!("Rowan{control}Vale"), "private", "Rowan Vale");
    }
}

#[test]
fn forwarded_truncation_private_word_is_withheld() {
    // The Unicode prefix counts characters, not bytes. Only the extra letter is cut.
    let prose = format!("{} orchidhousex", "é".repeat(7988));
    assert_eq!(prose.chars().count(), 8001);
    withheld(&prose, "private", "orchidhouse…");
}

#[test]
fn clean_control_post_is_still_shared() {
    let home = Home::new();
    let turns = Script::new(post("Try\u{1}a small trial."));
    let id = home.start(&turns);
    assert!(detail(&home.0, &id).unwrap().rounds[0].withheld.is_empty());
    let prompts = turns.prompts.lock().unwrap();
    let second: Vec<_> = prompts
        .iter()
        .filter(|p| p.starts_with("Round 2."))
        .collect();
    assert_eq!(second.len(), 2);
    for prompt in second {
        assert!(prompt.contains("Try a small trial."));
        assert!(!prompt.contains('\u{1}'));
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    let mut saved = BTreeMap::new();
    fn visit(root: &Path, path: &Path, saved: &mut BTreeMap<PathBuf, (u32, Vec<u8>)>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let bytes = if metadata.is_file() {
            fs::read(path).unwrap()
        } else {
            Vec::new()
        };
        saved.insert(
            path.strip_prefix(root).unwrap().to_owned(),
            (metadata.permissions().mode(), bytes),
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), saved);
            }
        }
    }
    visit(root, root, &mut saved);
    saved
}

fn matched_invalid_name(name: &str) {
    let home = Home::new();
    let turns = Script::new(post("A modest trial."));
    let id = home.start(&turns);
    let room = home.room(&id);
    for (file, field) in [("convene.json", "seats"), ("state.json", "crew")] {
        let path = room.join(file);
        let mut value = read(&path);
        value[field][0] = json!(name);
        fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    }
    assert_eq!(
        read(&room.join("convene.json"))["seats"],
        read(&room.join("state.json"))["crew"]
    );
    let before = snapshot(&home.0);
    assert_eq!(
        status(&home.0, &id).unwrap_err(),
        voice_panel_runner::text::READ_STATE
    );
    assert_eq!(snapshot(&home.0), before, "status changed files");
    assert_eq!(
        detail(&home.0, &id).unwrap_err(),
        voice_panel_runner::text::READ_STATE
    );
    assert_eq!(snapshot(&home.0), before, "detail changed files");
    let unused = Script::new(post("A modest trial."));
    assert_eq!(
        resume(
            &home.0,
            &Resume {
                id,
                steer: Some("Try a smaller trial.".into()),
                time_box: None
            },
            &|_, _| true,
            &unused,
            &FakeClock,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap_err(),
        voice_panel_runner::text::READ_STATE
    );
    assert_eq!(snapshot(&home.0), before, "resume changed files");
    assert!(unused.prompts.lock().unwrap().is_empty());
}

#[test]
fn matched_readback_name_with_space_is_refused_without_writes() {
    matched_invalid_name("a b");
}

#[test]
fn matched_readback_name_with_backtick_is_refused_without_writes() {
    matched_invalid_name("peb`ble");
}

#[test]
fn text_guard_checks_forwarded_controls() {
    for (value, category) in [
        ("See\u{1}/Users/rowan/notes/x.md", "path"),
        ("rowan\u{1}@\u{1}example.com", "email"),
    ] {
        assert_eq!(voice_panel_runner::guard::text_findings(value), [category]);
    }
}

#[test]
fn decoded_block_values_check_forwarded_forms() {
    use persona_builder_ingest::filters::Filters;
    use voice_panel_runner::guard::{post_findings, post_findings_refs};
    use voice_panels::{Block, Position};

    let member = persona_builder::member::read_member(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template/member.json"),
    )
    .unwrap();
    let mut config = member.filters;
    config.personal = "Rowan Vale".into();
    config.secret = "copper key".into();
    let filters = [Filters::compile(&config).unwrap()];
    for (value, category) in [
        ("See\u{1}/Users/rowan/notes/x.md", "path"),
        ("rowan\u{1}@\u{1}example.com", "email"),
        ("Rowan\u{1}Vale", "private"),
        ("copper\u{1}key", "secret"),
    ] {
        for field in ["items", "draft", "agree_with", "objections"] {
            let mut block = Block {
                position: Position::Propose,
                items: Vec::new(),
                draft: None,
                draft_id: None,
                agree_with: None,
                ratings: BTreeMap::new(),
                objections: String::new(),
            };
            match field {
                "items" => block.items.push(value.into()),
                "draft" => block.draft = Some(value.into()),
                "agree_with" => block.agree_with = Some(value.into()),
                _ => block.objections = value.into(),
            }
            assert_eq!(
                post_findings("A modest trial.", Some(&block), &filters),
                [category],
                "{field}"
            );
            assert_eq!(
                post_findings_refs("A modest trial.", Some(&block), &[&filters[0]]),
                [category],
                "{field}"
            );
        }
    }
}

#[test]
fn reopened_result_is_scrubbed_before_fencing() {
    let home = Home::new();
    let id = home.start(&Script::new(post("A modest trial.")));
    fs::write(
        home.room(&id).join("result.md"),
        "Earlier\u{e0041}\u{ad}\u{fe0f} proposal.",
    )
    .unwrap();
    let turns = Script::new(post("A modest trial."));
    resume(
        &home.0,
        &Resume {
            id,
            steer: Some("Try a smaller trial.".into()),
            time_box: None,
        },
        &|_, _| true,
        &turns,
        &FakeClock,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    let prompts = turns.prompts.lock().unwrap();
    assert!(!prompts.is_empty());
    for prompt in prompts.iter() {
        let section = prompt.split_once("where you left off\n").unwrap().1;
        let section = section.split_once("\n```").unwrap().0;
        assert!(section.contains("Earlier"));
        for hidden in ['\u{e0041}', '\u{ad}', '\u{fe0f}'] {
            assert!(
                !section.contains(hidden),
                "reopened result retained {hidden:?}"
            );
        }
        assert!(section.contains("Earlier proposal."));
    }
}

fn text_finding_is_withheld(value: &str, category: &str) {
    assert_eq!(voice_panel_runner::guard::text_findings(value), [category]);
    withheld(value, category, value);
}

#[test]
fn fullwidth_at_localhost_is_withheld() {
    text_finding_is_withheld("me＠localhost", "email");
}

#[test]
fn single_segment_home_path_is_withheld() {
    text_finding_is_withheld("~/notes.txt", "path");
}

#[test]
fn bare_home_prefix_at_boundary_is_withheld() {
    for value in ["~/", "see:~/", "x,~/", "(~/", " ~/", "é,~/"] {
        text_finding_is_withheld(value, "path");
    }
}

#[test]
fn comma_user_home_path_is_withheld() {
    text_finding_is_withheld("x,~rowan/y", "path");
}

#[test]
fn colon_user_home_path_is_withheld() {
    text_finding_is_withheld("see:~rowan/x", "path");
}

#[test]
fn ordinary_prose_and_bare_spelled_words_are_not_flagged() {
    for value in [
        "meet at noon",
        "look at the dot plot",
        "a, b: c",
        "rowan at example dot org",
    ] {
        assert!(voice_panel_runner::guard::text_findings(value).is_empty());
    }
}

#[test]
fn unknown_long_seat_has_a_short_error() {
    let home = Home::new();
    let bin = home.0.join("bin");
    fs::create_dir(&bin).unwrap();
    let program = bin.join("claude");
    fs::write(
        &program,
        "#!/bin/sh\nif [ \"$1\" = '--version' ]; then printf '2.1.292\\n'; exit 0; fi\nprintf '{\"loggedIn\":true,\"authMethod\":\"claude.ai\"}\\n'\n",
    )
    .unwrap();
    fs::set_permissions(program, fs::Permissions::from_mode(0o755)).unwrap();
    let seat = "q".repeat(500);
    let error = voice_panel_runner::turns::ClaudeTurns::new(
        &home.0,
        Some(bin.as_os_str()),
        &[],
        std::slice::from_ref(&seat),
    )
    .err()
    .expect("unknown seat must be refused");
    assert!(
        !error.contains(&seat),
        "error echoed the entire unknown seat"
    );
    assert!(error.chars().count() <= 160, "seat error is too long");
    assert!(
        error.starts_with(&"q".repeat(64)),
        "must reach the seat error"
    );
}

#[test]
fn invisible_boundary_question_is_refused_without_writes() {
    let home = Home::new();
    let before = snapshot(&home.0);
    let turns = Script::new(post("A modest trial."));
    let result = convene(
        &home.0,
        &Convene {
            question: "Should x\u{200b}/Users/rowan/ go?".into(),
            background: String::new(),
            kind: "list:1".into(),
            seats: vec!["pebble".into(), "quill".into()],
            time_box: Some("deadlock".into()),
        },
        &|_, _| true,
        &turns,
        &FakeClock,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(result.is_err(), "question path must be refused");
    assert_eq!(snapshot(&home.0), before, "refused question wrote files");
    assert!(turns.prompts.lock().unwrap().is_empty());
}

#[test]
fn invisible_boundary_steer_is_refused_without_writes() {
    let home = Home::new();
    let id = home.start(&Script::new(post("A modest trial.")));
    let before = snapshot(&home.0);
    let turns = Script::new(post("A modest trial."));
    let result = resume(
        &home.0,
        &Resume {
            id,
            steer: Some("see\u{200b}/Users/rowan/x".into()),
            time_box: None,
        },
        &|_, _| true,
        &turns,
        &FakeClock,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(result.is_err(), "steer path must be refused");
    assert_eq!(snapshot(&home.0), before, "refused steer wrote files");
    assert!(turns.prompts.lock().unwrap().is_empty());
}

#[test]
fn saved_inputs_are_scrubbed_before_cleaning() {
    let home = Home::new();
    let turns = Script::new(post("A modest trial."));
    let ran = convene(
        &home.0,
        &Convene {
            question: "Which tri\u{200b}al?".into(),
            background: "A modest tri\u{200b}al.".into(),
            kind: "list:1".into(),
            seats: vec!["pebble".into(), "quill".into()],
            time_box: Some("deadlock".into()),
        },
        &|_, _| true,
        &turns,
        &FakeClock,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    let metadata = read(&home.room(&ran.id).join("convene.json"));
    assert_eq!(metadata["question"], "Which trial?");
    assert_eq!(metadata["background"], "A modest trial.");
    resume(
        &home.0,
        &Resume {
            id: ran.id.clone(),
            steer: Some("Try a tri\u{200b}al.".into()),
            time_box: None,
        },
        &|_, _| true,
        &turns,
        &FakeClock,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap();
    let transcript = fs::read_to_string(home.room(&ran.id).join("transcript.jsonl")).unwrap();
    let steer = transcript
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|entry| entry["type"] == "steer")
        .unwrap();
    assert_eq!(steer["text"], "Try a trial.");
    for prompt in turns.prompts.lock().unwrap().iter() {
        assert!(!prompt.contains('\u{200b}'));
        assert!(!prompt.contains("tri al"));
    }
}

macro_rules! review_guard_case {
    ($name:ident, $value:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_eq!(voice_panel_runner::guard::text_findings($value), $expected);
        }
    };
}

review_guard_case!(small_at_domain_is_email, "me﹫example.com", ["email"]);
review_guard_case!(small_at_localhost_is_email, "me﹫localhost", ["email"]);
review_guard_case!(small_at_spaced_is_email, "a ﹫ b . com", ["email"]);
review_guard_case!(double_tilde_home_is_path, "~~/notes.txt", ["path"]);
review_guard_case!(double_tilde_user_is_path, "~~rowan/x", ["path"]);
review_guard_case!(word_tilde_home_is_path, "foo~/notes/x", ["path"]);
review_guard_case!(score_tilde_is_prose, "Score:~7/10", Vec::<&str>::new());
review_guard_case!(approx_tilde_is_prose, "approx.~20/day", Vec::<&str>::new());
review_guard_case!(
    rated_tilde_is_prose,
    "Rated ~8/10 by most people.",
    Vec::<&str>::new()
);
review_guard_case!(fraction_tilde_is_prose, "~3/4", Vec::<&str>::new());
review_guard_case!(
    cost_tilde_is_prose,
    "It costs ~$5 a month.",
    Vec::<&str>::new()
);
review_guard_case!(ratio_is_prose, "Ratio 3:2, roughly.", Vec::<&str>::new());
review_guard_case!(letter_tilde_user_stays_path, "x,~ten/twelve", ["path"]);
review_guard_case!(
    spaced_at_sentence_stays_email,
    "Meet @ noon. See you",
    ["email"]
);
review_guard_case!(plain_user_home_stays_path, "~rowan/x", ["path"]);
