use crate::filters::Filters;
use persona_builder::{
    error::{Error, Result},
    member::read_member,
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub struct Report {
    pub hits: Vec<String>,
    pub filters_differ: bool,
}
pub const ADVICE: &str = "move member-specific filter patterns to the gitignored private.json and copy filters from crew/_template/member.json";
fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
fn git(folder: &Path) -> Command {
    let mut command = Command::new("git");
    command.env_clear();
    for key in ["HOME", "PATH"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .args(["-c", "core.fsmonitor=false", "-C"])
        .arg(folder);
    command
}
fn walk(folder: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(folder).map_err(|e| io(folder, e))? {
        let entry = entry.map_err(|e| io(folder, e))?;
        let path = entry.path();
        paths.push(path.clone());
        if entry.file_type().map_err(|e| io(&path, e))?.is_dir() {
            walk(&path, paths)?;
        }
    }
    Ok(())
}
fn nul_paths(bytes: &[u8]) -> impl Iterator<Item = PathBuf> + '_ {
    bytes
        .split(|&b| b == 0)
        .filter(|b| !b.is_empty())
        .map(|b| PathBuf::from(std::ffi::OsStr::from_bytes(b)))
}
fn junk(path: &Path) -> bool {
    path.components().any(|part| {
        let part = part.as_os_str().as_bytes();
        [
            b".git".as_slice(),
            b".DS_Store",
            b"__MACOSX",
            b"Thumbs.db",
            b"desktop.ini",
        ]
        .contains(&part)
            || part.starts_with(b"._")
    })
}

/// Tracked files of a clone, otherwise a walk excluding ignored paths and OS junk.
pub fn tracked_or_walked(folder: &Path) -> Result<Vec<PathBuf>> {
    let folder = folder.canonicalize().map_err(|e| io(folder, e))?;
    let tracked = git(&folder)
        .args(["ls-files", "-z", "--", "."])
        .output()
        .map_err(|e| io(&folder, e))?;
    let paths = if tracked.status.success() {
        nul_paths(&tracked.stdout)
            .map(|p| folder.join(p))
            .collect::<Vec<_>>()
    } else {
        let mut paths = vec![];
        walk(&folder, &mut paths)?;
        let mut child = git(&folder)
            .args(["check-ignore", "--no-index", "-z", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| io(&folder, e))?;
        let mut input = Vec::new();
        for path in &paths {
            input.extend_from_slice(path.as_os_str().as_bytes());
            input.push(0);
        }
        // Write concurrently so a long ignore result cannot fill stdout and deadlock stdin.
        let mut stdin = child.stdin.take().expect("piped stdin");
        let output = std::thread::scope(|scope| {
            let writer = scope.spawn(move || stdin.write_all(&input));
            let output = child.wait_with_output();
            let written = writer.join().expect("ignore writer thread");
            if let Err(error) = written
                && error.kind() != std::io::ErrorKind::BrokenPipe
            {
                return Err(error);
            }
            output
        })
        .map_err(|e| io(&folder, e))?;
        let ignored: BTreeSet<_> = if matches!(output.status.code(), Some(0 | 1)) {
            nul_paths(&output.stdout).collect()
        } else {
            BTreeSet::new()
        };
        paths.retain(|p| {
            !ignored.contains(p) && !junk(p.strip_prefix(&folder).expect("walked child"))
        });
        paths
    };
    let mut files = vec![];
    for path in paths {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                files.push(path.strip_prefix(&folder).expect("listed child").to_owned())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(&path, e)),
        }
    }
    files.sort();
    Ok(files)
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| io(path, e))?;
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).map_err(|e| io(path, e))?;
    Ok(bytes)
}
fn raw_member(path: &Path) -> Result<Value> {
    serde_json::from_slice(&read(path)?)
        .map_err(|_| Error::Invalid(format!("{}: member.json is not JSON", path.display())))
}
fn string_hits(value: &Value, path: &str, filters: &Filters, hits: &mut Vec<String>) {
    let mut add = |text: &str, path: &str, key: bool| {
        if let Some((_, kind)) = filters.lines(text).first() {
            hits.push(format!(
                "member.json:{path} ({}{})",
                if key { "a key, " } else { "" },
                kind.as_str()
            ));
        }
    };
    match value {
        Value::String(text) => add(text, path, false),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                string_hits(item, &format!("{path}[{i}]"), filters, hits);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                if let Some((_, kind)) = filters.lines(key).first() {
                    hits.push(format!(
                        "member.json:{} (a key, {})",
                        if path.is_empty() { "(top level)" } else { path },
                        kind.as_str()
                    ));
                }
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                string_hits(value, &child, filters, hits);
            }
        }
        _ => {}
    }
}

/// Check exactly the supplied files; member configuration is inspected separately from its filters.
pub fn scan(folder: &Path, files: &[PathBuf], filters: &Filters) -> Result<Report> {
    let mut report = Report {
        hits: vec![],
        filters_differ: false,
    };
    let mut files = files.to_vec();
    files.sort();
    for relative in &files {
        if relative == Path::new("member.json") || relative == Path::new("private.json") {
            continue;
        }
        let bytes = read(&folder.join(relative))?;
        let name = relative.display();
        match std::str::from_utf8(&bytes) {
            Err(_) => report.hits.push(format!("{name} (not text)")),
            Ok(text) => {
                for (line, kind) in filters.lines(text) {
                    report
                        .hits
                        .push(format!("{name}:{line} ({})", kind.as_str()));
                }
            }
        }
    }
    if files.iter().any(|p| p == Path::new("member.json")) {
        let mut config = raw_member(&folder.join("member.json"))?;
        let object = config.as_object_mut().ok_or_else(|| {
            Error::Invalid(format!(
                "{}/member.json: member.json must be a JSON object",
                folder.display()
            ))
        })?;
        let own = object.remove("filters").unwrap_or(Value::Null);
        string_hits(&config, "", filters, &mut report.hits);
        let template: Value =
            serde_json::from_str(include_str!("../../../crew/_template/member.json"))
                .expect("compiled template JSON");
        let template = template["filters"].as_object().expect("template filters");
        let own = own.as_object();
        let keys: BTreeSet<_> = template
            .keys()
            .chain(own.into_iter().flat_map(|o| o.keys()))
            .collect();
        for key in keys {
            if template.get(key) != own.and_then(|o| o.get(key)) {
                report.hits.push(format!(
                    "member.json:filters.{key} (differs from crew/_template)"
                ));
                report.filters_differ = true;
            }
        }
    }
    Ok(report)
}

pub fn privacy_check(folder: &Path) -> Result<Report> {
    let folder = folder.canonicalize().map_err(|e| io(folder, e))?;
    let files = tracked_or_walked(&folder)?;
    let path = folder.join("member.json");
    if !path.is_file() {
        return Err(Error::Invalid(format!(
            "{}: no member.json to check",
            path.display()
        )));
    }
    if !raw_member(&path)?.is_object() {
        return Err(Error::Invalid(format!(
            "{}: member.json must be a JSON object",
            path.display()
        )));
    }
    let member = read_member(&path)?;
    scan(&folder, &files, &Filters::compile(&member.filters)?)
}
pub fn render(report: &Report) -> String {
    let mut output = String::new();
    for hit in &report.hits {
        output.push_str(hit);
        output.push('\n');
    }
    if report.filters_differ {
        output.push_str(ADVICE);
        output.push('\n');
    }
    if report.hits.is_empty() {
        output.push_str("clean\n");
    } else {
        output.push_str(&format!("{} hits\n", report.hits.len()));
    }
    output
}
