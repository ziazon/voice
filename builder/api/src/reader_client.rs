//! The app reads history and skills only through its own reader under the profile.
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
    sync::mpsc,
    thread,
    time::Duration,
};
use voice_agent::{
    reader::{Reader, Source},
    spawn::Spawned,
};
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
const QUIET: Duration = Duration::from_secs(10 * 60);
const AFTER: Duration = Duration::from_secs(5);

struct Judge<'a, T> {
    step: Step,
    progress: &'a mut dyn FnMut(ReadProgress),
    final_line: Option<Final<T>>,
}
impl<T: DeserializeOwned> Judge<'_, T> {
    fn line(&mut self, bytes: &[u8]) -> Result<()> {
        let stopped = || failure(reader::STOPPED);
        if bytes.last() != Some(&b'\n') || self.final_line.is_some() {
            return Err(stopped());
        }
        let line: reader::Line<T> = serde_json::from_slice(bytes).map_err(|_| stopped())?;
        match line {
            reader::Line::Progress(value) => {
                if !matches!(self.step, Step::ReadClaudeCode | Step::ReadClaudeai) {
                    return Err(stopped());
                }
                (self.progress)(value);
            }
            reader::Line::Result(value) => self.final_line = Some(Final::Result(value)),
            reader::Line::Error(message) => {
                let message = if [
                    crate::calendar::NONE,
                    crate::calendar::GONE,
                    crate::calendar::BAD,
                    crate::calendar::NOT_ICS,
                    crate::calendar::TOO_MANY,
                    crate::calendar::TOO_BIG,
                    crate::calendar::ZONE,
                    crate::calendar::LOG,
                    crate::calendar::BAD_GRANTS,
                    crate::calendar::NO_STATE,
                    crate::calendar::MAC_ONLY_CALENDAR,
                    &voice_agent::text::no_grant(voice_agent::grants::Capability::CalendarRead),
                    history::FIND,
                    history::ELSEWHERE,
                    history::PERMISSION,
                    history::BUSY,
                    history::SAVE,
                    history::FOLDERS,
                    history::READ,
                    history::EXPORT_GONE,
                    history::EXPORT_BAD,
                    history::NO_CHATS,
                    crate::skills::SKILLS_READ,
                    crate::skills::SKILLS_SAVE,
                    crate::skills::GONE,
                    crate::skills::NONE,
                    crate::skills::REFRESH,
                    crate::skills::REFRESH_STALE,
                    crate::skills::REFRESH_REMOVED,
                    reader::REQUEST,
                    reader::TOO_MUCH,
                    reader::STOPPED,
                ]
                .contains(&message.as_str())
                {
                    message
                } else {
                    step_failure(self.step).into()
                };
                self.final_line = Some(Final::Error(Failure { message }));
            }
        }
        Ok(())
    }
    fn end(self) -> Result<Final<T>> {
        self.final_line.ok_or_else(|| failure(reader::STOPPED))
    }
}

enum Output {
    Line(Vec<u8>),
    End,
    Failed,
}
fn read_output(input: impl Read, lines: mpsc::Sender<Output>) {
    let mut input = BufReader::new(input);
    let mut total = 0;
    loop {
        let mut bytes = Vec::new();
        // Read at most one byte past the largest permitted line including its newline.
        let size = (&mut input)
            .take((reader::MAX_LINE + 66) as u64)
            .read_until(b'\n', &mut bytes);
        let message = match size {
            Ok(0) => Output::End,
            Ok(size) => {
                total += size;
                if total > reader::MAX_LINE + (1 << 20) || size - 1 > reader::MAX_LINE + 64 {
                    Output::Failed
                } else {
                    Output::Line(bytes)
                }
            }
            Err(_) => Output::Failed,
        };
        let finished = !matches!(message, Output::Line(_));
        if lines.send(message).is_err() || finished {
            return;
        }
    }
}
fn judge_with_deadlines<T: DeserializeOwned>(
    input: impl Read + Send + 'static,
    step: Step,
    progress: &mut dyn FnMut(ReadProgress),
    quiet: Duration,
    after: Duration,
    mut kill: impl FnMut(),
) -> Result<Final<T>> {
    let (send, receive) = mpsc::channel();
    let reading = thread::spawn(move || read_output(input, send));
    let mut judge = Judge {
        step,
        progress,
        final_line: None,
    };
    loop {
        let deadline = if judge.final_line.is_some() {
            after
        } else {
            quiet
        };
        match receive.recv_timeout(deadline) {
            Ok(Output::Line(bytes)) if judge.line(&bytes).is_ok() => (),
            Ok(Output::End) => {
                if reading.join().is_ok() {
                    match judge.end() {
                        Ok(answer) => return Ok(answer),
                        Err(error) => {
                            kill();
                            return Err(error);
                        }
                    }
                }
                kill();
                return Err(failure(reader::STOPPED));
            }
            _ => {
                // Another process may still hold the pipe: never join after a kill.
                kill();
                return Err(failure(reader::STOPPED));
            }
        }
    }
}

