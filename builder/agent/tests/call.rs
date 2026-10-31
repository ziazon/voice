mod common;

use common::*;
use persona_builder::{
    ceilings::{Ceilings, Source},
    member::Member,
};
use persona_builder_agent::guard::Guard;
use persona_builder_agent::{
    claude::MAX_PROMPT,
    ledger::local_day,
    provider::{Brief, Provider, Usage},
    text,
};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::symlink, time::Duration};

const MARKER: &str = "PLOVERMARK";

fn expected_argv(schema: &Value) -> Vec<String> {
    [
        "-p",
        "--system-prompt",
        "Label each record.",
        "--model",
        "sonnet",
        "--safe-mode",
        "--no-session-persistence",
        "--tools",
        "",
        "--output-format",
        "stream-json",
        "--verbose",
        "--json-schema",
        &schema.to_string(),
        "--setting-sources",
        "project,local",
        "--strict-mcp-config",
        "--mcp-config",
        r#"{"mcpServers":{}}"#,
        "--permission-mode",
        "dontAsk",
        "--max-budget-usd",
        "5.00",
    ]
    .map(str::to_owned)
    .to_vec()
}

#[test]
fn a_call_runs_tool_less_in_the_work_folder_with_the_prompt_on_stdin() {
    let home = Scratch::new("call-argv");
    let output = json!({"labels": [MARKER]});
    let stub = Stub::replying(&ok_reply(&output), 0);
    let session = common::session(&home.0);
    let schema = schema();
    let prompt = format!("record one\nrecord two {MARKER}\n");
    let claude = stub.claude();
    let guard = Guard::new(vec![&claude], Ceilings::defaults());
    let reply = guard
        .call(
            &session,
            &persona_builder_agent::provider::Brief {
                sources: &[Source::ClaudeHistory],
                ..call(&prompt, &schema)
            },
        )
        .unwrap();

    assert_eq!(stub.argv(0).unwrap(), expected_argv(&schema));
    assert!(!stub.argv(0).unwrap().iter().any(|a| a.contains(MARKER)));
    assert_eq!(stub.file("stdin"), prompt);
    assert_eq!(
        stub.file("cwd").trim(),
        state(&home.0).join("agent").to_str().unwrap()
    );
    assert_eq!(reply.output, output);
    assert_eq!(
        reply.usage,
        Usage {
            input_tokens: 1200,
            output_tokens: 300,
            cache_creation_input_tokens: 7,
            cache_read_input_tokens: 9
        }
    );
    assert_eq!(reply.cost_usd, 0.0123);
    assert_eq!(
        fs::read_dir(state(&home.0).join("agent")).unwrap().count(),
        0
    );

    let lines = ledger_lines(&home.0);
    assert_eq!(lines.len(), 1);
    let line = lines[0].as_object().unwrap();
    let keys: Vec<&str> = line.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "at",
            "id",
            "day",
            "agent",
            "step",
            "model",
            "outcome",
            "seconds",
            "input_tokens",
            "output_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
            "cost_usd",
            "cost_from"
        ]
    );
    assert_eq!(line["day"], local_day(fixed_time()));
    assert_eq!(line["agent"], "claude-code");
    assert_eq!(line["step"], "label");
    assert_eq!(line["model"], "sonnet");
    assert_eq!(line["outcome"], "ok");
    assert_eq!(line["input_tokens"], 1200);
    assert_eq!(line["cache_read_input_tokens"], 9);
    assert_eq!(line["cost_usd"], 0.0123);
    assert_eq!(line["cost_from"], "claude");
    let at = line["at"].as_str().unwrap();
    assert!(
        at.len() == 20 && at.ends_with('Z') && at.as_bytes()[10] == b'T',
        "{at}"
    );
    assert!(line["seconds"].as_f64().unwrap() >= 0.0);
    assert!(!ledger(&home.0).unwrap().contains(MARKER));
}

/// Run one call against a stub that prints `reply` and exits `code`; the error text and the line.
fn failing(reply: &str, code: i32) -> (String, Value) {
    let home = Scratch::new("call-fail");
    let stub = Stub::replying(reply, code);
    let session = common::session(&home.0);
    let schema = schema();
    let prompt = format!("records {MARKER}");
    let error = stub
        .claude()
        .call(&session, &call(&prompt, &schema))
        .unwrap_err()
        .to_string();
    let ledger_text = ledger(&home.0).unwrap();
    assert!(!ledger_text.contains(MARKER));
    assert!(!ledger_text.contains("secret words"));
    let lines = ledger_lines(&home.0);
    assert_eq!(lines.len(), 1);
    (error, lines[0].clone())
}

