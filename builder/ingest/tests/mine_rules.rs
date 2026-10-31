use persona_builder::deny::DenyList;
mod common;
use common::{Scratch, repo};
use persona_builder::member::read_member;
use persona_builder_ingest::{
    filters::Filters,
    mine::{Split, folders, mine},
};
use std::fs;
#[test]
fn links_inside_history_are_refused() {
    let s = Scratch::new("mine-link");
    let p = s.0.join("projects");
    fs::create_dir(&p).unwrap();
    fs::create_dir(s.0.join("target")).unwrap();
    std::os::unix::fs::symlink(s.0.join("target"), p.join("linked")).unwrap();
    let m = read_member(&repo().join("crew/_template/member.json")).unwrap();
    let f = Filters::compile(&m.filters).unwrap();
    assert!(mine(&m, &f, &p, &s.0, &Split::Holdout(0.15)).is_err());
    assert!(folders(&f, &p, &s.0, &DenyList::builtin()).is_err());
}

fn member() -> persona_builder::member::Member {
    read_member(&repo().join("crew/_template/member.json")).unwrap()
}
fn run(
    p: &std::path::Path,
    h: &std::path::Path,
    split: &Split,
) -> persona_builder::error::Result<persona_builder_ingest::mine::Mined> {
    let m = member();
    mine(&m, &Filters::compile(&m.filters).unwrap(), p, h, split)
}
#[test]
fn unsupported_json_is_skipped() {
    let s = Scratch::new("unsupported-json");
    let p = s.0.join("projects");
    fs::create_dir_all(p.join("app")).unwrap();
    let lines = [
        "NaN".to_owned(),
        "Infinity".into(),
        r#""\ud800""#.into(),
        "1e9999".into(),
        format!("{}0{}", "[".repeat(130), "]".repeat(130)),
    ];
    fs::write(p.join("app/s.jsonl"), lines.join("\n")).unwrap();
    assert!(
        run(&p, &s.0, &Split::Holdout(0.15))
            .unwrap()
            .records
            .is_empty()
    );
}
#[test]
fn holdout_requires_finite_unit_interval() {
    let s = Scratch::new("bad-holdout");
    for h in [-0.1, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            run(&s.0, &s.0, &Split::Holdout(h)).unwrap_err().to_string(),
            "--holdout must be a number from 0 to 1"
        );
    }
}
#[test]
fn non_utf8_names_error_where_filesystem_allows_them() {
    use std::os::unix::ffi::OsStringExt;
    let s = Scratch::new("nonutf8");
    let p = s.0.join(std::ffi::OsString::from_vec(b"bad\xff".to_vec()));
    if fs::create_dir(&p).is_err() {
        return;
    }
    assert!(
        run(&s.0, &s.0, &Split::Holdout(0.15))
            .unwrap_err()
            .to_string()
            .contains("is not UTF-8")
    );
    fs::remove_dir(p).unwrap();
    let app = s.0.join("app");
    fs::create_dir(&app).unwrap();
    let p = app.join(std::ffi::OsString::from_vec(b"bad\xff.jsonl".to_vec()));
    fs::write(&p, b"").unwrap();
    assert!(
        run(&s.0, &s.0, &Split::Holdout(0.15))
            .unwrap_err()
            .to_string()
            .contains("is not UTF-8")
    );
}
#[test]
fn jsonl_directories_and_pipes_are_not_files() {
    use std::process::Command;
    let s = Scratch::new("not-files");
    let app = s.0.join("app");
    fs::create_dir(&app).unwrap();
    let bad = app.join("bad.jsonl");
    let m = member();
    let f = Filters::compile(&m.filters).unwrap();
    for pipe in [false, true] {
        if pipe {
            assert!(Command::new("mkfifo").arg(&bad).status().unwrap().success())
        } else {
            fs::create_dir(&bad).unwrap()
        }
        let expected = format!("{} is not a file", bad.display());
        assert_eq!(
            run(&s.0, &s.0, &Split::Holdout(0.15))
                .unwrap_err()
                .to_string(),
            expected
        );
        assert_eq!(
            folders(&f, &s.0, &s.0, &DenyList::builtin())
                .unwrap_err()
                .to_string(),
            expected
        );
        if pipe {
            fs::remove_file(&bad).unwrap()
        } else {
            fs::remove_dir(&bad).unwrap()
        }
    }
}
#[test]
fn project_root_link_is_allowed_session_links_are_refused() {
    let s = Scratch::new("session-links");
    let projects = s.0.join("projects");
    let root = s.0.join("root-link");
    fs::create_dir_all(projects.join("app")).unwrap();
    std::os::unix::fs::symlink(&projects, &root).unwrap();
    assert!(run(&root, &s.0, &Split::Holdout(0.15)).is_ok());
    let target = s.0.join("outside");
    fs::write(&target, b"secret sentinel").unwrap();
    let link = projects.join("app/linked.jsonl");
    std::os::unix::fs::symlink(target, &link).unwrap();
    assert_eq!(
        run(&root, &s.0, &Split::Holdout(0.15))
            .unwrap_err()
            .to_string(),
        format!(
            "{} is a link: Persona Builder doesn't follow links in your Claude Code history. Move it away and try again",
            root.join("app/linked.jsonl").display()
        )
    );
}
#[test]
fn folders_match_python_including_equal_project_names() {
    if !common::parity_tools_available() {
        return;
    }
    let s = Scratch::new("folders-parity");
    let p = s.0.join("projects");
    common::checked(
        std::process::Command::new("python3")
            .arg("-I")
            .arg(repo().join("tests/crew_mine_fixtures.py"))
            .arg("folders")
            .arg(&p),
    );
    let config = s.0.join("member.json");
    fs::copy(repo().join("crew/_template/member.json"), &config).unwrap();
    let m = read_member(&config).unwrap();
    let f = Filters::compile(&m.filters).unwrap();
    let py = common::checked(
        std::process::Command::new("python3")
            .env("HOME", "/Users/p")
            .arg("-I")
            .arg(repo().join("scripts/crew_decisions.py"))
            .arg("--member")
            .arg(config)
            .arg("folders")
            .arg("--projects")
            .arg(&p),
    );
    assert_eq!(
        persona_builder_ingest::mine::render_folders(
            &folders(
                &f,
                &p,
                std::path::Path::new("/Users/p"),
                &DenyList::builtin()
            )
            .unwrap()
        )
        .as_bytes(),
        py.stdout
    );
}
#[test]
fn excluded_folders_are_not_listed_by_mine() {
    let s = Scratch::new("excluded");
    let app = s.0.join("excluded");
    fs::create_dir(&app).unwrap();
    fs::create_dir(app.join("bad.jsonl")).unwrap();
    let mut m = member();
    m.filters.excluded_folders = "excluded".into();
    let f = Filters::compile(&m.filters).unwrap();
    assert_eq!(
        mine(&m, &f, &s.0, &s.0, &Split::Holdout(0.15))
            .unwrap()
            .excluded,
        1
    );
    assert!(folders(&f, &s.0, &s.0, &DenyList::builtin()).is_err());
}