fn step_failure(step: Step) -> &'static str {
    match step {
        Step::ClaudeCodeFolders => history::FOLDERS,
        Step::ReadClaudeCode => history::READ,
        Step::Skills => crate::skills::SKILLS_READ,
        Step::ReadCalendar => crate::calendar::BAD,
        _ => history::EXPORT_BAD,
    }
}

fn wait_for_exit(child: &mut Spawned, limit: Duration) -> Result<std::process::ExitStatus> {
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < limit => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(failure(reader::STOPPED));
            }
        }
    }
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
    let source_error = step_failure(step);
    let member = assistants::find(home, slug)
        .map_err(|_| failure(history::FIND))?
        .ok_or_else(|| failure(history::FIND))?;
    let state = member::private_state_dir(&member.member, home)
        .ok_or_else(|| failure(history::ELSEWHERE))?;
    let deny =
        DenyList::for_member(&member.member, home).map_err(|_| failure(step_failure(step)))?;
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
    let child = reader
        .spawn(&["reader", step.name()])
        .map_err(|_| failure(reader::STOPPED))?;
    supervise(child, &bytes, step, progress, QUIET, AFTER)
}

fn supervise<T: DeserializeOwned>(
    mut child: Spawned,
    request: &[u8],
    step: Step,
    progress: &mut dyn FnMut(ReadProgress),
    quiet: Duration,
    after: Duration,
) -> Result<T> {
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(request);
    }
    let judged = match child.stdout.take() {
        Some(stdout) => judge_with_deadlines(stdout, step, progress, quiet, after, || {
            let _ = child.kill();
        }),
        None => {
            let _ = child.kill();
            Err(failure(reader::STOPPED))
        }
    };
    let status = wait_for_exit(&mut child, after)?;
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
            return Ok(ClaudeCodeFolders {
                found: false,
                folders: vec![],
            });
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
            let (_member, _lock) = history::locked(home, &request.slug)?;
            return Ok(history::empty_read_counts());
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
    path: &Path,
    folder: bool,
) -> Result<T> {
    let source = if folder {
        Source::Folder(path)
    } else {
        Source::File(path)
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
    let member = history::readable(home, &request.slug)?;
    let Some((path, folder)) = history::chosen_export(home, &request.slug)? else {
        return Ok(None);
    };
    history::export_allowed(&member, home, &path, folder)?;
    export_step(
        home,
        place,
        request,
        Step::ClaudeaiExportStatus,
        &mut |_| {},
        &path,
        folder,
    )
}
pub fn list_claudeai_chats(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
) -> Result<ChatList> {
    let member = history::readable(home, &request.slug)?;
    let (path, folder) = history::chosen_export(home, &request.slug)?
        .ok_or_else(|| failure(history::EXPORT_GONE))?;
    history::export_allowed(&member, home, &path, folder)?;
    export_step(
        home,
        place,
        request,
        Step::ListClaudeaiChats,
        &mut |_| {},
        &path,
        folder,
    )
}
pub fn read_claudeai(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<ClaudeaiCounts> {
    let member = history::readable(home, &request.slug)?;
    let (path, folder) = history::chosen_export(home, &request.slug)?
        .ok_or_else(|| failure(history::EXPORT_GONE))?;
    history::export_allowed(&member, home, &path, folder)?;
    export_step(
        home,
        place,
        request,
        Step::ReadClaudeai,
        progress,
        &path,
        folder,
    )
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

pub fn read_calendar(
    home: &Path,
    place: &Place<'_>,
    slug: &str,
    step: &str,
) -> Result<CalendarRead> {
    crate::calendar::read_calendar_through(
        home,
        slug,
        step,
        std::time::SystemTime::now(),
        |request| {
            let member = crate::calendar::member_in(home, slug)?;
            let (path, folder) = crate::calendar::chosen_export(home, slug)?
                .ok_or_else(|| failure(crate::calendar::NONE))?;
            crate::calendar::export_allowed(home, &member, &path)?;
            let source = if folder {
                Source::Folder(&path)
            } else {
                Source::File(&path)
            };
            run(
                home,
                place,
                slug,
                Step::ReadCalendar,
                request,
                &[source],
                &mut |_| {},
            )
        },
    )
}

fn skills_run<T: DeserializeOwned>(
    home: &Path,
    place: &Place<'_>,
    request: reader::SkillsStep,
) -> Result<T> {
    let member = assistants::find(home, &request.slug)
        .map_err(|_| failure(history::FIND))?
        .ok_or_else(|| failure(history::FIND))?;
    let state = member::private_state_dir(&member.member, home)
        .ok_or_else(|| failure(history::ELSEWHERE))?;
    let deny = DenyList::for_member(&member.member, home)
        .map_err(|_| failure(crate::skills::SKILLS_READ))?;
    let rules = voice_ingest::anonymize::Rules::for_member(&member.member, home)
        .map_err(|_| failure(crate::skills::SKILLS_READ))?;
    let paths = voice_ingest::skills::sources(home, &rules);
    let sources = paths
        .iter()
        .map(|(path, folder)| {
            if *folder {
                Source::Folder(path)
            } else {
                Source::File(path)
            }
        })
        .collect::<Vec<_>>();
    let usable =
        voice_agent::reader::usable(home, &member.dir, &state, place.runs, &sources, &deny);
    run(
        home,
        place,
        &request.slug,
        Step::Skills,
        &request,
        &usable,
        &mut |_| {},
    )
}
pub fn list_skills(home: &Path, place: &Place<'_>, request: SlugRequest) -> Result<Skills> {
    skills_run(
        home,
        place,
        reader::SkillsStep {
            slug: request.slug,
            action: reader::SkillsAction::List,
            ids: vec![],
        },
    )
    .map(crate::skills::checked)
}
pub fn agree_skills(home: &Path, place: &Place<'_>, request: SkillsRequest) -> Result<Skills> {
    skills_run(
        home,
        place,
        reader::SkillsStep {
            slug: request.slug,
            action: reader::SkillsAction::Agree,
            ids: request.ids,
        },
    )
    .map(crate::skills::checked)
}
pub fn withdraw_skills(
    home: &Path,
    place: &Place<'_>,
    request: SkillsRequest,
) -> Result<SkillsWithdrawn> {
    skills_run::<SkillsWithdrawn>(
        home,
        place,
        reader::SkillsStep {
            slug: request.slug,
            action: reader::SkillsAction::Withdraw,
            ids: request.ids,
        },
    )
    .map(|mut result| {
        result.skills = crate::skills::checked(result.skills);
        result
    })
}
pub fn refresh_skills(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
) -> Result<SkillsRefreshed> {
    skills_run(
        home,
        place,
        reader::SkillsStep {
            slug: request.slug,
            action: reader::SkillsAction::Refresh,
            ids: vec![],
        },
    )
}
pub fn refresh_held(
    home: &Path,
    place: &Place<'_>,
    request: SlugRequest,
) -> Result<SkillsRefreshed> {
    skills_run(
        home,
        place,
        reader::SkillsStep {
            slug: request.slug,
            action: reader::SkillsAction::RefreshHeld,
            ids: vec![],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[cfg(target_os = "macos")]
    fn shell(script: &str) -> Spawned {
        voice_agent::spawn::spawn(
            Path::new("/bin/sh"),
            &["-c".into(), script.into()],
            &[("PATH".into(), "/usr/bin:/bin".into())],
            &std::env::temp_dir(),
        )
        .unwrap()
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_reader_that_does_not_exit_is_killed() {
        let mut child = shell("exec /bin/sleep 5");
        let started = std::time::Instant::now();
        let result = wait_for_exit(&mut child, Duration::from_millis(100));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(result.unwrap_err().message, reader::STOPPED);
        assert!(!child.try_wait().unwrap().unwrap().success());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_reader_that_exits_returns_its_status() {
        let mut child = shell("exec /usr/bin/true");
        assert!(
            wait_for_exit(&mut child, Duration::from_secs(5))
                .unwrap()
                .success()
        );
    }

    fn judge<T: DeserializeOwned>(
        input: impl Read + Send + 'static,
        step: Step,
        progress: &mut dyn FnMut(ReadProgress),
    ) -> Result<Final<T>> {
        judge_with_deadlines(input, step, progress, QUIET, AFTER, || {})
    }

    fn stopped(bytes: &[u8], step: Step, code: i32) {
        let result = judge::<Option<ExportStatus>>(Cursor::new(bytes.to_vec()), step, &mut |_| {})
            .and_then(|line| line.finish(Some(code)));
        assert_eq!(result.unwrap_err().message, reader::STOPPED);
    }
    #[test]
    fn calendar_sentences_pass_through_and_unknown_errors_are_fixed() {
        use crate::calendar;
        for message in [
            calendar::NONE,
            calendar::GONE,
            calendar::BAD,
            calendar::NOT_ICS,
            calendar::TOO_MANY,
            calendar::TOO_BIG,
            calendar::ZONE,
            calendar::LOG,
            calendar::BAD_GRANTS,
            calendar::NO_STATE,
            calendar::MAC_ONLY_CALENDAR,
            &voice_agent::text::no_grant(voice_agent::grants::Capability::CalendarRead),
        ] {
            refusal(message, message, Step::ReadCalendar);
        }
        refusal("private canary path", calendar::BAD, Step::ReadCalendar);
        stopped(
            b"{\"progress\":{\"done\":1,\"total\":1}}\n",
            Step::ReadCalendar,
            0,
        );
    }

    #[test]
    fn skills_notes_from_a_reader_keep_only_the_known_sentences() {
        use voice_ingest::skills::{PLUGIN_NOTE, SYNCED_TWICE_NOTE, UNREADABLE_NOTE};
        let bytes = format!(
            "{}\n",
            json!({"result": {"skills": [], "notes": [UNREADABLE_NOTE, "An invented private sentence.", SYNCED_TWICE_NOTE, PLUGIN_NOTE]}})
        );
        let answer = judge::<Skills>(Cursor::new(bytes.into_bytes()), Step::Skills, &mut |_| {})
            .unwrap()
            .finish(Some(0))
            .map(crate::skills::checked)
            .unwrap();
        assert_eq!(
            answer.notes,
            vec![UNREADABLE_NOTE, SYNCED_TWICE_NOTE, PLUGIN_NOTE]
        );
    }
    #[test]
    fn a_bad_deny_entry_uses_the_steps_fixed_sentence() {
        let home = std::env::temp_dir().join(format!("voice-reader-deny-{}", std::process::id()));
        fs::create_dir(&home).unwrap();
        crate::new_assistant_in(
            &home,
            NewAssistantRequest {
                name: "Ari".into(),
                pronouns: "they/them".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        let config = home.join(".claude/crew/ari/member.json");
        let mut member: serde_json::Value =
            serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
        member["filters"]["denied_paths"] = json!(["invented/private/entry"]);
        fs::write(config, serde_json::to_vec(&member).unwrap()).unwrap();
        let place = Place {
            program: Path::new("/usr/bin/true"),
            runs: &home.join("runs"),
        };
        let result = run::<Skills>(
            &home,
            &place,
            "ari",
            Step::Skills,
            &SlugRequest { slug: "ari".into() },
            &[],
            &mut |_| {},
        );
        fs::remove_dir_all(home).unwrap();
        assert_eq!(result.unwrap_err().message, crate::skills::SKILLS_READ);
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
                Cursor::new(bytes.clone()),
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
    fn refusal(message: &str, expected: &str, step: Step) {
        let bytes = format!("{}\n", json!({"error": message})).into_bytes();
        let result = judge::<Option<ExportStatus>>(Cursor::new(bytes.to_vec()), step, &mut |_| {})
            .unwrap()
            .finish(Some(1));
        assert_eq!(result.unwrap_err().message, expected);
    }
    #[test]
    fn an_error_with_control_characters_uses_the_step_sentence() {
        for control in ['\n', '\r', '\0', '\u{85}'] {
            refusal(
                &format!("canary{control}"),
                history::EXPORT_BAD,
                Step::ClaudeaiExportStatus,
            );
        }
    }
    #[test]
    fn an_error_over_4096_bytes_uses_the_step_sentence() {
        refusal(
            &"é".repeat(2049),
            history::EXPORT_BAD,
            Step::ClaudeaiExportStatus,
        );
    }
    #[test]
    fn unlisted_errors_use_each_steps_sentence() {
        for (step, expected) in [
            (Step::ClaudeCodeFolders, history::FOLDERS),
            (Step::ReadClaudeCode, history::READ),
            (Step::ClaudeaiExportStatus, history::EXPORT_BAD),
            (Step::ListClaudeaiChats, history::EXPORT_BAD),
            (Step::ReadClaudeai, history::EXPORT_BAD),
        ] {
            refusal("private canary path", expected, step);
        }
    }
    #[test]
    fn a_result_with_exit_one_is_stopped() {
        stopped(b"{\"result\":null}\n", Step::ClaudeaiExportStatus, 1);
    }
    #[test]
    fn an_error_with_exit_zero_is_stopped() {
        stopped(
            format!("{}\n", json!({"error": history::FIND})).as_bytes(),
            Step::ClaudeaiExportStatus,
            0,
        );
    }
    #[test]
    fn listed_errors_with_exit_one_keep_their_message() {
        for message in [
            history::FIND,
            history::ELSEWHERE,
            history::PERMISSION,
            history::BUSY,
            history::SAVE,
            history::FOLDERS,
            history::READ,
            history::EXPORT_GONE,
            history::EXPORT_BAD,
            history::NO_CHATS,
            reader::REQUEST,
            reader::TOO_MUCH,
            reader::STOPPED,
        ] {
            refusal(message, message, Step::ClaudeaiExportStatus);
        }
    }
    #[test]
    fn a_silent_reader_is_killed_after_the_quiet_deadline() {
        let (input, output) = std::io::pipe().unwrap();
        // A fallback closes the pipe even on the old, blocking implementation.
        let writer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            drop(output);
        });
        let mut kills = 0;
        let started = std::time::Instant::now();
        let result = judge_with_deadlines::<Option<ExportStatus>>(
            input,
            Step::ClaudeaiExportStatus,
            &mut |_| {},
            std::time::Duration::from_millis(50),
            std::time::Duration::from_millis(50),
            || kills += 1,
        );
        assert!(started.elapsed() < std::time::Duration::from_millis(400));
        writer.join().unwrap();
        assert_eq!(result.err().unwrap().message, reader::STOPPED);
        assert_eq!(kills, 1);
    }

    #[test]
    fn an_open_pipe_after_the_result_is_killed_after_the_after_deadline() {
        let (input, mut output) = std::io::pipe().unwrap();
        output.write_all(b"{\"result\":null}\n").unwrap();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            drop(output);
        });
        let mut kills = 0;
        let started = std::time::Instant::now();
        let result = judge_with_deadlines::<Option<ExportStatus>>(
            input,
            Step::ClaudeaiExportStatus,
            &mut |_| {},
            std::time::Duration::from_secs(2),
            std::time::Duration::from_millis(50),
            || kills += 1,
        );
        assert!(started.elapsed() < std::time::Duration::from_millis(400));
        writer.join().unwrap();
        assert_eq!(result.err().unwrap().message, reader::STOPPED);
        assert_eq!(kills, 1);
    }

    #[test]
    fn a_result_then_closed_output_does_not_kill() {
        let (input, mut output) = std::io::pipe().unwrap();
        output.write_all(b"{\"result\":null}\n").unwrap();
        drop(output);
        let mut kills = 0;
        let result = judge_with_deadlines::<Option<ExportStatus>>(
            input,
            Step::ClaudeaiExportStatus,
            &mut |_| {},
            std::time::Duration::from_millis(100),
            std::time::Duration::from_millis(100),
            || kills += 1,
        )
        .unwrap()
        .finish(Some(0))
        .unwrap();
        assert_eq!(result, None);
        assert_eq!(kills, 0);
    }

    #[test]
    fn progress_resets_the_quiet_deadline() {
        let (input, mut output) = std::io::pipe().unwrap();
        let writer = std::thread::spawn(move || {
            for done in 1..=8 {
                writeln!(
                    output,
                    "{}",
                    json!({"progress": {"done": done, "total": 8}})
                )
                .unwrap();
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            output.write_all(b"{\"result\":null}\n").unwrap();
        });
        let mut kills = 0;
        let mut progress = vec![];
        let result = judge_with_deadlines::<Option<ExportStatus>>(
            input,
            Step::ReadClaudeai,
            &mut |p| progress.push(p),
            std::time::Duration::from_millis(200),
            std::time::Duration::from_millis(100),
            || kills += 1,
        )
        .unwrap()
        .finish(Some(0))
        .unwrap();
        writer.join().unwrap();
        assert_eq!(result, None);
        assert_eq!(kills, 0);
        assert_eq!(
            progress,
            (1..=8)
                .map(|done| ReadProgress { done, total: 8 })
                .collect::<Vec<_>>()
        );
    }
    #[cfg(target_os = "macos")]
    fn reaped(pid: u32) {
        // SAFETY: signal zero only checks whether this pid exists.
        assert_eq!(unsafe { libc::kill(pid as libc::pid_t, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_silent_reader_is_killed_at_the_quiet_deadline() {
        let child = shell("exec /bin/sleep 30");
        let pid = child.id();
        let started = std::time::Instant::now();
        let result = supervise::<Option<ExportStatus>>(
            child,
            b"",
            Step::ClaudeaiExportStatus,
            &mut |_| {},
            Duration::from_millis(200),
            Duration::from_secs(3),
        );
        assert_eq!(result.unwrap_err().message, reader::STOPPED);
        assert!(started.elapsed() < Duration::from_millis(1500));
        reaped(pid);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_reader_that_closes_output_but_does_not_exit_is_killed() {
        let child = shell("printf '{\"result\":null}\n'; exec >&-; exec /bin/sleep 30");
        let pid = child.id();
        let started = std::time::Instant::now();
        let result = supervise::<Option<ExportStatus>>(
            child,
            b"",
            Step::ClaudeaiExportStatus,
            &mut |_| {},
            Duration::from_secs(3),
            Duration::from_millis(200),
        );
        assert_eq!(result.unwrap_err().message, reader::STOPPED);
        assert!(started.elapsed() < Duration::from_millis(1500));
        reaped(pid);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_panic_in_progress_kills_and_reaps_the_reader() {
        let child = shell("printf '{\"progress\":{\"done\":1,\"total\":2}}\n'; exec /bin/sleep 30");
        let pid = child.id();
        let started = std::time::Instant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            supervise::<ClaudeaiCounts>(
                child,
                b"",
                Step::ReadClaudeai,
                &mut |_| panic!("progress"),
                Duration::from_secs(3),
                Duration::from_millis(200),
            )
        }));
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_millis(1500));
        reaped(pid);
    }
}
