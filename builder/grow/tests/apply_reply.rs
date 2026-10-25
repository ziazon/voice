use persona_builder_grow::{apply::*, record::*};
use serde_json::{Value, json};
fn case(options: bool) -> Case {
    Case {
        schema: CASE_V1.into(),
        id: "case-one".into(),
        persona: "sam".into(),
        created: "2026-01-01T00:00:00Z".into(),
        from_signals: vec!["sig-hidden".into()],
        area: "work".into(),
        situation: "Which first step?".into(),
        options: options.then(|| vec!["Start small".into(), "Expand".into()]),
        expected: "SECRET_EXPECTATION".into(),
        r#match: if options {
            Match::Choice
        } else {
            Match::Rubric
        },
        status: Status::New,
    }
}
#[test]
fn closed_required_schemas() {
    for (schema, key, field) in [
        (answer_schema(), "answers", "answer"),
        (grade_schema(), "grades", "grade"),
    ] {
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!([key]));
        assert_eq!(schema["properties"][key]["type"], "array");
        let item = &schema["properties"][key]["items"];
        assert_eq!(item["type"], "object");
        assert_eq!(item["additionalProperties"], false);
        assert_eq!(item["required"], json!(["id", field]));
        assert_eq!(item["properties"]["id"]["type"], "string");
        assert_eq!(item["properties"][field]["type"], "string");
        assert_eq!(schema["properties"].as_object().unwrap().len(), 1);
        assert_eq!(item["properties"].as_object().unwrap().len(), 2);
    }
    assert_eq!(
        grade_schema()["properties"]["grades"]["items"]["properties"]["grade"]["enum"],
        json!(["pass", "fail"])
    );
}
#[test]
fn answer_prompt_is_blind_and_exact() {
    let mut c = case(true);
    c.expected = "Expand".into();
    let open = case(false);
    let prompt = answer_prompt("DECISION_SENTINEL", &[&c, &open]);
    assert!(prompt.starts_with("Answer each test the way the person would.\n\nHow the person decides (their decisions.md):\n\nDECISION_SENTINEL\n\nThe tests, one JSON object per line:\n"));
    let lines: Vec<Value> = prompt
        .lines()
        .filter(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines,
        vec![
            json!({"id":"case-one","area":"work","situation":"Which first step?","options":["Start small","Expand"]}),
            json!({"id":"case-one","area":"work","situation":"Which first step?"})
        ]
    );
    for secret in [
        "SECRET_EXPECTATION",
        "sig-hidden",
        "expected",
        "match",
        "candidate",
        "base",
    ] {
        assert!(!prompt.contains(secret));
    }
    assert_eq!(
        ANSWER_SYSTEM,
        "You are one person's AI assistant, built by Persona Builder to answer the way they would. The\nmessage gives how the person decides (their decisions.md), then tests, one JSON object per line.\nAnswer every test the way the person would, from how they decide. When a test has `options`,\nanswer with the exact text of one option, copied exactly. When it has none, answer in one or two\nplain sentences in your own words. You have no tools. Answer only with the JSON the schema asks\nfor: exactly one answer for every test id, and none for an id that is not there.\n"
    );
}
#[test]
fn grade_prompt_is_blind_and_exact() {
    let prompt = grade_prompt(&[("item-1".into(), "Use a smaller step.", "Try smaller.")]);
    assert_eq!(
        prompt,
        "Grade each answer against what the right answer must do.\n\nThe items, one JSON object per line:\n{\"id\":\"item-1\",\"expected\":\"Use a smaller step.\",\"answer\":\"Try smaller.\"}\n"
    );
    assert_eq!(
        GRADE_SYSTEM,
        "You grade answers. Each item, one JSON object per line, has an `id`, `expected` (what the right\nanswer must do or say) and an `answer`. Grade \"pass\" when the answer does what `expected`\ndescribes, in substance, even in other words. Grade \"fail\" when it misses it, contradicts it, or\npicks something `expected` rules out. Judge each item on its own. You have no tools. Answer only\nwith the JSON the schema asks for: exactly one grade for every item id.\n"
    );
}
#[test]
fn answers_require_exact_shape_ids_and_choices() {
    let c = case(true);
    let batch = [&c];
    let valid = json!({"answers":[{"id":"case-one","answer":"Start small"}]});
    assert_eq!(
        check_answers(&batch, &valid)
            .unwrap()
            .get("case-one")
            .map(String::as_str),
        Some("Start small")
    );
    for bad in [
        json!({"answers":[{"id":"case-one","answer":"Start small"}],"extra":1}),
        json!({"answers":[{"id":"case-one","answer":"Start small"},{"id":"foreign-secret","answer":"Start small"}]}),
        json!({}),
        json!({"answers":null}),
        json!({"answers":[],"extra":1}),
        json!({"answers":[{"id":"case-one","answer":"Start small","extra":1}]}),
        json!({"answers":[{"id":"case-one"}]}),
        json!({"answers":[{"id":4,"answer":"Start small"}]}),
        json!({"answers":[{"id":"case-one","answer":4}]}),
        json!({"answers":[]}),
        json!({"answers":[{"id":"foreign-secret","answer":"Start small"}]}),
        json!({"answers":[{"id":"case-one","answer":"Start small"},{"id":"case-one","answer":"Start small"}]}),
        json!({"answers":[{"id":"case-one","answer":" Start small"}]}),
        json!({"answers":[{"id":"case-one","answer":"unknown-secret"}]}),
    ] {
        let error = check_answers(&batch, &bad).unwrap_err();
        assert!(!error.shown.contains("secret"));
        assert!(!error.told.contains("secret"));
        assert!(error.shown.contains("of its answers didn't fit"));
    }
}
#[test]
fn open_answers_require_trimmed_plain_bounded_lines() {
    let c = case(false);
    let batch = [&c];
    for good in ["A useful small step.", &"é".repeat(1000)] {
        assert_eq!(
            check_answers(
                &batch,
                &json!({"answers":[{"id":"case-one","answer":good}]})
            )
            .unwrap()
            .get("case-one")
            .map(String::as_str),
            Some(good)
        );
    }
    for bad in [
        String::new(),
        " ".into(),
        " x".into(),
        "x ".into(),
        "x\ny".into(),
        "x\ry".into(),
        "x\u{200b}y".into(),
        "x\u{202e}y".into(),
        "x\u{0}y".into(),
        "x".repeat(1001),
    ] {
        let error = check_answers(&batch, &json!({"answers":[{"id":"case-one","answer":bad}]}))
            .unwrap_err();
        assert!(!error.shown.contains(&bad) || bad.is_empty() || bad == " ");
    }
}
#[test]
fn grades_require_exact_shape_ids_and_values() {
    let items = [
        ("item-1".into(), "expected", "answer"),
        ("item-2".into(), "expected", "answer"),
    ];
    let good = json!({"grades":[{"id":"item-1","grade":"pass"},{"id":"item-2","grade":"fail"}]});
    let grades = check_grades(&items, &good).unwrap();
    assert_eq!(grades.get("item-1"), Some(&Grade::Pass));
    assert_eq!(grades.get("item-2"), Some(&Grade::Fail));
    for bad in [
        json!({"grades":[{"id":"item-1","grade":"pass"},{"id":"item-2","grade":"fail"}],"extra":1}),
        json!({"grades":[{"id":"item-1","grade":"pass"},{"id":"item-1","grade":"pass"},{"id":"item-2","grade":"fail"}]}),
        json!({"grades":[{"id":"item-1","grade":"pass"},{"id":"item-2","grade":"fail"},{"id":"foreign-secret","grade":"pass"}]}),
        json!({}),
        json!({"grades":null}),
        json!({"grades":[],"extra":1}),
        json!({"grades":[{"id":"item-1","grade":"pass","extra":1}]}),
        json!({"grades":[{"id":"item-1"}]}),
        json!({"grades":[{"id":4,"grade":"pass"}]}),
        json!({"grades":[{"id":"item-1","grade":4}]}),
        json!({"grades":[]}),
        json!({"grades":[{"id":"foreign-secret","grade":"pass"},{"id":"item-2","grade":"fail"}]}),
        json!({"grades":[{"id":"item-1","grade":"pass"},{"id":"item-1","grade":"fail"}]}),
        json!({"grades":[{"id":"item-1","grade":"PASS"},{"id":"item-2","grade":"fail"}]}),
    ] {
        let mut bad = bad;
        if let Some(rows) = bad.get_mut("grades").and_then(Value::as_array_mut)
            && rows.len() == 1
        {
            rows.push(json!({"id":"item-2","grade":"fail"}));
        }
        let error = check_grades(&items, &bad).unwrap_err();
        assert!(!error.shown.contains("secret"));
        assert!(!error.told.contains("secret"));
        assert!(error.shown.contains("of its grades didn't fit"));
    }
}
