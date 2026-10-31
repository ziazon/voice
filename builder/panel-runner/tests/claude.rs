//! Public model-call and leak-guard contracts, using only invented assistants and details.
use persona_builder::crew::{Decisions, NewMember, new_member_embedded};
use persona_builder_agent::ledger::{Cap, Session, local_day};
use persona_builder_ingest::filters::Filters;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::SystemTime,
};
use voice_panel_runner::{
    guard,
    turns::{ClaudeTurns, Turns},
};
use voice_panels::{Block, Position};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    home: PathBuf,
    bin: PathBuf,
    seats: Vec<String>,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "voice-panel-claude-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let bin = home.join("bin");
        fs::create_dir(&bin).unwrap();
        let seats = vec!["pebble".into(), "quill".into()];
        for (slug, name) in [("pebble", "Pebble"), ("quill", "Quill")] {
            let target = home.join(".claude/crew").join(slug);
            new_member_embedded(
                &target,
                &NewMember {
                    name,
                    pronouns: "she/her",
                    principal: "Rowan Vale",
                    principal_pronouns: "they/them",
                    repo_shown: "~/assistants",
                    date: "2026-10-30",
                    decisions: Decisions::Python,
                },
            )
            .unwrap();
            fs::write(
                target.join("decisions.md"),
                "Prefer small reversible experiments.\n",
            )
            .unwrap();
            drop(Session::open(&home, slug, Cap::default()).unwrap());
        }
        let program = bin.join("claude");
        fs::write(&program, r#"#!/bin/sh
here=${0%/*}
if [ "$1" = '--version' ]; then printf '2.1.292\n'; exit 0; fi
if [ "$1" = 'auth' ]; then printf '{"loggedIn":true}\n'; exit 0; fi
printf '%s\000' "$@" > "$here/argv"
/bin/cat > "$here/stdin"
printf 'called\n' >> "$here/turn-calls"
printf '%s\n' '{"type":"result","is_error":false,"structured_output":{"post":"Try the smaller experiment."},"usage":{"input_tokens":11,"output_tokens":7,"cache_creation_input_tokens":3,"cache_read_input_tokens":5},"total_cost_usd":0.125}'
"#).unwrap();
        fs::set_permissions(program, fs::Permissions::from_mode(0o755)).unwrap();
        Self { home, bin, seats }
    }
    fn runner(&self) -> Result<ClaudeTurns, String> {
        ClaudeTurns::new(&self.home, Some(self.bin.as_os_str()), &[], &self.seats)
    }
    fn ledger(&self) -> PathBuf {
        self.home.join(".claude/state/pebble/runs.jsonl")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

#[test]
fn claude_turn_has_no_tools_uses_post_schema_and_records_usage() {
    let fixture = Fixture::new();
    let result = fixture.runner();
    assert!(
        result.is_ok(),
        "Claude Code should be ready: {:?}",
        result.as_ref().err()
    );
    let turns = result.unwrap().with_stop(Arc::new(AtomicBool::new(false)));
    let turn = turns.turn(
        "pebble",
        "Answer as Pebble.",
        "Round 1. Choose a small experiment.",
    );
    assert_eq!(turn.error, None);
    assert_eq!(turn.post.as_deref(), Some("Try the smaller experiment."));
    let usage = turn.usage.expect("a successful turn reports usage");
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_creation_input_tokens,
            usage.cache_read_input_tokens
        ),
        (11, 7, 3, 5)
    );
    assert_eq!(usage.cost_usd, 0.125);
    let bytes = fs::read(fixture.bin.join("argv")).unwrap();
    let argv: Vec<&str> = bytes
        .split(|byte| *byte == 0)
        .map(|arg| std::str::from_utf8(arg).unwrap())
        .collect();
    let argument = |flag: &str| {
        argv.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| pair[1])
    };
    assert_eq!(argument("--tools"), Some(""));
    assert!(argv.contains(&"--strict-mcp-config"));
    assert_eq!(argument("--mcp-config"), Some(r#"{"mcpServers":{}}"#));
    assert!(argv.contains(&"--safe-mode"));
    assert_eq!(argument("--permission-mode"), Some("dontAsk"));
    assert_eq!(argument("--model"), Some("sonnet"));
    assert_eq!(argument("--system-prompt"), Some("Answer as Pebble."));
    for forbidden in ["Bash", "Write", "Edit", "WebFetch", "WebSearch"] {
        assert!(
            argv.iter().all(|arg| !arg.contains(forbidden)),
            "unexpected tool {forbidden}"
        );
    }
    let schema: Value =
        serde_json::from_str(argument("--json-schema").expect("post schema argument")).unwrap();
    assert_eq!(
        schema,
        json!({"type":"object","properties":{"post":{"type":"string","minLength":1,"maxLength":12000}},"required":["post"],"additionalProperties":false})
    );
    assert_eq!(
        fs::read_to_string(fixture.bin.join("stdin")).unwrap(),
        "Round 1. Choose a small experiment."
    );
    let lines: Vec<Value> = fs::read_to_string(fixture.ledger())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["step"], "panel-turn");
    assert_eq!(lines[0]["outcome"], "ok");
    for (field, expected) in [
        ("input_tokens", 11),
        ("output_tokens", 7),
        ("cache_creation_input_tokens", 3),
        ("cache_read_input_tokens", 5),
    ] {
        assert_eq!(lines[0][field], expected);
    }
    assert_eq!(lines[0]["cost_usd"], 0.125);
}

#[test]
fn spent_daily_cap_returns_exact_sentence_without_a_turn_call() {
    let fixture = Fixture::new();
    let session = Session::open_existing(&fixture.home, "pebble", Cap::default()).unwrap();
    Cap::new(1.0, 1.0)
        .unwrap()
        .save(&fixture.home, "pebble")
        .unwrap();
    drop(session);
    let day = local_day(SystemTime::now());
    let spent = json!({"at":format!("{day}T12:00:00Z"),"day":day,"agent":"claude-code","step":"panel-turn","model":"sonnet","outcome":"ok","seconds":1.0,"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"cost_usd":1.0,"cost_from":"claude"});
    let before = format!("{spent}\n");
    fs::write(fixture.ledger(), &before).unwrap();
    fs::set_permissions(fixture.ledger(), fs::Permissions::from_mode(0o600)).unwrap();
    let result = fixture.runner();
    assert!(
        result.is_ok(),
        "a spent cap is checked at the turn: {:?}",
        result.as_ref().err()
    );
    let turns = result.unwrap();
    let turn = turns.turn(
        "pebble",
        "Answer as Pebble.",
        "Round 1. Choose a small experiment.",
    );
    assert_eq!(
        turn.error.as_deref(),
        Some(
            "Today's spending limit for this assistant is reached: about $1.00 of $1.00, as Claude Code estimates it. The build can carry on tomorrow, or you can raise the limit."
        )
    );
    assert!(turn.post.is_none());
    assert!(turn.usage.is_none());
    assert!(
        !fixture.bin.join("turn-calls").exists(),
        "version and sign-in checks are allowed, a paid turn is not"
    );
    assert_eq!(fs::read_to_string(fixture.ledger()).unwrap(), before);
}

#[test]
fn scrub_deletes_the_complete_inclusive_set_and_preserves_boundary_neighbors() {
    let ranges = [
        (0x00ad, 0x00ad),
        (0x034f, 0x034f),
        (0x061c, 0x061c),
        (0x115f, 0x1160),
        (0x17b4, 0x17b5),
        (0x180b, 0x180f),
        (0x200b, 0x200f),
        (0x202a, 0x202e),
        (0x2060, 0x206f),
        (0x3164, 0x3164),
        (0xfe00, 0xfe0f),
        (0xfeff, 0xfeff),
        (0xffa0, 0xffa0),
        (0x1d173, 0x1d17a),
        (0xe0000, 0xe007f),
        (0xe0100, 0xe01ef),
    ];
    let removed: String = ranges
        .iter()
        .flat_map(|&(lo, hi)| lo..=hi)
        .map(|n| char::from_u32(n).unwrap())
        .collect();
    assert_eq!(guard::scrub(&format!("left{removed}right")), "leftright");
    let neighbors: BTreeSet<u32> = ranges
        .iter()
        .flat_map(|&(lo, hi)| [lo - 1, hi + 1])
        .filter(|n| !ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(n)))
        .collect();
    let kept: String = neighbors
        .into_iter()
        .map(|n| char::from_u32(n).unwrap())
        .collect();
    assert_eq!(guard::scrub(&kept), kept);
    assert_eq!(
        guard::scrub("A plain, useful answer."),
        "A plain, useful answer."
    );
}

#[test]
fn text_findings_are_sorted_deduplicated_and_detect_obfuscation() {
    assert_eq!(
        guard::text_findings(
            "file:///x C:\\x \\\\srv\\x ~rowan/x a @ b . com rowan(at)example(dot)org me@localhost me\u{200b}@localhost"
        ),
        vec!["email", "path"]
    );
    for value in [
        "file:///x",
        "C:\\x",
        "\\\\srv\\x",
        "~rowan/x",
        "fi\u{e0041}le:///x",
    ] {
        assert_eq!(guard::text_findings(value), vec!["path"], "{value:?}");
    }
    for value in [
        "a @ b . com",
        "rowan(at)example(dot)org",
        "me@localhost",
        "me\u{fe0f}@localhost",
        "a＠b.com",
    ] {
        assert_eq!(guard::text_findings(value), vec!["email"], "{value:?}");
    }
    assert!(guard::text_findings("Choose the smaller experiment.").is_empty());
}

fn filters() -> Vec<Filters> {
    [("amberfern", "a^"), ("a^", "violetvault")]
        .into_iter()
        .map(|(personal, secret)| {
            Filters::compile(&persona_builder::member::Filters {
                excluded_folders: "a^".into(),
                personal: personal.into(),
                secret: secret.into(),
                pasted_prefixes: Vec::new(),
                denied_paths: Vec::new(),
                extra: serde_json::Map::new(),
            })
            .unwrap()
        })
        .collect()
}
fn block() -> Block {
    Block {
        position: Position::Propose,
        items: Vec::new(),
        draft: None,
        draft_id: None,
        agree_with: None,
        ratings: BTreeMap::new(),
        objections: String::new(),
    }
}
fn decoded_field_findings(field: &str) {
    let decoded: String = serde_json::from_str(
        r#""me\u200b@localhost file:\u00ad///x amber\u200bfern violet\ufe0fvault""#,
    )
    .unwrap();
    let mut value = block();
    match field {
        "items" => value.items = vec![decoded],
        "draft" => value.draft = Some(decoded),
        "agree_with" => value.agree_with = Some(decoded),
        "objections" => value.objections = decoded,
        _ => unreachable!(),
    }
    assert_eq!(
        guard::post_findings("A harmless visible post.", Some(&value), &filters()),
        vec!["email", "path", "private", "secret"],
        "decoded {field}"
    );
}
#[test]
fn decoded_items_are_scrubbed_and_checked() {
    decoded_field_findings("items");
}
#[test]
fn decoded_draft_is_scrubbed_and_checked() {
    decoded_field_findings("draft");
}
#[test]
fn decoded_agreement_is_scrubbed_and_checked() {
    decoded_field_findings("agree_with");
}
#[test]
fn decoded_objections_are_scrubbed_and_checked() {
    decoded_field_findings("objections");
}

#[test]
fn post_filters_check_raw_and_scrubbed_text_and_allow_a_clean_control() {
    assert_eq!(
        guard::post_findings("amberfern", None, &filters()),
        vec!["private"]
    );
    assert_eq!(
        guard::post_findings("amber\u{200b}fern", None, &filters()),
        vec!["private"]
    );
    assert_eq!(
        guard::post_findings("violetvault", None, &filters()),
        vec!["secret"]
    );
    assert_eq!(
        guard::post_findings("violet\u{e0041}vault", None, &filters()),
        vec!["secret"]
    );
    let mut clean = block();
    clean.items = vec!["Try a small experiment".into()];
    clean.draft = Some("Learn before spending more".into());
    clean.agree_with = Some("pebble-r1".into());
    clean.objections = "Measure its benefit first".into();
    assert!(guard::post_findings("A useful answer.", Some(&clean), &filters()).is_empty());
}

#[test]
fn guard_input_names_sorted_categories_and_does_not_apply_member_filters() {
    assert_eq!(
        guard::guard_input("me@localhost file:///x"),
        Err("the question has: email, path".into())
    );
    assert_eq!(
        guard::guard_input("me\u{200b}@localhost"),
        Err("the question has: email".into())
    );
    assert_eq!(
        guard::guard_input("Choose the amberfern experiment."),
        Ok(())
    );
}
