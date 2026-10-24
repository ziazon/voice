mod common;

use common::{checked, parity_tools_available};
use persona_builder_ingest::{
    mine::to_jsonl,
    python::{decode, py_space, splitlines, truthy},
};
use serde_json::{Value, json};
use std::process::Command;

fn oracle(code: &str, input: &str) -> Vec<u8> {
    checked(Command::new("python3").args(["-I", "-c", code, input])).stdout
}

#[test]
fn whitespace_matches_every_python_codepoint() {
    if !parity_tools_available() {
        return;
    }
    let expected: Vec<u32> = serde_json::from_slice(&oracle(
        "import json; print(json.dumps([i for i in range(0x110000) if chr(i).isspace()]))",
        "",
    ))
    .unwrap();
    let actual: Vec<u32> = (0..=0x10ffff)
        .filter_map(char::from_u32)
        .filter(|c| py_space(*c))
        .map(u32::from)
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn decode_drops_invalid_utf8_like_python() {
    if !parity_tools_available() {
        return;
    }
    for invalid in [
        vec![0xff],
        vec![0xe2, 0x82],
        vec![0xed, 0xa0, 0x80],
        vec![0xc0, 0xaf],
        vec![0xf4, 0x90, 0x80, 0x80],
    ] {
        for bytes in [
            invalid.clone(),
            [
                b"before".as_slice(),
                invalid.as_slice(),
                "éafter".as_bytes(),
            ]
            .concat(),
        ] {
            let expected: String = serde_json::from_slice(&oracle(
                "import json,sys; print(json.dumps(bytes(json.loads(sys.argv[1])).decode('utf-8','ignore')))",
                &serde_json::to_string(&bytes).unwrap())).unwrap();
            assert_eq!(decode(&bytes), expected);
        }
    }
}

#[test]
fn lines_match_python_separators_and_empty_lines() {
    if !parity_tools_available() {
        return;
    }
    let text = "a\nb\r\nc\rd\u{b}e\u{c}f\u{1c}g\u{1d}h\u{1e}i\u{85}j\u{2028}k\u{2029}\n\nend\r\n";
    let expected: Vec<String> = serde_json::from_slice(&oracle(
        "import json,sys; print(json.dumps([s for s in sys.argv[1].splitlines() if s]))",
        text,
    ))
    .unwrap();
    let actual: Vec<_> = splitlines(text)
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn jsonl_matches_python_escaping_spacing_and_key_order() {
    if !parity_tools_available() {
        return;
    }
    let text: String = (0..0x100)
        .filter_map(char::from_u32)
        .chain("é日😀\u{2028}".chars())
        .collect();
    let record = json!({"text": text, "nested": [true, false, null, [], {}, {"first": "日", "second": [1, 2]}]});
    let input = serde_json::to_string(&record).unwrap();
    let expected = oracle(
        "import json,sys; print(json.dumps(json.loads(sys.argv[1]),ensure_ascii=False))",
        &input,
    );
    assert_eq!(
        to_jsonl(&[record.as_object().unwrap().clone()]).as_bytes(),
        expected
    );
}

#[test]
fn truthiness_matches_python_json_values() {
    if !parity_tools_available() {
        return;
    }
    let values = json!([null, false, 0, -0.0, "", [], {}, true, 1, -1, 0.5, "x", [0], {"a": null}]);
    let expected: Vec<bool> = serde_json::from_slice(&oracle(
        "import json,sys; print(json.dumps([bool(v) for v in json.loads(sys.argv[1])]))",
        &values.to_string(),
    ))
    .unwrap();
    let actual: Vec<_> = values.as_array().unwrap().iter().map(truthy).collect();
    assert_eq!(actual, expected);
    assert!(!truthy(&Value::Null));
}
