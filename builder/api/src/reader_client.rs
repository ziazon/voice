//! The app reads history only through its own reader under the profile.
//! Nothing it prints is trusted beyond the protocol.
use crate::{
    dto::*,
    history::{self, failure},
    reader::{self, Step},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::Stdio,
};
use voice_agent::reader::{Reader, Source};
use voice_core::{assistants, deny::DenyList, member};

type Result<T> = std::result::Result<T, Failure>;

/// Where the app's reader runs: the shipped `voice` and the folder for its run folders.
pub struct Place<'a> {
    pub program: &'a Path,
    pub runs: &'a Path,
}

enum Final<T> {
    Result(T),
    Error(Failure),
}
impl<T> Final<T> {
    fn finish(self, code: Option<i32>) -> Result<T> {
        match (self, code) {
            (Self::Result(value), Some(0)) => Ok(value),
            (Self::Error(error), Some(1)) => Err(error),
            _ => Err(failure(reader::STOPPED)),
        }
    }
}
fn judge<T: DeserializeOwned>(
    mut input: impl BufRead,
    step: Step,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<Final<T>> {
    let stopped = || failure(reader::STOPPED);
    let mut final_line = None;
    let mut total = 0;
    loop {
        let mut bytes = Vec::new();
        // Read at most one byte past the largest permitted line including its newline.
        let size = (&mut input)
            .take((reader::MAX_LINE + 66) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| stopped())?;
        if size == 0 {
            break;
        }
        total += size;
        if total > reader::MAX_LINE + (1 << 20)
            || bytes.last() != Some(&b'\n')
            || size - 1 > reader::MAX_LINE + 64
            || final_line.is_some()
        {
            return Err(stopped());
        }
        let line: reader::Line<T> = serde_json::from_slice(&bytes).map_err(|_| stopped())?;
        match line {
            reader::Line::Progress(value) => {
                if !matches!(step, Step::ReadClaudeCode | Step::ReadClaudeai) {
                    return Err(stopped());
                }
                progress(value);
            }
            reader::Line::Result(value) => final_line = Some(Final::Result(value)),
            reader::Line::Error(message) => {
                if message.len() > 4096 || message.chars().any(char::is_control) {
                    return Err(stopped());
                }
                final_line = Some(Final::Error(Failure { message }));
            }
        }
    }
    final_line.ok_or_else(stopped)
}

fn run<T: DeserializeOwned>(
    home: &Path,
    place: &Place<'_>,
    slug: &str,
    step: Step,
    request: &impl Serialize,
    sources: &[Source<'_>],
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<T> {
    let source_error = match step {
        Step::ClaudeCodeFolders => history::FOLDERS,
        Step::ReadClaudeCode => history::READ,
        _ => history::EXPORT_BAD,
    };
    let member = assistants::find(home, slug)
        .map_err(|_| failure(history::FIND))?
        .ok_or_else(|| failure(history::FIND))?;
    let state = member::private_state_dir(&member.member, home)
        .ok_or_else(|| failure(history::ELSEWHERE))?;
    let deny = DenyList::for_member(&member.member, home).map_err(|e| failure(&e.to_string()))?;
    let reader = Reader::new(
        place.program,
        home,
        &member.dir,
        &state,
        place.runs,
        sources,
        &deny,
    )
    .map_err(|e| {
        failure(if e.to_string() == voice_agent::text::READER_SOURCE {
            source_error
        } else {
            reader::STOPPED
        })
    })?;
    let bytes = serde_json::to_vec(request).map_err(|_| failure(reader::STOPPED))?;
    let mut child = reader
        .command(&["reader", step.name()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| failure(reader::STOPPED))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&bytes);
    }
    let judged = match child.stdout.take() {
        Some(stdout) => judge(BufReader::new(stdout), step, progress),
        None => Err(failure(reader::STOPPED)),
    };
    if judged.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().map_err(|_| failure(reader::STOPPED))?;
    judged?.finish(status.code())
}

pub fn claude_code_folders(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
) -> Result<ClaudeCodeFolders> {
    history::readable(home, &request.slug)?;
    let projects = home.join(".claude/projects");
    match fs::symlink_metadata(&projects) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return history::claude_code_folders_in(home, request);
        }
        Err(_) => return Err(failure(history::FOLDERS)),
        Ok(_) => (),
    }
    run(
        home,
        place,
        &request.slug,
        Step::ClaudeCodeFolders,
        &request,
        &[Source::Folder(&projects)],
        &mut |_| {},
    )
}
pub fn read_claude_code(
    home: &Path,
    place: &Place<'_>,
    request: ReadRequest,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<ReadCounts> {
    history::readable(home, &request.slug)?;
    let projects = home.join(".claude/projects");
    match fs::symlink_metadata(&projects) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return history::read_claude_code_in(home, request, progress);
        }
        Err(_) => return Err(failure(history::READ)),
        Ok(_) => (),
    }
    run(
        home,
        place,
        &request.slug,
        Step::ReadClaudeCode,
        &request,
        &[Source::Folder(&projects)],
        progress,
    )
}
fn export_step<T: DeserializeOwned>(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
    step: Step,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<T> {
    let (path, folder) = history::chosen_export(home, &request.slug)?
        .ok_or_else(|| failure(history::EXPORT_GONE))?;
    let source = if folder {
        Source::Folder(&path)
    } else {
        Source::File(&path)
    };
    run(
        home,
        place,
        &request.slug,
        step,
        &request,
        &[source],
        progress,
    )
}
pub fn claudeai_export_status(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
) -> Result<Option<ExportStatus>> {
    history::readable(home, &request.slug)?;
    if history::chosen_export(home, &request.slug)?.is_none() {
        return Ok(None);
    }
    export_step(
        home,
        place,
        request,
        Step::ClaudeaiExportStatus,
        &mut |_| {},
    )
}
pub fn list_claudeai_chats(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
) -> Result<ChatList> {
    history::readable(home, &request.slug)?;
    export_step(home, place, request, Step::ListClaudeaiChats, &mut |_| {})
}
pub fn read_claudeai(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<ClaudeaiCounts> {
    history::readable(home, &request.slug)?;
    export_step(home, place, request, Step::ReadClaudeai, progress)
}
pub fn use_claudeai_export(
    home: &Path,
    place: &Place<'_>,
    slug: &str,
    path: &Path,
    folder: bool,
) -> Result<ExportStatus> {
    history::readable(home, slug)?;
    history::choose_claudeai_export_in(home, slug, path, folder)?;
    claudeai_export_status(home, place, SlugRequest { slug: slug.into() })?
        .ok_or_else(|| failure(history::EXPORT_GONE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    fn stopped(bytes: &[u8], step: Step, code: i32) {
        let result = judge::<Option<ExportStatus>>(Cursor::new(bytes), step, &mut |_| {})
            .and_then(|line| line.finish(Some(code)));
        assert_eq!(result.unwrap_err().message, reader::STOPPED);
    }
    #[test]
    fn progress_is_relayed_before_the_result() {
        let mut progress = vec![];
        let result = judge::<ClaudeaiCounts>(Cursor::new(b"{\"progress\":{\"done\":1,\"total\":1}}\n{\"result\":{\"chats\":1,\"messages\":2,\"privateLeftOut\":0}}\n"), Step::ReadClaudeai, &mut |p| progress.push(p)).unwrap().finish(Some(0)).unwrap();
        assert_eq!(progress, vec![ReadProgress { done: 1, total: 1 }]);
        assert_eq!(
            result,
            ClaudeaiCounts {
                chats: 1,
                messages: 2,
                private_left_out: 0
            }
        );
    }
    #[test]
    fn a_line_over_the_cap_is_stopped() {
        let mut bytes = b"{\"result\":null}".to_vec();
        bytes.resize(reader::MAX_LINE + 64, b' ');
        bytes.push(b'\n');
        assert_eq!(
            judge::<Option<ExportStatus>>(
                Cursor::new(&bytes),
                Step::ClaudeaiExportStatus,
                &mut |_| {}
            )
            .unwrap()
            .finish(Some(0))
            .unwrap(),
            None
        );
        bytes.insert(bytes.len() - 1, b' ');
        stopped(&bytes, Step::ClaudeaiExportStatus, 0);
    }
    #[test]
    fn total_output_over_the_cap_is_stopped() {
        let mut bytes = vec![];
        let line = format!(
            "{{\"progress\":{{\"done\":0,\"total\":1}}}}{}\n",
            " ".repeat(1 << 19)
        );
        while bytes.len() <= reader::MAX_LINE + (1 << 20) {
            bytes.extend_from_slice(line.as_bytes());
        }
        bytes.extend_from_slice(b"{\"result\":null}\n");
        stopped(&bytes, Step::ReadClaudeai, 0);
    }
    macro_rules! violation {
        ($name:ident, $bytes:expr) => {
            #[test]
            fn $name() {
                stopped($bytes, Step::ClaudeaiExportStatus, 0);
            }
        };
    }
    violation!(non_json_is_stopped, b"private canary\n");
    violation!(
        two_keys_are_stopped,
        b"{\"result\":null,\"error\":\"canary\"}\n"
    );
    violation!(unknown_key_is_stopped, b"{\"canary\":null}\n");
    violation!(
        progress_on_a_non_read_is_stopped,
        b"{\"progress\":{\"done\":1,\"total\":1}}\n{\"result\":null}\n"
    );
    violation!(
        a_line_after_result_is_stopped,
        b"{\"result\":null}\n{\"result\":null}\n"
    );
    violation!(no_final_line_is_stopped, b"");
    violation!(no_newline_is_stopped, b"{\"result\":null}");
    violation!(a_wrong_result_dto_is_stopped, b"{\"result\":123}\n");
    #[test]
    fn an_error_with_control_characters_is_stopped() {
        for control in ['\n', '\r', '\0', '\u{85}'] {
            stopped(
                format!("{}\n", json!({"error":format!("canary{control}")})).as_bytes(),
                Step::ClaudeaiExportStatus,
                1,
            );
        }
    }
    #[test]
    fn an_error_over_4096_bytes_is_stopped() {
        stopped(
            format!("{}\n", json!({"error":"é".repeat(2049)})).as_bytes(),
            Step::ClaudeaiExportStatus,
            1,
        );
    }
    #[test]
    fn a_result_with_exit_one_is_stopped() {
        stopped(b"{\"result\":null}\n", Step::ClaudeaiExportStatus, 1);
    }
    #[test]
    fn an_error_with_exit_zero_is_stopped() {
        stopped(
            b"{\"error\":\"choose again\"}\n",
            Step::ClaudeaiExportStatus,
            0,
        );
    }
    #[test]
    fn an_error_with_exit_one_keeps_its_message() {
        let result = judge::<Option<ExportStatus>>(
            Cursor::new(b"{\"error\":\"choose again\"}\n"),
            Step::ClaudeaiExportStatus,
            &mut |_| {},
        )
        .unwrap()
        .finish(Some(1));
        assert_eq!(result.unwrap_err().message, "choose again");
    }
}
