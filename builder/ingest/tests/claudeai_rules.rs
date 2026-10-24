//! The claude.ai export's refusals and the accepted differences from the Python, each beside what
//! the Python does with the same input. Every export here is invented; `SENTINEL` stands for a
//! person's words and must never appear in an error.
mod common;
use common::{Scratch, parity_tools_available, repo};
use persona_builder::member::read_member;
use persona_builder_ingest::{
    claudeai::{self, Export, ReviewRow},
    filters::Filters,
    mine::Split,
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

const SENTINEL: &str = "zebracorn";

struct Kit {
    s: Scratch,
    config: PathBuf,
    filters: Filters,
}
impl Kit {
    fn new(label: &str) -> Self {
        let s = Scratch::new(label);
        let config = s.0.join("member.json");
        let py = Command::new("python3").arg("-I").arg("-c").arg(
            "import sys,json;sys.path.insert(0,sys.argv[1]);from crew_decisions_test import ARI;c=dict(ARI);c['state_dir']='/fictional/state';print(json.dumps(c))")
            .arg(repo().join("tests")).output().unwrap();
        assert!(py.status.success());
        fs::write(&config, py.stdout).unwrap();
        let filters = Filters::compile(&read_member(&config).unwrap().filters).unwrap();
        Self { s, config, filters }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.s.0.join(name)
    }
    fn python(&self, args: &[&str]) -> Output {
        Command::new("python3")
            .arg("-I")
            .arg(repo().join("scripts/crew_decisions.py"))
            .arg("--member")
            .arg(&self.config)
            .arg("claudeai")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    /// Python's exit code for `titles` (or `mine` with every conversation marked) on an export.
    fn python_titles(&self, export: &Path) -> Option<i32> {
        let review = self.path("py-review.tsv");
        let _ = fs::remove_file(&review);
        self.python(&[
            "titles",
            "--export",
            export.to_str().unwrap(),
            "--out",
            review.to_str().unwrap(),
        ])
        .status
        .code()
    }
    fn python_mine(&self, export: &Path, review: &str) -> Output {
        let path = self.path("py-mine.tsv");
        fs::write(&path, review).unwrap();
        self.python(&[
            "mine",
            "--export",
            export.to_str().unwrap(),
            "--review",
            path.to_str().unwrap(),
            "--out",
            self.path("py-mine.jsonl").to_str().unwrap(),
        ])
    }
    fn python_inspect(&self, export: &Path) -> Output {
        self.python(&["inspect", "--export", export.to_str().unwrap()])
    }
    fn write_json(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }
    /// A zip from `[name, text, method]` entries, written by Python's zipfile.
    fn zip(&self, name: &str, entries: Value) -> PathBuf {
        let path = self.path(name);
        let out = Command::new("python3")
            .arg("-I")
            .arg("-c")
            .arg(
                "import json,sys,zipfile,warnings;warnings.simplefilter('ignore')\n\
                 m={'stored':zipfile.ZIP_STORED,'deflate':zipfile.ZIP_DEFLATED,'bzip2':zipfile.ZIP_BZIP2}\n\
                 with zipfile.ZipFile(sys.argv[1],'w') as z:\n\
                 \x20for n,t,c in json.loads(sys.argv[2]):\n\
                 \x20\x20z.writestr(zipfile.ZipInfo(n) if c=='stored' else n,t.encode('latin-1') if c=='raw' else t,compress_type=m.get(c,zipfile.ZIP_STORED))",
            )
            .arg(&path)
            .arg(entries.to_string())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        path
    }
}
fn conversation(uuid: Value, name: Value, when: Value, messages: Value) -> Value {
    json!({"uuid": uuid, "name": name, "created_at": when, "updated_at": when, "chat_messages": messages})
}
fn human(when: &str, text: &str) -> Value {
    json!({"sender": "human", "created_at": when, "text": text})
}
fn good() -> Value {
    conversation(
        json!("c-1"),
        json!("Kit plan"),
        json!("2026-03-01T09:00:00Z"),
        json!([human("2026-03-01T09:00:00Z", "keep the patch small")]),
    )
}
fn export(value: Value) -> Export {
    Export {
        files: 1,
        conversations: value,
    }
}
fn error<T>(result: persona_builder::error::Result<T>) -> String {
    let Err(e) = result else {
        panic!("expected an error")
    };
    let message = e.to_string();
    assert!(!message.contains(SENTINEL), "{message}");
    message
}
fn row(uuid: &str, mine: &str) -> ReviewRow {
    ReviewRow {
        mine: mine.into(),
        created: "2026-03-01".into(),
        updated: "2026-03-01".into(),
        messages: "1".into(),
        project: String::new(),
        title: "Kit plan".into(),
        uuid: uuid.into(),
    }
}
fn all_marked(conversations: &Value) -> String {
    let mut text = format!("{}\n", claudeai::REVIEW_HEADER);
    for c in conversations.as_array().unwrap() {
        if let Some(uuid) = c.get("uuid").and_then(Value::as_str) {
            text += &format!("y\t2026-03-01\t2026-03-01\t1\t\tt\t{uuid}\n");
        }
    }
    text
}
fn titles_error(k: &Kit, value: Value) -> String {
    error(claudeai::titles(&k.filters, &export(value), &[], None))
}
fn rust_mine(k: &Kit, value: Value) -> persona_builder::error::Result<claudeai::MinedExport> {
    let rows = claudeai::read_review("r.tsv", all_marked(&value).as_bytes()).unwrap();
    claudeai::mine(&k.filters, &export(value), &rows, &Split::Holdout(0.15))
}

#[test]
fn shape_errors_name_the_place_and_never_the_words() {
    let k = Kit::new("claudeai-shapes");
    let parity = parity_tools_available();
    let date = "a date in the export isn't one Persona Builder can read: the export's shape changed, run persona-builder claudeai inspect";
    let cases: Vec<(&str, Value, &str)> = vec![
        (
            "not a list",
            json!({"uuid": SENTINEL}),
            "the export's conversations.json isn't a list of conversations",
        ),
        (
            "not an object",
            json!([SENTINEL]),
            "conversations.json: conversation 1 has an unexpected shape (conversation)",
        ),
        (
            "no created_at",
            json!([{"uuid": "c", "name": SENTINEL, "chat_messages": []}]),
            "conversations.json: conversation 1 has an unexpected shape (created_at)",
        ),
        (
            "bad created_at",
            json!([conversation(
                json!("c"),
                json!("t"),
                json!(SENTINEL),
                json!([])
            )]),
            date,
        ),
        (
            "numeric created_at",
            json!([conversation(json!("c"), json!("t"), json!(5), json!([]))]),
            date,
        ),
        (
            "bad updated_at",
            json!([good(), {"uuid": "c-2", "name": "t", "created_at": "2026-03-01", "updated_at": SENTINEL, "chat_messages": []}]),
            date,
        ),
        (
            "null chat_messages",
            json!([conversation(
                json!("c"),
                json!(SENTINEL),
                json!("2026-03-01"),
                Value::Null
            )]),
            "conversations.json: conversation 1 has an unexpected shape (chat_messages)",
        ),
        (
            "message not an object",
            json!([
                good(),
                conversation(
                    json!("c-2"),
                    json!("t"),
                    json!("2026-03-01"),
                    json!([SENTINEL])
                )
            ]),
            "conversations.json: conversation 2, message 1 has an unexpected shape (message)",
        ),
        (
            "numeric name",
            json!([conversation(
                json!("c"),
                json!(5),
                json!("2026-03-01"),
                json!([])
            )]),
            "conversations.json: conversation 1 has an unexpected shape (name)",
        ),
        (
            "list name",
            json!([conversation(
                json!("c"),
                json!([SENTINEL]),
                json!("2026-03-01"),
                json!([])
            )]),
            "conversations.json: conversation 1 has an unexpected shape (name)",
        ),
        (
            "numeric block text",
            json!([conversation(
                json!("c"),
                json!(""),
                json!("2026-03-01"),
                json!([{"sender": "human", "content": [{"type": "text", "text": 5}]}])
            )]),
            "conversations.json: conversation 1, message 1 has an unexpected shape (text)",
        ),
        (
            "numeric content",
            json!([conversation(
                json!("c"),
                json!(""),
                json!("2026-03-01"),
                json!([{"sender": "human", "content": 5}])
            )]),
            "conversations.json: conversation 1, message 1 has an unexpected shape (content)",
        ),
        (
            "numeric uuid",
            json!([conversation(
                json!(7),
                json!(SENTINEL),
                json!("2026-03-01"),
                json!([])
            )]),
            "conversations.json: conversation 1 has an unexpected shape (uuid)",
        ),
    ];
    for (label, value, message) in cases {
        assert_eq!(titles_error(&k, value.clone()), message, "{label}");
        if parity {
            let path = k.write_json("bad.json", &value);
            assert_eq!(k.python_titles(&path), Some(1), "python titles: {label}");
        }
    }
}

#[test]
fn mine_errors_where_the_python_does() {
    let k = Kit::new("claudeai-mine-errors");
    let cases: Vec<(&str, Value, &str)> = vec![
        (
            "message not an object",
            json!([conversation(
                json!("c"),
                json!("t"),
                json!("x"),
                json!([SENTINEL])
            )]),
            "conversations.json: conversation 1, message 1 has an unexpected shape (message)",
        ),
        (
            "kept text without a date",
            json!([conversation(
                json!("c"),
                json!("t"),
                json!("x"),
                json!([{"sender": "human", "text": SENTINEL}])
            )]),
            "conversations.json: conversation 1, message 1 has an unexpected shape (created_at)",
        ),
        (
            "kept text with a bad date",
            json!([conversation(
                json!("c"),
                json!("t"),
                json!("x"),
                json!([{"sender": "human", "text": "fine", "created_at": SENTINEL}])
            )]),
            "a date in the export isn't one Persona Builder can read: the export's shape changed, run persona-builder claudeai inspect",
        ),
        (
            "assistant numeric content",
            json!([conversation(
                json!("c"),
                json!("t"),
                json!("x"),
                json!([{"sender": "assistant", "content": 3}])
            )]),
            "conversations.json: conversation 1, message 1 has an unexpected shape (content)",
        ),
        (
            "null chat_messages",
            json!([conversation(
                json!("c"),
                json!("t"),
                json!("x"),
                Value::Null
            )]),
            "conversations.json: conversation 1 has an unexpected shape (chat_messages)",
        ),
    ];
    for (label, value, message) in cases {
        assert_eq!(error(rust_mine(&k, value.clone())), message, "{label}");
        if parity_tools_available() {
            let path = k.write_json("bad.json", &value);
            assert_eq!(
                k.python_mine(&path, &all_marked(&value)).status.code(),
                Some(1),
                "python: {label}"
            );
        }
    }
    // A conversation nobody marked is never read, so its odd fields are no error on either side.
    let unread = json!([{"uuid": "c-9", "chat_messages": SENTINEL}]);
    let rows = vec![row("other", "y")];
    let m = claudeai::mine(
        &k.filters,
        &export(unread.clone()),
        &rows,
        &Split::Holdout(0.15),
    )
    .unwrap();
    assert_eq!((m.read, m.unmarked), (0, 1));
    if parity_tools_available() {
        let path = k.write_json("unread.json", &unread);
        let py = k.python_mine(
            &path,
            &format!("{}\ny\t\t\t\t\t\tother\n", claudeai::REVIEW_HEADER),
        );
        assert!(py.status.success());
    }
}

#[test]
fn odd_types_the_python_tolerates_are_refused() {
    let k = Kit::new("claudeai-odd-types");
    // Difference 2: in `mine`, Python counts a numeric uuid as unmarked.
    let numeric = json!([conversation(
        json!(7),
        json!("t"),
        json!("2026-03-01"),
        json!([])
    )]);
    assert_eq!(
        error(claudeai::mine(
            &k.filters,
            &export(numeric.clone()),
            &[],
            &Split::Holdout(0.15)
        )),
        "conversations.json: conversation 1 has an unexpected shape (uuid)"
    );
    // Difference 2: Python iterates a string's characters, so "" gives no messages.
    let empty = json!([conversation(
        json!("c"),
        json!("t"),
        json!("2026-03-01"),
        json!("")
    )]);
    assert_eq!(
        titles_error(&k, empty.clone()),
        "conversations.json: conversation 1 has an unexpected shape (chat_messages)"
    );
    // Difference 3: a uuid with a tab or line break would break the review file.
    for bad in ["a\tb", "a\nb", "a\u{2028}b", "a\u{85}b", "a\rb"] {
        let value = json!([conversation(
            json!(bad),
            json!("t"),
            json!("2026-03-01"),
            json!([])
        )]);
        assert_eq!(
            titles_error(&k, value.clone()),
            "conversations.json: conversation 1 has a uuid Persona Builder can't list"
        );
    }
    if parity_tools_available() {
        let path = k.write_json("numeric.json", &numeric);
        let py = k.python_mine(&path, &format!("{}\n", claudeai::REVIEW_HEADER));
        assert!(py.status.success());
        assert!(String::from_utf8_lossy(&py.stdout).contains("skipped 0 not mine and 1 unmarked"));
        let path = k.write_json("empty.json", &empty);
        assert_eq!(k.python_titles(&path), Some(0));
        let path = k.write_json(
            "tab.json",
            &json!([conversation(
                json!("a\tb"),
                json!("t"),
                json!("2026-03-01"),
                json!([])
            )]),
        );
        assert_eq!(k.python_titles(&path), Some(0));
        let written = fs::read_to_string(k.path("py-review.tsv")).unwrap();
        assert_eq!(written.lines().nth(1).unwrap().split('\t').count(), 8);
    }
}

#[test]
fn review_file_errors() {
    let k = Kit::new("claudeai-review");
    let header = claudeai::REVIEW_HEADER;
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "empty",
            vec![],
            "r.tsv: not a review file (its first line must be mine created updated messages project title uuid)",
        ),
        (
            "wrong header",
            format!("mine\tuuid\n{SENTINEL}\n").into_bytes(),
            "r.tsv: not a review file (its first line must be mine created updated messages project title uuid)",
        ),
        (
            "short row",
            format!("{header}\n\ny\t{SENTINEL}\n").into_bytes(),
            "r.tsv:3: a row needs 7 columns",
        ),
        (
            "not UTF-8",
            format!("{header}\n")
                .into_bytes()
                .into_iter()
                .chain([0xff, b'\n'])
                .collect(),
            "r.tsv is not UTF-8 text",
        ),
    ];
    let good = k.write_json("good.json", &json!([good()]));
    for (label, bytes, message) in cases {
        assert_eq!(
            error(claudeai::read_review("r.tsv", &bytes)),
            message,
            "{label}"
        );
        if parity_tools_available() {
            let path = k.path("py-mine.tsv");
            fs::write(&path, &bytes).unwrap();
            let py = k.python(&[
                "mine",
                "--export",
                good.to_str().unwrap(),
                "--review",
                path.to_str().unwrap(),
                "--out",
                k.path("o.jsonl").to_str().unwrap(),
            ]);
            assert_eq!(py.status.code(), Some(1), "python: {label}");
        }
    }
    let rows = claudeai::read_review(
        "r.tsv",
        format!("{header}\r\ny\ta\tb\tc\td\te\tf\textra\r\n \x1c\r\n").as_bytes(),
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].uuid, "f");
}

