use crate::common::{Scratch, repo};
use serde_json::json;
use std::{
    fs,
    process::{Command, Output},
};

pub fn run(home: &Scratch, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_voice"))
        .env_clear()
        .env("HOME", &home.0)
        .current_dir(&home.0)
        .args(args)
        .output()
        .unwrap()
}
pub fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
pub fn setup(home: &Scratch, rules: serde_json::Value) {
    fs::create_dir(home.0.join("member")).unwrap();
    let mut member: serde_json::Value =
        serde_json::from_slice(&fs::read(repo().join("crew/_template/member.json")).unwrap())
            .unwrap();
    member["slug"] = json!("pebble");
    member["name"] = json!("Pebble");
    member["principal"] = json!("Rowan Vale");
    member["state_dir"] = json!("~/.claude/state/pebble");
    fs::write(home.0.join("member/member.json"), member.to_string()).unwrap();
    fs::write(
        home.0.join("member/private.json"),
        json!({"anonymizer":rules}).to_string(),
    )
    .unwrap();
}
pub fn skill(home: &Scratch, relative: &str, text: &str) {
    let folder = home.0.join(relative);
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("SKILL.md"), text).unwrap();
}
pub fn agree(home: &Scratch, ids: &[&str]) {
    let mut args = vec!["skills", "agree", "member"];
    args.extend(ids);
    success(run(home, &args));
}
pub fn sha256(path: &std::path::Path) -> String {
    let output = Command::new("python3")
        .args(["-I", "-c", "import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())"])
        .arg(path).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

pub fn check_manifest(out: &std::path::Path) -> serde_json::Value {
    let bytes = fs::read(out.join(".manifest.json")).unwrap();
    assert!(bytes.ends_with(b"\n"));
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(value["made_on"].as_str().unwrap().len(), 10);
    let mut files = vec![];
    fn walk(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), files);
            } else {
                files.push(entry.path());
            }
        }
    }
    walk(out, &mut files);
    let mut listed = vec![out.join(".manifest.json"), out.join(".anonymized-copy")];
    for (name, paths) in value["skills"].as_object().unwrap() {
        for (relative, hash) in paths.as_object().unwrap() {
            let path = out.join(name).join(relative);
            assert_eq!(hash.as_str().unwrap(), sha256(&path));
            listed.push(path);
        }
    }
    listed.sort();
    files.sort();
    assert_eq!(listed, files);
    value
}