fn reply_with(fields: Value) -> String {
    let mut reply: Value = serde_json::from_str(&ok_reply(&json!({"labels": []}))).unwrap();
    for (key, value) in fields.as_object().unwrap() {
        if value.is_null() {
            reply.as_object_mut().unwrap().remove(key);
        } else {
            reply[key] = value.clone();
        }
    }
    reply.to_string()
}

#[test]
fn an_error_reply_fails_and_keeps_its_cost() {
    let (error, line) = failing(
        &reply_with(json!({"is_error": true, "result": "secret words"})),
        0,
    );
    assert_eq!(error, text::FAILED);
    assert_eq!(line["outcome"], "error");
    assert_eq!(line["cost_usd"], 0.0123);
    assert_eq!(line["cost_from"], "claude");
    assert_eq!(line["output_tokens"], 300);

    let (error, line) = failing(&reply_with(json!({})), 1);
    assert_eq!(error, text::FAILED);
    assert_eq!(line["outcome"], "error");

    let (error, line) = failing(&reply_with(json!({"is_error": null})), 0);
    assert_eq!(error, text::FAILED);
    assert_eq!(line["outcome"], "error");
}

#[test]
fn a_signed_out_reply_says_so() {
    for result in [
        "Not logged in · Please run /login",
        "Failed to authenticate. API Error: 401",
        "Could not authenticate with your account",
    ] {
        let (error, line) = failing(&reply_with(json!({"is_error": true, "result": result})), 1);
        assert_eq!(error, text::SIGNED_OUT, "{result}");
        assert_eq!(line["outcome"], "signed-out");
    }
}

#[test]
fn a_reply_without_usable_output_fails() {
    for output in [Value::Null, json!("labels"), json!(3)] {
        let reply = if output.is_null() {
            reply_with(json!({"structured_output": null}))
        } else {
            reply_with(json!({"structured_output": output}))
        };
        let (error, line) = failing(&reply, 0);
        assert_eq!(error, text::FAILED);
        assert_eq!(line["outcome"], "no-output");
    }
    let home = Scratch::new("call-array");
    let stub = Stub::replying(&ok_reply(&json!([1, 2])), 0);
    let session = common::session(&home.0);
    let schema = schema();
    let reply = stub.claude().call(&session, &call("x", &schema)).unwrap();
    assert_eq!(reply.output, json!([1, 2]));
}

#[test]
fn an_unreadable_reply_is_recorded_at_the_whole_budget() {
    for reply in ["not json", "[1,2]", "", "{\"is_error\": false"] {
        let (error, line) = failing(reply, 0);
        assert_eq!(error, text::FAILED, "{reply:?}");
        assert_eq!(line["outcome"], "unreadable");
        assert_eq!(line["cost_usd"], 5.0);
        assert_eq!(line["cost_from"], "budget");
        assert_eq!(line["input_tokens"], 0);
    }
    for cost in [json!(null), json!("0.5"), json!(-1.0)] {
        let (_, line) = failing(
            &reply_with(json!({"total_cost_usd": cost, "is_error": true})),
            0,
        );
        assert_eq!(line["cost_usd"], 5.0, "{cost}");
        assert_eq!(line["cost_from"], "budget");
    }
}

#[test]
fn odd_usage_counts_as_zero() {
    let (_, line) = failing(
        &reply_with(
            json!({"is_error": true, "usage": {"input_tokens": -4, "output_tokens": "9", "cache_read_input_tokens": 2.5}}),
        ),
        0,
    );
    assert_eq!(line["input_tokens"], 0);
    assert_eq!(line["output_tokens"], 0);
    assert_eq!(line["cache_read_input_tokens"], 0);
}

#[test]
fn output_past_the_cap_is_unreadable() {
    let home = Scratch::new("call-big");
    let stub = Stub::new(
        "printf '{\"is_error\":false,\"structured_output\":{\"a\":\"'\nhead -c 17000000 /dev/zero | tr '\\0' a\nprintf '\"}}'",
    );
    let session = common::session(&home.0);
    let schema = schema();
    let error = stub
        .claude()
        .call(&session, &call("x", &schema))
        .unwrap_err();
    assert_eq!(error.to_string(), text::FAILED);
    assert_eq!(ledger_lines(&home.0)[0]["outcome"], "unreadable");
}

#[test]
fn a_call_past_its_time_is_cut_off_with_everything_it_started() {
    let home = Scratch::new("call-hang");
    let stub = Stub::new("sleep 30 &\necho $! > \"$here/pid\"\nwait");
    let session = common::session(&home.0);
    let schema = schema();
    let error = stub
        .claude()
        .with_timeout(Duration::from_secs(1))
        .call(&session, &call("x", &schema))
        .unwrap_err();
    assert_eq!(error.to_string(), text::cut_off(1));
    let line = &ledger_lines(&home.0)[0];
    assert_eq!(line["outcome"], "cut-off");
    assert_eq!(line["cost_usd"], 5.0);
    assert_eq!(line["cost_from"], "budget");
    let pid = stub.file("pid");
    let alive = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .status()
        .unwrap();
    assert!(!alive.success(), "the stub's child {pid} outlived the call");
}

