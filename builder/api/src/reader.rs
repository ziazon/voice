//! The protocol of Voice's reader, the hidden `voice reader <step>`; see
//! docs/spikes/VO11-os-sandbox.md.
use crate::{dto::*, history, json_object, skills};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};

pub const MAX_REQUEST: u64 = 64 * 1024;
pub const MAX_LINE: usize = 16 << 20;
pub const REQUEST: &str =
    "Voice's reader couldn't understand what it was asked to do. Close Voice and open it again.";
pub const STOPPED: &str = "Voice's reader stopped unexpectedly. Close Voice and open it again.";
pub const TOO_MUCH: &str = "Voice found more here than it can show at once.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    ClaudeCodeFolders,
    ReadClaudeCode,
    ClaudeaiExportStatus,
    ListClaudeaiChats,
    ReadClaudeai,
    Skills,
}
impl Step {
    pub const ALL: [Step; 6] = [
        Self::ClaudeCodeFolders,
        Self::ReadClaudeCode,
        Self::ClaudeaiExportStatus,
        Self::ListClaudeaiChats,
        Self::ReadClaudeai,
        Self::Skills,
    ];
    /// The word after `voice reader`.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCodeFolders => "claude-code-folders",
            Self::ReadClaudeCode => "read-claude-code",
            Self::ClaudeaiExportStatus => "claudeai-export-status",
            Self::ListClaudeaiChats => "list-claudeai-chats",
            Self::ReadClaudeai => "read-claudeai",
            Self::Skills => "skills",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillsAction {
    List,
    Agree,
    Withdraw,
    Refresh,
    #[serde(rename = "refresh-held")]
    RefreshHeld,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillsStep {
    pub slug: String,
    pub action: SkillsAction,
    #[serde(default)]
    pub ids: Vec<String>,
}
/// One line the reader prints: progress lines, then exactly one result or error line.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Line<T> {
    Progress(ReadProgress),
    Result(T),
    Error(String),
}

/// Answer one request: read it from `input`, do the step with the API's own checks, write JSON
/// lines to `output`. True when it ended with a result line.
pub fn serve(
    home: Option<&Path>,
    step: Step,
    input: &mut dyn Read,
    output: &mut dyn Write,
) -> bool {
    serve_with_limit(home, step, input, output, MAX_LINE)
}
struct Lines<'a> {
    output: &'a mut dyn Write,
    max_line: usize,
    alive: bool,
}
impl Lines<'_> {
    fn write<T: Serialize>(&mut self, line: Line<T>) -> bool {
        if !self.alive {
            return false;
        }
        let Ok(mut bytes) = serde_json::to_vec(&line) else {
            return false;
        };
        if bytes.len() > self.max_line {
            return false;
        }
        bytes.push(b'\n');
        self.alive = self
            .output
            .write_all(&bytes)
            .and_then(|()| self.output.flush())
            .is_ok();
        self.alive
    }
    fn error(&mut self, message: String) -> bool {
        if !self.write(Line::<()>::Error(message)) && self.alive {
            self.write(Line::<()>::Error(TOO_MUCH.into()));
        }
        false
    }
    fn result<T: Serialize>(&mut self, result: std::result::Result<T, Failure>) -> bool {
        match result {
            Ok(value) => {
                if self.write(Line::Result(value)) {
                    return true;
                }
                if self.alive {
                    self.error(TOO_MUCH.into());
                }
                false
            }
            Err(failure) => self.error(failure.message),
        }
    }
    fn progress(&mut self, progress: ReadProgress) {
        self.write(Line::<()>::Progress(progress));
    }
}
fn serve_with_limit(
    home: Option<&Path>,
    step: Step,
    input: &mut dyn Read,
    output: &mut dyn Write,
    max_line: usize,
) -> bool {
    let mut lines = Lines {
        output,
        max_line,
        alive: true,
    };
    guard(&mut lines, |lines| serve_step(home, step, input, lines))
}
fn serve_step(
    home: Option<&Path>,
    step: Step,
    input: &mut dyn Read,
    lines: &mut Lines<'_>,
) -> bool {
    let Some(home) = home else {
        return lines.error(history::FIND.into());
    };
    macro_rules! request {
        ($ty:ty) => {
            match json_object::read_object::<$ty>(input, MAX_REQUEST) {
                Ok(request) => request,
                Err(_) => return lines.error(REQUEST.into()),
            }
        };
    }
    match step {
        Step::ClaudeCodeFolders => {
            let request = request!(SlugRequest);
            lines.result(history::claude_code_folders_in(home, request))
        }
        Step::ReadClaudeCode => {
            let request = request!(ReadRequest);
            let result = history::read_claude_code_in(home, request, &mut |p| lines.progress(p));
            lines.result(result)
        }
        Step::ClaudeaiExportStatus => {
            let request = request!(SlugRequest);
            lines.result(history::claudeai_export_status_in(home, request))
        }
        Step::ListClaudeaiChats => {
            let request = request!(SlugRequest);
            lines.result(history::list_claudeai_chats_in(home, request))
        }
        Step::ReadClaudeai => {
            let request = request!(SlugRequest);
            let result = history::read_claudeai_in(home, request, &mut |p| lines.progress(p));
            lines.result(result)
        }
        Step::Skills => {
            let request = request!(SkillsStep);
            if matches!(
                request.action,
                SkillsAction::List | SkillsAction::Refresh | SkillsAction::RefreshHeld
            ) && !request.ids.is_empty()
            {
                return lines.error(REQUEST.into());
            }
            let slug = SlugRequest {
                slug: request.slug.clone(),
            };
            let chosen = SkillsRequest {
                slug: request.slug,
                ids: request.ids,
            };
            match request.action {
                SkillsAction::List => lines.result(skills::list_skills_in(home, slug)),
                SkillsAction::Agree => lines.result(skills::agree_skills_in(home, chosen)),
                SkillsAction::Withdraw => lines.result(skills::withdraw_skills_in(home, chosen)),
                SkillsAction::Refresh => lines.result(skills::refresh_skills_in(home, slug)),
                SkillsAction::RefreshHeld => lines.result(skills::refresh_held_in(home, slug)),
            }
        }
    }
}