#[test]
fn zip_caps_and_odd_archives() {
    let k = Kit::new("claudeai-zips");
    let parity = parity_tools_available();
    let body = serde_json::to_string(&json!([good()])).unwrap();
    let read = |path: &Path| error(claudeai::read_export(path));
    let python_fails = |path: &Path| {
        if parity {
            assert_eq!(
                k.python_inspect(path).status.code(),
                Some(1),
                "python: {}",
                path.display()
            );
        }
    };
    // More than 100:1: both refuse before reading.
    let bomb = k.zip(
        "bomb.zip",
        json!([["conversations.json", " ".repeat(100_000), "deflate"]]),
    );
    assert_eq!(
        read(&bomb),
        "the export's conversations.json is compressed more than 100:1, so Persona Builder won't open it"
    );
    python_fails(&bomb);
    // A central directory forged to declare more than 2 GB.
    let forged = k.zip(
        "forged.zip",
        json!([["conversations.json", body, "deflate"]]),
    );
    let mut bytes = fs::read(&forged).unwrap();
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[central + 24..central + 28]
        .copy_from_slice(&(2u32 * 1024 * 1024 * 1024 + 1).to_le_bytes());
    fs::write(&forged, &bytes).unwrap();
    assert_eq!(
        read(&forged),
        "the export's conversations.json is bigger than the 2 GB cap"
    );
    python_fails(&forged);
    // A declared size smaller than the data.
    let short = k.zip(
        "short.zip",
        json!([["conversations.json", body, "deflate"]]),
    );
    let mut bytes = fs::read(&short).unwrap();
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[central + 24..central + 28].copy_from_slice(&10u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&10u32.to_le_bytes());
    fs::write(&short, &bytes).unwrap();
    let message = read(&short);
    assert!(
        message == "the export's conversations.json is bigger than the zip says"
            || message.starts_with("the export zip can't be read"),
        "{message}"
    );
    python_fails(&short);
    // None, and two, conversations.json.
    let none = k.zip("none.zip", json!([["users.json", "[]", "deflate"]]));
    assert_eq!(
        read(&none),
        "the export zip should hold one conversations.json, and it holds 0"
    );
    python_fails(&none);
    let two = k.zip(
        "two.zip",
        json!([
            ["a/conversations.json", body, "deflate"],
            ["b/./conversations.json", body, "deflate"]
        ]),
    );
    assert_eq!(
        read(&two),
        "the export zip should hold one conversations.json, and it holds 2"
    );
    python_fails(&two);
    // An empty archive.
    let empty = k.path("empty.zip");
    fs::write(&empty, b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0").unwrap();
    assert_eq!(
        read(&empty),
        "the export zip should hold one conversations.json, and it holds 0"
    );
    python_fails(&empty);
    // A CRC that doesn't match the data.
    let crc = k.zip("crc.zip", json!([["conversations.json", body, "stored"]]));
    let mut bytes = fs::read(&crc).unwrap();
    let at = bytes.windows(5).position(|w| w == b"patch").unwrap();
    bytes[at] = b'b';
    fs::write(&crc, &bytes).unwrap();
    assert!(read(&crc).starts_with("the export zip can't be read"));
    python_fails(&crc);
    // An encrypted entry: the flag set in both headers.
    let locked = k.zip(
        "locked.zip",
        json!([["conversations.json", body, "stored"]]),
    );
    let mut bytes = fs::read(&locked).unwrap();
    bytes[6] |= 1;
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[central + 8] |= 1;
    fs::write(&locked, &bytes).unwrap();
    assert!(read(&locked).starts_with("the export zip can't be read"));
    python_fails(&locked);
    // Difference 4: bzip2, and a duplicate name, which Python reads.
    let bz = k.zip("bz.zip", json!([["conversations.json", body, "bzip2"]]));
    assert!(read(&bz).starts_with("the export zip can't be read"));
    let duplicate = k.zip(
        "dup.zip",
        json!([
            ["conversations.json", body, "deflate"],
            ["users.json", "[]", "deflate"],
            ["users.json", "[]", "deflate"]
        ]),
    );
    let twice = "the export zip can't be read (invalid Zip archive: header inconsistency detected: duplicate file names in central directory)";
    assert_eq!(read(&duplicate), twice);
    let repeated = k.zip(
        "repeated.zip",
        json!([
            ["conversations.json", body, "deflate"],
            ["conversations.json", "[]", "deflate"]
        ]),
    );
    assert_eq!(read(&repeated), twice);
    python_fails(&repeated);
    if parity {
        for path in [&bz, &duplicate] {
            assert!(
                k.python_inspect(path).status.success(),
                "python: {}",
                path.display()
            );
        }
    }
    // A good zip: folder entries are not counted, every other entry is.
    let fine = k.zip(
        "fine.zip",
        json!([
            ["data/", "", "stored"],
            ["data/conversations.json", body, "deflate"],
            ["data/users.json", "[]", "deflate"]
        ]),
    );
    assert_eq!(claudeai::read_export(&fine).unwrap().files, 2);
}

#[test]
fn links_pipes_and_missing_paths() {
    let k = Kit::new("claudeai-paths");
    let body = serde_json::to_string(&json!([good()])).unwrap();
    let real = k.path("real.json");
    fs::write(&real, &body).unwrap();
    let link = k.path("link.json");
    symlink(&real, &link).unwrap();
    assert_eq!(
        error(claudeai::read_export(&link)),
        format!(
            "{} is a link: Persona Builder doesn't follow links. Use the export file itself",
            link.display()
        )
    );
    let folder = k.path("folder");
    fs::create_dir(&folder).unwrap();
    symlink(&real, folder.join("conversations.json")).unwrap();
    assert_eq!(
        error(claudeai::read_export(&folder)),
        format!(
            "{} is a link: Persona Builder doesn't follow links. Use the export file itself",
            folder.join("conversations.json").display()
        )
    );
    if parity_tools_available() {
        // Difference 4: Python follows both links.
        assert!(k.python_inspect(&link).status.success());
        assert!(k.python_inspect(&folder).status.success());
    }
    let empty = k.path("empty-folder");
    fs::create_dir(&empty).unwrap();
    assert_eq!(
        error(claudeai::read_export(&empty)),
        format!("{} has no conversations.json", empty.display())
    );
    let missing = k.path("missing.zip");
    assert_eq!(
        error(claudeai::read_export(&missing)),
        format!("{} doesn't exist", missing.display())
    );
    let pipe = k.path("pipe");
    assert!(
        Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        error(claudeai::read_export(&pipe)),
        format!("{} is not a file or a folder", pipe.display())
    );
}

#[test]
fn encodings_and_json_limits() {
    let k = Kit::new("claudeai-encodings");
    let parity = parity_tools_available();
    let body = serde_json::to_string(&json!([good()])).unwrap();
    let bom = format!("\u{feff}{body}");
    // Difference 5: a byte-order mark is skipped in every form; Python skips it only in a zip.
    let plain = k.path("bom.json");
    fs::write(&plain, &bom).unwrap();
    assert_eq!(claudeai::read_export(&plain).unwrap().files, 1);
    let zipped = k.zip("bom.zip", json!([["conversations.json", bom, "deflate"]]));
    assert!(claudeai::read_export(&zipped).is_ok());
    // UTF-16, which Python detects inside a zip.
    let utf16: String = body
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .map(char::from)
        .collect();
    let wide = k.zip(
        "wide.zip",
        json!([["conversations.json", format!("\u{ff}\u{fe}{utf16}"), "raw"]]),
    );
    assert_eq!(
        error(claudeai::read_export(&wide)),
        "the export's conversations.json is not UTF-8 text"
    );
    // Not UTF-8 at all, and not JSON: both refuse.
    let latin = k.path("latin.json");
    fs::write(&latin, b"[\"caf\xe9\"]").unwrap();
    assert_eq!(
        error(claudeai::read_export(&latin)),
        "the export's conversations.json is not UTF-8 text"
    );
    let broken = k.path("broken.json");
    fs::write(&broken, format!("[{{\"name\": \"{SENTINEL}\"")).unwrap();
    assert_eq!(
        error(claudeai::read_export(&broken)),
        "the export's conversations.json is not valid JSON"
    );
    let nan = k.path("nan.json");
    fs::write(&nan, body.replacen("{", "{\"x\": NaN, ", 1)).unwrap();
    assert_eq!(
        error(claudeai::read_export(&nan)),
        "the export's conversations.json is not valid JSON"
    );
    if parity {
        assert_eq!(k.python_inspect(&plain).status.code(), Some(1));
        assert!(k.python_inspect(&zipped).status.success());
        assert!(k.python_inspect(&wide).status.success());
        assert_eq!(k.python_inspect(&latin).status.code(), Some(1));
        assert_eq!(k.python_inspect(&broken).status.code(), Some(1));
        assert!(k.python_inspect(&nan).status.success());
    }
    // An integer past 64 bits reads as a float (Python says int); -0 as serde_json reads it.
    let numbers = k.path("numbers.json");
    fs::write(&numbers, "[{\"big\": 18446744073709551616, \"zero\": -0}]").unwrap();
    let text = claudeai::inspect(&claudeai::read_export(&numbers).unwrap())
        .unwrap()
        .text;
    assert!(text.contains("  .big: float (1/1)\n"), "{text}");
    assert!(text.contains("  .zero: float (1/1)\n"), "{text}");
}

#[test]
fn inspect_shows_field_names_never_values() {
    let _k = Kit::new("claudeai-inspect");
    let value = json!([conversation(
        json!(format!("{SENTINEL}-uuid")),
        json!(format!("{SENTINEL} title")),
        json!("2026-03-01T09:00:00Z"),
        json!([human("2026-03-01T09:00:00Z", SENTINEL), {"sender": "assistant", "created_at": "2026-03-01", "content": [{"type": "text", "text": SENTINEL}]}])
    )]);
    let inspection = claudeai::inspect(&export(value)).unwrap();
    assert!(inspection.missing.is_empty());
    assert!(!inspection.text.contains(SENTINEL));
    assert!(!inspection.text.contains("2026"));
    assert!(
        inspection
            .text
            .contains("  .chat_messages[].sender: str (2/2) values: assistant, human\n")
    );
    assert_eq!(
        error(claudeai::inspect(&export(json!({"a": 1})))),
        "the export's conversations.json isn't a list of conversations"
    );
}

#[test]
fn title_patterns_the_regex_crate_cannot_read_are_refused() {
    for pattern in ["(?=x)", r"(a)\1", r"x\Z", "("] {
        assert_eq!(
            error(claudeai::title_pattern(pattern)),
            "--title is not a valid pattern, or it uses look-around or a backreference, which Persona Builder can't read"
        );
    }
    assert!(claudeai::title_pattern("(?i)acme").is_ok());
}

#[test]
fn holdout_is_checked() {
    let k = Kit::new("claudeai-holdout");
    for h in [-0.1, 1.5, f64::NAN] {
        assert_eq!(
            error(claudeai::mine(
                &k.filters,
                &export(json!([])),
                &[],
                &Split::Holdout(h)
            )),
            "--holdout must be a number from 0 to 1"
        );
    }
}

#[test]
fn inspect_shows_only_message_senders_and_escapes_control_characters() {
    let value = json!([{"uuid": "c", "name": "t", "created_at": "2026-03-01", "bad\u{1b}[2Jkey": 1,
        "chat_messages[].sender": SENTINEL, "chat_messages[]": {"sender": SENTINEL},
        "chat_messages": [{"sender": "human\u{7}", "created_at": "2026-03-01", "text": "x",
            "content": [{"type": "tool_use", "input": {"sender": SENTINEL}}]}]}]);
    let text = claudeai::inspect(&export(value)).unwrap().text;
    assert!(!text.contains(SENTINEL) && !text.contains('\u{1b}') && !text.contains('\u{7}'));
    assert!(text.contains("  .bad\\u{1b}[2Jkey: int (1/1)\n"), "{text}");
    assert!(text.contains(" values: human\\u{7}\n"), "{text}");
    assert!(
        text.contains("  .chat_messages[].content[].input.sender: str (1/1)\n"),
        "{text}"
    );
}

#[test]
fn the_zip_itself_is_capped() {
    let k = Kit::new("claudeai-zip-caps");
    let big = k.path("big.zip");
    let file = fs::File::create(&big).unwrap();
    use std::io::Write;
    (&file).write_all(b"PK\x03\x04").unwrap();
    file.set_len(2 * 1024 * 1024 * 1024 + 1).unwrap();
    assert_eq!(
        error(claudeai::read_export(&big)),
        "the export zip is bigger than the 2 GB cap"
    );
    let many = k.path("many.zip");
    let out = Command::new("python3").arg("-I").arg("-c")
        .arg("import sys,zipfile\nwith zipfile.ZipFile(sys.argv[1],'w') as z:\n for i in range(100001): z.writestr(str(i),'')")
        .arg(&many).output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        error(claudeai::read_export(&many)),
        "the export zip holds more than 100,000 files, so Persona Builder won't open it"
    );
}