fn refused(home: &Scratch, stub: &Stub, call: &Brief) -> String {
    let session = common::session(&home.0);
    let error = stub.claude().call(&session, call).unwrap_err().to_string();
    assert!(!stub.ran(), "the stub ran for a refused call");
    assert!(ledger(&home.0).is_none());
    error
}

#[test]
fn bad_calls_are_refused_before_anything_runs() {
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let schema = schema();
    let big = "x".repeat(MAX_PROMPT + 1);
    let not_object = json!(["labels"]);
    let base = call("records", &schema);
    let cases: Vec<Brief> = vec![
        Brief {
            step: "",
            ..call("records", &schema)
        },
        Brief {
            step: "Label",
            ..call("records", &schema)
        },
        Brief {
            step: "label step",
            ..call("records", &schema)
        },
        Brief {
            model: "",
            ..call("records", &schema)
        },
        Brief {
            model: "Sonnet",
            ..call("records", &schema)
        },
        Brief {
            model: "sonnet --tools default",
            ..call("records", &schema)
        },
        Brief {
            model: "-sonnet",
            ..call("records", &schema)
        },
        Brief {
            prompt: "  \n",
            ..call("records", &schema)
        },
        Brief {
            prompt: &big,
            ..call("records", &schema)
        },
        Brief {
            system: " ",
            ..call("records", &schema)
        },
        Brief {
            schema: &not_object,
            ..call("records", &schema)
        },
    ];
    for case in &cases {
        let home = Scratch::new("call-refused");
        refused(&home, &stub, case);
    }
    let home = Scratch::new("call-ok-after");
    let session = common::session(&home.0);
    stub.claude().call(&session, &base).unwrap();
}

#[test]
fn a_prompt_at_the_limit_is_accepted() {
    let home = Scratch::new("call-limit");
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let session = common::session(&home.0);
    let schema = schema();
    let prompt = "x".repeat(MAX_PROMPT);
    stub.claude()
        .call(&session, &call(&prompt, &schema))
        .unwrap();
    assert_eq!(stub.file("stdin").len(), MAX_PROMPT);
}

#[test]
fn anything_in_the_work_folder_stops_the_call() {
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let schema = schema();
    for name in ["notes.txt", ".mcp.json", ".claude"] {
        let home = Scratch::new("call-planted");
        let session = common::session(&home.0);
        let agent = state(&home.0).join("agent");
        if name == ".claude" {
            fs::create_dir_all(agent.join(".claude")).unwrap();
            fs::write(agent.join(".claude/settings.json"), "{}").unwrap();
        } else {
            fs::write(agent.join(name), "{}").unwrap();
        }
        let error = stub
            .claude()
            .call(&session, &call("records", &schema))
            .unwrap_err();
        assert_eq!(error.to_string(), text::work_folder_not_empty(&agent));
        assert!(!stub.ran());
        assert!(ledger(&home.0).is_none());
    }
    let home = Scratch::new("call-linked");
    let session = common::session(&home.0);
    let agent = state(&home.0).join("agent");
    fs::remove_dir(&agent).unwrap();
    let elsewhere = home.0.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    symlink(&elsewhere, &agent).unwrap();
    let error = stub
        .claude()
        .call(&session, &call("records", &schema))
        .unwrap_err();
    assert!(
        error.to_string().contains("is a link or not a folder"),
        "{error}"
    );
    assert!(!stub.ran());
}

#[test]
fn a_program_that_cannot_start_spends_nothing() {
    let home = Scratch::new("call-missing");
    let session = common::session(&home.0);
    let schema = schema();
    let claude = persona_builder_agent::claude::Claude::at(home.0.join("no-such-claude"));
    let error = claude
        .call(&session, &call("records", &schema))
        .unwrap_err();
    assert_eq!(error.to_string(), text::DOES_NOT_RUN);
    assert!(ledger_lines(&home.0).is_empty());
}

#[test]
fn a_call_that_never_started_gives_its_budget_back() {
    let home = Scratch::new("call-missing-then-ok");
    seed_ledger(
        &home.0,
        &[spent_line(
            &persona_builder_agent::ledger::local_day(fixed_time()),
            45.0,
        )],
    );
    let session = common::session(&home.0);
    let schema = schema();
    let missing = persona_builder_agent::claude::Claude::at(home.0.join("no-such-claude"));
    missing
        .call(&session, &call("records", &schema))
        .unwrap_err();
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    assert_eq!(ledger_lines(&home.0).len(), 2);
}

