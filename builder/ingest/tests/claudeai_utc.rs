mod common;
use common::{parity_tools_available, repo};
use persona_builder_ingest::claudeai::utc;
use std::process::Command;

const BOTH: &[&str] = &[
    "2026-03-01T12:00:00.000000Z",
    "2026-03-01T12:00:00+00:00",
    "2026-03-01",
    "2026-03-01T12:00",
    "2026-03-01T12:00Z",
    "2026-03-01T12:00+01:00",
    "2026-03-01 12:00:00",
    "2026-03-01T12:00:00.1234567Z",
    "2026-03-01T12:00:00.123456789Z",
    "2026-03-01T12:00:00.9999999Z",
    "2026-03-01T12:00:00.5",
    "2026-03-01T12:00:00.5+01:00",
    "2026-03-01T12:00:00.123+01:00",
    "2026-03-01T12:00:00.123456789123+05:00",
    "2026-03-01T12:00:00-23:59",
    "2026-03-01T12:00:00-00:00",
    "2024-02-29T00:00:00",
    "2025-12-31T23:30:00-01:00",
    "2026-01-01T00:30:00+01:00",
    "0001-01-01T00:00:00Z",
    "9999-12-31T23:59:59.999Z",
];
const NEITHER: &[&str] = &[
    "2026-03-01T12:00:00z",
    "2026-03-01T24:00:00",
    "2026-02-29T00:00:00",
    "2026-03-01T12:00:00+24:00",
    "2026-03-01T12:00:60",
    "2026-3-01",
    " 2026-03-01",
    "+2026-03-01",
    "2026-03-01T12:00:00ZZ",
    "2026-03-01T12:00:00+00:00Z",
    "2026-03-01T12:00:00Z+01:00",
    "２０２６-03-01",
    "2026-03-01T",
    "2026-03-01 ",
    "2026-03-01T12:00:00.",
    "2026-03-01T12:00:00Z ",
    "2026-03-01T12:00:00−01:00",
    "2026-03-01T12:0:00",
    "0001-01-01T00:00:00+01:00",
    "9999-12-31T23:59:59-01:00",
    "",
    "not a date",
];
/// Forms Python's `fromisoformat` takes that Persona Builder refuses (accepted difference 1).
const PYTHON_ONLY: &[&str] = &[
    "2026-03-01T12",
    "2026-03-01x12:00:00",
    "2026-03-01t12:00:00",
    "20260301T120000Z",
    "2026-W09-1",
    "2026-03-01T1200",
    "2026-03-01T12:00:00,5Z",
    "2026-03-01T12:00:00.Z",
    "2026-03-01T12:00:00+05",
    "2026-03-01T12:00:00+0530",
    "2026-03-01T12:00:00+05:30:15",
    "2026-03-01T12:00:00+05:30:15.5",
    "2026-03-01T12:00:00 +00:00",
    "2026-03-01T12:00.5",
    "2026-03-01Z",
    "2026-03-01+01:00",
];

fn python(cases: &[&str]) -> Vec<Option<String>> {
    let out = Command::new("python3")
        .arg("-I")
        .arg("-c")
        .arg(
            "import json,sys;sys.path.insert(0,sys.argv[1]);import crew_decisions as c\n\
             def f(w):\n try: return c.utc(w)\n except (c.Failed, OverflowError): return None\n\
             print(json.dumps([f(w) for w in json.loads(sys.stdin.read())]))",
        )
        .arg(repo().join("scripts"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(&serde_json::to_vec(cases).unwrap())?;
            child.wait_with_output()
        })
        .unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn shared_forms_agree_with_python() {
    if !parity_tools_available() {
        return;
    }
    let expected = python(BOTH);
    for (when, py) in BOTH.iter().zip(expected) {
        assert!(py.is_some(), "python refused {when:?}");
        assert_eq!(utc(when), py, "{when:?}");
    }
}
#[test]
fn both_refuse_the_same_bad_forms() {
    if !parity_tools_available() {
        return;
    }
    for (when, py) in NEITHER.iter().zip(python(NEITHER)) {
        assert_eq!(py, None, "python accepted {when:?}");
        assert_eq!(utc(when), None, "{when:?}");
    }
}
#[test]
fn forms_only_python_reads_are_refused() {
    if !parity_tools_available() {
        return;
    }
    for (when, py) in PYTHON_ONLY.iter().zip(python(PYTHON_ONLY)) {
        assert!(py.is_some(), "python refused {when:?}");
        assert_eq!(utc(when), None, "{when:?}");
    }
}
#[test]
fn the_output_shape() {
    assert_eq!(
        utc("2026-03-01T12:00:00.9999999Z").as_deref(),
        Some("2026-03-01T12:00:00.999Z")
    );
    assert_eq!(
        utc("2026-03-01T12:00:00-23:59").as_deref(),
        Some("2026-03-02T11:59:00.000Z")
    );
    assert_eq!(
        utc("2024-03-01T00:30:00+01:00").as_deref(),
        Some("2024-02-29T23:30:00.000Z")
    );
    assert_eq!(
        utc("2000-02-29").as_deref(),
        Some("2000-02-29T00:00:00.000Z")
    );
    assert_eq!(utc("1900-02-29"), None);
}