#[test]
fn answers_shape_is_skipped_before_question_text() {
    let s = Scratch::new("answers-shape");
    let app = s.0.join("app");
    fs::create_dir(&app).unwrap();
    let path = app.join("s.jsonl");
    for questions in [
        serde_json::json!([{}]),
        serde_json::json!([{"question": 7}]),
    ] {
        let record = serde_json::json!({
            "type": "user", "timestamp": "2026-09-01T10:00:00Z",
            "toolUseResult": {"questions": questions, "answers": []}
        });
        fs::write(&path, record.to_string()).unwrap();
        let mined = run(&s.0, &s.0, &Split::Holdout(0.15)).unwrap();
        assert!(mined.records.is_empty());
        assert!(
            persona_builder_ingest::mine::summary(&mined)
                .contains("; skipped 1 records of an unexpected shape")
        );
    }
    fs::write(
        &path,
        serde_json::json!({
            "type": "user", "timestamp": "2026-09-01T10:00:00Z",
            "toolUseResult": {"questions": [], "answers": []}
        })
        .to_string(),
    )
    .unwrap();
    assert!(run(&s.0, &s.0, &Split::Holdout(0.15)).is_ok());
}

#[test]
fn a_list_answer_is_the_picks_it_names() {
    let s = Scratch::new("list-answers");
    let p = s.0.join("projects");
    common::checked(
        std::process::Command::new("python3")
            .arg("-I")
            .arg(repo().join("tests/crew_mine_fixtures.py"))
            .arg("list_answers")
            .arg(&p),
    );
    let mined = run(&p, &s.0, &Split::Holdout(0.15)).unwrap();
    assert_eq!(mined.unreadable, 0);
    assert_eq!(mined.records.len(), 9);
    for (i, (picks, own)) in [
        (serde_json::json!(["A longer", "B"]), ""),
        (serde_json::json!(["B", "A"]), ""),
        (serde_json::json!(["A"]), "own words, with a comma"),
        (serde_json::json!(["Yes, keep it"]), ""),
        (serde_json::json!(["A (Recommended)"]), ""),
        (serde_json::json!(["A"]), ""),
        (serde_json::json!([]), ""),
        (serde_json::json!(["A"]), ""),
        (serde_json::json!(["A", "B"]), "first own, second own"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(mined.records[i]["question"], format!("List {i}?"));
        assert_eq!(mined.records[i]["picks"], picks);
        assert_eq!(mined.records[i]["own_words"], own);
    }
    assert_eq!(mined.records[4]["outcome"], "took_rec");
}

fn question_answer(answer: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"type":"user", "timestamp":"2026-09-01T10:01:00Z",
        "toolUseResult":{"questions":[{"question":"Scope?", "options":[{"label":"A"}]}],
            "answers":{"Scope?":answer}}})
}
fn typed_message(text: &str, when: &str) -> serde_json::Value {
    serde_json::json!({"type":"user", "timestamp":when,
        "origin":{"kind":"human"}, "message":{"content":text}})
}
fn session_records(s: &Scratch, records: &[serde_json::Value]) {
    fs::create_dir(s.0.join("app")).unwrap();
    fs::write(
        s.0.join("app/s.jsonl"),
        records.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
}
#[test]
fn records_of_an_unexpected_shape_are_skipped_and_counted() {
    use serde_json::json;
    let s = Scratch::new("skipped-records");
    let mut bad_options = question_answer(json!("A"));
    bad_options["toolUseResult"]["questions"][0]["options"] = json!({"a":1});
    let mut two_questions = question_answer(json!("A"));
    two_questions["toolUseResult"]["questions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"question":"Second?", "options":[{"label":"A"}]}));
    two_questions["toolUseResult"]["answers"]["Second?"] = json!(7);
    session_records(
        &s,
        &[
            typed_message("first kept", "2026-09-01T10:00:00Z"),
            question_answer(json!(7)),
            question_answer(json!(["A", 7])),
            bad_options,
            json!([1, 2]),
            two_questions,
            typed_message("last kept", "2026-09-01T10:02:00Z"),
        ],
    );
    let mined = run(&s.0, &s.0, &Split::Holdout(0.15)).unwrap();
    assert_eq!(mined.unreadable, 5);
    assert_eq!(
        mined
            .records
            .iter()
            .map(|r| (&r["kind"], &r["text"]))
            .collect::<Vec<_>>(),
        vec![
            (&json!("message"), &json!("first kept")),
            (&json!("message"), &json!("last kept"))
        ]
    );
    assert!(
        persona_builder_ingest::mine::summary(&mined)
            .ends_with("0 secret records; skipped 5 records of an unexpected shape\n")
    );
}
#[test]
fn a_skipped_record_keeps_the_reply_before_it_as_context() {
    let s = Scratch::new("skipped-context");
    session_records(
        &s,
        &[
            serde_json::json!({"type":"assistant", "message":{"content":"the reply"}}),
            question_answer(serde_json::json!(7)),
            typed_message("kept message", "2026-09-01T10:02:00Z"),
        ],
    );
    let mined = run(&s.0, &s.0, &Split::Holdout(0.15)).unwrap();
    assert_eq!(mined.unreadable, 1);
    assert_eq!(mined.records.len(), 1);
    assert_eq!(mined.records[0]["context"], "the reply");
}