#[test]
fn a_call_is_on_the_ledger_at_its_whole_budget_while_it_runs() {
    let home = Scratch::new("call-started");
    let runs = state(&home.0).join("runs.jsonl");
    let stub = Stub::new(&format!(
        "cp '{}' \"$here/during\"\ncat <<'EOF'\n{}\nEOF",
        runs.display(),
        ok_reply(&json!({}))
    ));
    let session = common::session(&home.0);
    let schema = schema();
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    let during: Vec<Value> = stub
        .file("during")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(during.len(), 1);
    assert_eq!(during[0]["outcome"], "started");
    assert_eq!(during[0]["cost_usd"], 5.0);
    assert_eq!(during[0]["cost_from"], "budget");
    let after = ledger_lines(&home.0);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0]["outcome"], "ok");
    assert_eq!(after[0]["cost_usd"], 0.0123);
    assert_eq!(after[0]["id"], during[0]["id"]);
    assert!(after[0]["id"].as_str().is_some_and(|id| !id.is_empty()));
}

#[test]
fn a_prompt_that_starts_like_a_command_is_refused() {
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let schema = schema();
    for prompt in [
        "/model opus",
        "  \n/config x=y",
        "\t/fast",
        "\u{feff}/model opus",
        "\u{200b} /config x=y",
        "\u{2060}\u{00ad}/fast",
    ] {
        let home = Scratch::new("call-slash");
        refused(&home, &stub, &call(prompt, &schema));
    }
    let home = Scratch::new("call-slash-later");
    let session = common::session(&home.0);
    stub.claude()
        .call(&session, &call("records\n/model opus", &schema))
        .unwrap();
}

#[test]
fn optional_budgets_change_only_the_budget_arguments() {
    use persona_builder_agent::ledger::{Cap, Session};
    for (daily, step, spent, budget) in [
        (0.0, 0.0, 0.0, None),
        (20.0, 0.0, 0.0, Some("20.00")),
        (20.0, 0.0, 12.344, Some("7.65")),
        (0.0, 5.0, 1500.0, Some("5.00")),
    ] {
        let home = Scratch::new("optional-budget");
        seed_ledger(&home.0, &[spent_line(&local_day(fixed_time()), spent)]);
        let stub = Stub::replying(&ok_reply(&json!({})), 0);
        let cap = Cap::new(daily, step).expect("zero means no limit");
        let session = Session::open_with_clock(&home.0, "wren", cap, fixed_clock()).unwrap();
        let schema = schema();
        stub.claude()
            .call(&session, &call("records", &schema))
            .unwrap();
        let mut expected = expected_argv(&schema);
        expected.truncate(expected.len() - 2);
        if let Some(budget) = budget {
            expected.extend(["--max-budget-usd".into(), budget.into()]);
        }
        assert_eq!(stub.argv(0).unwrap(), expected);
    }
}

#[test]
fn an_unlimited_call_without_a_cost_records_unknown_and_the_ledger_reads_it() {
    use persona_builder_agent::ledger::{Cap, Session};
    let home = Scratch::new("unknown-cost");
    let stub = Stub::replying(&reply_with(json!({"total_cost_usd": null})), 0);
    let cap = Cap::new(0.0, 0.0).expect("zero means no limit");
    let session = Session::open_with_clock(&home.0, "wren", cap, fixed_clock()).unwrap();
    let schema = schema();
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    let lines = ledger_lines(&home.0);
    assert_eq!(lines[0]["cost_usd"], 0.0);
    assert_eq!(lines[0]["cost_from"], "unknown");
    assert_eq!(session.spent_today().unwrap(), 0.0);
    // The reader accepts arbitrary cost_from and every finite, nonnegative cost.
    let mut line = lines[0].clone();
    line["cost_from"] = json!("future-source");
    line["cost_usd"] = json!(1234.56789);
    seed_ledger(&home.0, &[line.to_string()]);
    assert_eq!(session.spent_today().unwrap(), 1234.56789);
}

#[test]
fn local_history_is_refused_before_the_stub_runs_or_spends() {
    let home = Scratch::new("local-history");
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let claude = stub.claude();
    let mut member: Member =
        serde_json::from_str(include_str!("../../../crew/_template/member.json")).unwrap();
    member
        .extra
        .insert("ceilings".into(), json!({"claude-history": "local-only"}));
    let guard = Guard::new(vec![&claude], Ceilings::for_member(&member).unwrap());
    let schema = schema();
    let error = guard
        .call(
            &common::session(&home.0),
            &Brief {
                sources: &[Source::ClaudeHistory],
                ..call("records", &schema)
            },
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Nothing was sent. This step uses your Claude history (your Claude Code sessions and your claude.ai export), which you keep on your own machines, and Claude, through your own Claude Code sends what it's given to Anthropic. To let it go there, run: persona-builder ceilings set <assistant folder> claude-history anthropic"
    );
    assert!(!stub.ran());
    assert!(ledger_lines(&home.0).is_empty());
}