fn guard(lines: &mut Lines<'_>, work: impl FnOnce(&mut Lines<'_>) -> bool) -> bool {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(lines))) {
        Ok(result) => result,
        Err(_) => lines.error(STOPPED.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history;
    use serde_json::{Value, json};
    use std::{
        fs, io,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Home(PathBuf);
    impl Home {
        fn new(agree: bool) -> Self {
            let path = std::env::temp_dir().join(format!(
                "voice-reader-api-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            crate::new_assistant_in(
                &path,
                NewAssistantRequest {
                    name: "Ari".into(),
                    pronouns: "they/them".into(),
                    principal: "Robin".into(),
                    principal_pronouns: "they/them".into(),
                },
            )
            .unwrap();
            if agree {
                crate::agree_consent_in(&path, SlugRequest { slug: "ari".into() }).unwrap();
            }
            for i in 0..3 {
                let folder = path.join(format!(".claude/projects/project-{i}"));
                fs::create_dir_all(&folder).unwrap();
                let record = json!({"type":"user","timestamp":"2026-01-03T10:00:00.000Z","origin":{"kind":"human"},"message":{"content":"Start with the smallest useful change."}});
                fs::write(folder.join("session.jsonl"), format!("{record}\n")).unwrap();
            }
            Self(path)
        }
    }
    impl Drop for Home {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn lines(bytes: &[u8]) -> Vec<Value> {
        assert!(bytes.ends_with(b"\n"));
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .lines()
            .map(|line| {
                let value: Value = serde_json::from_str(line).unwrap();
                assert_eq!(value.as_object().unwrap().len(), 1);
                value
            })
            .collect()
    }
    fn error(home: Option<&Path>, step: Step, input: &mut dyn Read, expected: &str) {
        let mut output = vec![];
        assert!(!serve(home, step, input, &mut output));
        assert_eq!(lines(&output), vec![json!({"error": expected})]);
    }
    #[test]
    fn panic_becomes_one_stopped_error() {
        let mut output = vec![];
        let mut writer = Lines {
            output: &mut output,
            max_line: MAX_LINE,
            alive: true,
        };
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            guard(&mut writer, |_| panic!("private panic canary"))
        }));
        std::panic::set_hook(hook);
        assert!(outcome.is_ok(), "reader panic escaped its guard");
        assert!(!outcome.unwrap());
        assert_eq!(lines(&output), vec![json!({"error": STOPPED})]);
    }
    #[test]
    fn oversized_error_becomes_one_bounded_error() {
        let mut output = vec![];
        let limit = 100;
        let mut writer = Lines {
            output: &mut output,
            max_line: limit,
            alive: true,
        };
        assert!(!writer.error("private error canary".repeat(100)));
        assert_eq!(lines(&output), vec![json!({"error": TOO_MUCH})]);
        assert!(output.len() <= limit + 1);
    }
    #[test]
    fn progress_then_one_result_matches_the_api() {
        let home = Home::new(true);
        let request = ReadRequest {
            slug: "ari".into(),
            dry_run: false,
        };
        let mut progress = vec![];
        let expected = history::read_claude_code_in(&home.0, request.clone(), &mut |p| {
            progress.push(json!({"progress": p}))
        })
        .unwrap();
        assert!(!progress.is_empty());
        let mut output = vec![];
        assert!(serve(
            Some(&home.0),
            Step::ReadClaudeCode,
            &mut serde_json::to_vec(&request).unwrap().as_slice(),
            &mut output
        ));
        progress.push(json!({"result": expected}));
        assert_eq!(lines(&output), progress);
    }
    #[test]
    fn refresh_held_uses_the_panels_existing_state_lock() {
        let home = Home::new(false);
        let _lock = voice_ingest::private::lock_state(&home.0, "ari").unwrap();
        error(
            Some(&home.0),
            Step::Skills,
            &mut br#"{"slug":"ari","action":"refresh"}"#.as_slice(),
            history::BUSY,
        );
        let mut output = vec![];
        assert!(serve(
            Some(&home.0),
            Step::Skills,
            &mut br#"{"slug":"ari","action":"refresh-held"}"#.as_slice(),
            &mut output,
        ));
        assert_eq!(lines(&output).len(), 1);
        assert!(lines(&output)[0].get("result").is_some());
    }
    #[test]
    fn refresh_held_takes_a_free_lock_and_releases_it() {
        let home = Home::new(false);
        let mut output = vec![];
        assert!(serve(
            Some(&home.0),
            Step::Skills,
            &mut br#"{"slug":"ari","action":"refresh-held"}"#.as_slice(),
            &mut output,
        ));
        assert!(lines(&output)[0].get("result").is_some());
        drop(voice_ingest::private::lock_state(&home.0, "ari").unwrap());
    }
    #[test]
    fn refresh_held_refuses_a_lock_error_with_the_skills_sentence() {
        let home = Home::new(false);
        drop(voice_ingest::private::lock_state(&home.0, "ari").unwrap());
        let state = home.0.join(".claude/state/ari");
        fs::rename(&state, home.0.join("saved-state")).unwrap();
        std::os::unix::fs::symlink(home.0.join("saved-state"), state).unwrap();
        error(
            Some(&home.0),
            Step::Skills,
            &mut br#"{"slug":"ari","action":"refresh-held"}"#.as_slice(),
            skills::SKILLS_READ,
        );
    }
    #[test]
    fn refresh_held_refuses_ids() {
        let home = Home::new(false);
        error(
            Some(&home.0),
            Step::Skills,
            &mut br#"{"slug":"ari","action":"refresh-held","ids":["claude:notes"]}"#.as_slice(),
            REQUEST,
        );
    }
    #[test]
    fn no_permission_and_no_home_are_safe_errors() {
        let home = Home::new(false);
        error(
            Some(&home.0),
            Step::ReadClaudeCode,
            &mut br#"{"slug":"ari","dryRun":true}"#.as_slice(),
            history::PERMISSION,
        );
        error(
            None,
            Step::ClaudeCodeFolders,
            &mut br#"{"slug":"ari"}"#.as_slice(),
            history::FIND,
        );
    }
    #[test]
    fn every_bad_request_has_one_fixed_error() {
        let home = Home::new(true);
        for step in Step::ALL {
            for input in [
                b"".as_slice(),
                b"null",
                b"[]",
                b"42",
                b"\"text\"",
                b"{}",
                b"{",
                b"\xff",
                br#"{"slug":"ari"} {}"#,
                br#"{"slug":"ari","slug":"canary-outside"}"#,
                br#"{"slug":"ari","canary-outside":true}"#,
                br#"{"slug":123}"#,
            ] {
                error(Some(&home.0), step, &mut &*input, REQUEST);
            }
            error(
                Some(&home.0),
                step,
                &mut vec![b' '; MAX_REQUEST as usize + 1].as_slice(),
                REQUEST,
            );
            struct Broken;
            impl Read for Broken {
                fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                    Err(io::Error::other("canary-outside"))
                }
            }
            error(Some(&home.0), step, &mut Broken, REQUEST);
        }
        for input in [
            r#"{"slug":"ari","action":"list","ids":["canary-outside"]}"#,
            r#"{"slug":"ari","action":"refresh","ids":["canary-outside"]}"#,
            r#"{"slug":"ari","action":"list","action":"agree"}"#,
            r#"{"slug":"ari","action":"list","ids":12}"#,
            r#"{"slug":"ari","action":"unknown"}"#,
            r#"{"slug":"ari","action":"list","unknown":true}"#,
            r#"{"slug":"ari","action":"list","ids":[],"ids":[]}"#,
        ] {
            error(Some(&home.0), Step::Skills, &mut input.as_bytes(), REQUEST);
        }
    }
    #[test]
    fn oversized_result_becomes_one_error() {
        let home = Home::new(true);
        let mut output = vec![];
        assert!(!serve_with_limit(
            Some(&home.0),
            Step::ClaudeCodeFolders,
            &mut br#"{"slug":"ari"}"#.as_slice(),
            &mut output,
            100
        ));
        assert_eq!(lines(&output), vec![json!({"error": TOO_MUCH})]);
    }
    #[test]
    fn oversized_progress_is_dropped_and_the_next_line_is_flushed() {
        #[derive(Default)]
        struct Flushed {
            bytes: Vec<u8>,
            flushes: usize,
        }
        impl Write for Flushed {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushes += 1;
                Ok(())
            }
        }
        let mut output = Flushed::default();
        let limit = br#"{"result":null}"#.len();
        let mut writer = Lines {
            output: &mut output,
            max_line: limit,
            alive: true,
        };
        writer.progress(ReadProgress { done: 1, total: 2 });
        assert!(writer.result(Ok(())));
        assert_eq!(lines(&output.bytes), vec![json!({"result":null})]);
        assert_eq!(output.flushes, 1);
    }
    #[test]
    fn failed_flush_stops_writes_and_still_finishes_the_work() {
        struct Disconnect {
            bytes: Vec<u8>,
            failed: bool,
        }
        impl Write for Disconnect {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                assert!(!self.failed, "wrote after flush failed");
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                assert!(!self.failed, "flushed again after failure");
                self.failed = true;
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        let home = Home::new(true);
        let mut output = Disconnect {
            bytes: vec![],
            failed: false,
        };
        assert!(!serve(
            Some(&home.0),
            Step::ReadClaudeCode,
            &mut br#"{"slug":"ari","dryRun":false}"#.as_slice(),
            &mut output
        ));
        assert!(output.failed);
        assert_eq!(lines(&output.bytes).len(), 1);
        assert!(
            home.0
                .join(".claude/state/ari/decisions-claude-code.jsonl")
                .is_file()
        );
    }
    #[test]
    fn broken_writer_is_never_retried_but_work_finishes() {
        struct Disconnect {
            bytes: Vec<u8>,
            failed: bool,
        }
        impl Write for Disconnect {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                assert!(!self.failed, "wrote again after failure");
                if self.bytes.ends_with(b"\n") {
                    self.failed = true;
                    return Err(io::ErrorKind::BrokenPipe.into());
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                assert!(!self.failed, "flushed after failure");
                Ok(())
            }
        }
        let home = Home::new(true);
        let mut output = Disconnect {
            bytes: vec![],
            failed: false,
        };
        assert!(!serve(
            Some(&home.0),
            Step::ReadClaudeCode,
            &mut br#"{"slug":"ari","dryRun":false}"#.as_slice(),
            &mut output
        ));
        assert!(output.failed);
        assert_eq!(lines(&output.bytes).len(), 1);
        assert!(lines(&output.bytes)[0].get("progress").is_some());
        assert!(
            home.0
                .join(".claude/state/ari/decisions-claude-code.jsonl")
                .is_file()
        );
    }
}
