//! Reading local histories after the assistant's current agreement.
use crate::dto::*;
use persona_builder::{
    assistants,
    error::Error,
    member::{self, Member},
};
use persona_builder_ingest::{claudeai, corpus};
use persona_builder_ingest::{
    filters::Filters,
    mine::{self, Split},
    private,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub(crate) const FIND: &str =
    "Persona Builder couldn't find that assistant. Close it and open it again.";
const ELSEWHERE: &str = "This assistant keeps its history outside your Claude folder. Use the persona-builder command for it.";
pub(crate) const PERMISSION: &str =
    "Persona Builder needs your permission before it reads anything. Give it in the first step.";
pub(crate) fn failure(message: &str) -> Failure {
    Failure {
        message: message.into(),
    }
}
pub(crate) fn readable(home: &Path, slug: &str) -> Result<Member> {
    assistants::assistant_dir(home, slug).map_err(|_| failure(FIND))?;
    let member = assistants::list(home)
        .map_err(|_| failure(FIND))?
        .into_iter()
        .find(|a| a.member.slug == slug)
        .ok_or_else(|| failure(FIND))?
        .member;
    member::private_state_dir(&member, home).ok_or_else(|| failure(ELSEWHERE))?;
    if !crate::read_consent(home, slug).agreed {
        return Err(failure(PERMISSION));
    }
    Ok(member)
}
type Result<T> = std::result::Result<T, Failure>;
pub(crate) const BUSY: &str =
    "Persona Builder is already working on this assistant. Wait for it to finish, then try again.";
const SAVE: &str = "Persona Builder couldn't save what it read. Check there's space on this computer, then try again.";
const FOLDERS: &str =
    "Persona Builder couldn't look through your Claude Code history. Close it and open it again.";
const READ: &str =
    "Persona Builder couldn't read your Claude Code history. Close it and open it again.";
pub(crate) fn locked(home: &Path, slug: &str) -> Result<(Member, private::StateLock)> {
    locked_after(home, slug, &mut || {})
}
// The callback lets tests forget or withdraw permission between the check and the lock.
pub(crate) fn locked_after(
    home: &Path,
    slug: &str,
    between: &mut dyn FnMut(),
) -> Result<(Member, private::StateLock)> {
    let member = readable(home, slug)?;
    between();
    let lock = private::lock_existing_state(home, slug).map_err(|e| match e {
        Error::Invalid(text) if text == private::BUSY => failure(BUSY),
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            failure(PERMISSION)
        }
        _ => failure(SAVE),
    })?;
    if !crate::read_consent(home, slug).agreed {
        return Err(failure(PERMISSION));
    }
    Ok((member, lock))
}
pub(crate) fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
fn progress_callback(progress: &mut dyn FnMut(ReadProgress)) -> impl FnMut(usize, usize) + '_ {
    let mut last = None;
    move |done, total| {
        let percent = if total == 0 {
            100
        } else {
            (done as u128 * 100 / total as u128) as u32
        };
        if last != Some(percent) {
            progress(ReadProgress {
                done: count(done),
                total: count(total),
            });
            last = Some(percent);
        }
    }
}
pub fn can_read_in(home: &Path, request: SlugRequest) -> Result<()> {
    readable(home, &request.slug).map(|_| ())
}
pub fn claude_code_folders_in(home: &Path, request: SlugRequest) -> Result<ClaudeCodeFolders> {
    let member = readable(home, &request.slug)?;
    let projects = home.join(".claude/projects");
    if !projects.try_exists().map_err(|_| failure(FOLDERS))? {
        return Ok(ClaudeCodeFolders {
            found: false,
            folders: vec![],
        });
    }
    let filters = Filters::compile(&member.filters).map_err(|_| failure(FOLDERS))?;
    let folders = mine::folders(&filters, &projects, home)
        .map_err(|_| failure(FOLDERS))?
        .into_iter()
        .map(|row| ClaudeCodeFolder {
            project: row.project,
            conversations: count(row.sessions),
            read: !row.excluded,
        })
        .collect();
    Ok(ClaudeCodeFolders {
        found: true,
        folders,
    })
}
pub fn read_claude_code_in(
    home: &Path,
    request: ReadRequest,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<ReadCounts> {
    let (member, _lock) = locked(home, &request.slug)?;
    let projects = home.join(".claude/projects");
    if !projects.try_exists().map_err(|_| failure(READ))? {
        return Ok(ReadCounts {
            questions: 0,
            held_out: 0,
            messages: 0,
            private_left_out: 0,
            first: None,
            last: None,
        });
    }
    let filters = Filters::compile(&member.filters).map_err(|_| failure(READ))?;
    let mined = mine::mine_with_progress(
        &member,
        &filters,
        &projects,
        home,
        &Split::Holdout(mine::DEFAULT_HOLDOUT),
        &mut progress_callback(progress),
    )
    .map_err(|_| failure(READ))?;
    if !request.dry_run {
        private::write_private(
            home,
            &request.slug,
            "decisions-claude-code.jsonl",
            mine::to_jsonl(&mined.records).as_bytes(),
        )
        .map_err(|_| failure(SAVE))?;
    }
    let date = |r: &serde_json::Map<String, serde_json::Value>| {
        r["timestamp"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect()
    };
    Ok(ReadCounts {
        questions: count(mined.questions),
        held_out: count(mined.held),
        messages: count(mined.messages),
        private_left_out: count(mined.personal.saturating_add(mined.secret)),
        first: mined.records.first().map(date),
        last: mined.records.last().map(date),
    })
}
const EXPORT_GONE: &str =
    "Persona Builder can't find your claude.ai download any more. Choose it again.";
const EXPORT_BAD: &str = "Persona Builder couldn't read this download. Check it's the file claude.ai sent you, then choose it again.";
const NO_CHATS: &str = "Choose at least one chat first.";
const NOTHING: &str = "There's nothing to put together yet. Read your Claude Code history or your claude.ai chats first.";
const MERGE: &str =
    "Persona Builder couldn't put your history together. Close it and open it again.";
const EXPORT: &str = "claudeai-export.json";
const REVIEW: &str = "claudeai-review.tsv";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportChoice {
    path: String,
}
fn chosen_export(home: &Path, slug: &str) -> Result<Option<PathBuf>> {
    private::read_private_optional(home, slug, EXPORT)
        .map_err(|_| failure(EXPORT_BAD))?
        .map(|bytes| {
            let choice: ExportChoice =
                serde_json::from_slice(&bytes).map_err(|_| failure(EXPORT_BAD))?;
            let path = PathBuf::from(choice.path);
            if !path.is_absolute() {
                return Err(failure(EXPORT_BAD));
            }
            match fs::symlink_metadata(&path) {
                Ok(_) => Ok(path),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(failure(EXPORT_GONE)),
                Err(_) => Err(failure(EXPORT_BAD)),
            }
        })
        .transpose()
}
fn export_status(home: &Path, slug: &str) -> Result<Option<ExportStatus>> {
    let Some(path) = chosen_export(home, slug)? else {
        return Ok(None);
    };
    let export = claudeai::read_export(&path).map_err(|_| failure(EXPORT_BAD))?;
    let inspection = claudeai::inspect(&export).map_err(|_| failure(EXPORT_BAD))?;
    let chats = claudeai::conversations(&export).map_err(|_| failure(EXPORT_BAD))?;
    Ok(Some(ExportStatus {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        ready: inspection.missing.is_empty(),
        chats: count(chats),
    }))
}
fn export(home: &Path, slug: &str) -> Result<claudeai::Export> {
    let path = chosen_export(home, slug)?.ok_or_else(|| failure(EXPORT_GONE))?;
    claudeai::read_export(&path).map_err(|_| failure(EXPORT_BAD))
}
fn review(home: &Path, slug: &str, missing: &str, error: &str) -> Result<Vec<claudeai::ReviewRow>> {
    let bytes = private::read_private_optional(home, slug, REVIEW)
        .map_err(|_| failure(error))?
        .ok_or_else(|| failure(missing))?;
    claudeai::read_review(REVIEW, &bytes).map_err(|_| failure(error))
}
fn chat_list(rows: Vec<claudeai::ReviewRow>) -> ChatList {
    let mut list = ChatList {
        chats: vec![],
        mine: 0,
        not_mine: 0,
        unmarked: 0,
    };
    for row in rows {
        let mark = if claudeai::is_mine(&row.mine) {
            list.mine = list.mine.saturating_add(1);
            ChatMark::Mine
        } else if row
            .mine
            .trim_matches(persona_builder_ingest::python::py_space)
            .is_empty()
        {
            list.unmarked = list.unmarked.saturating_add(1);
            ChatMark::Unmarked
        } else {
            list.not_mine = list.not_mine.saturating_add(1);
            ChatMark::NotMine
        };
        list.chats.push(Chat {
            uuid: row.uuid,
            title: row.title,
            created: row.created,
            messages: count(row.messages.parse().unwrap_or_default()),
            mark,
        });
    }
    list
}
pub fn use_claudeai_export_in(home: &Path, slug: &str, path: &Path) -> Result<ExportStatus> {
    let (_member, _lock) = locked(home, slug)?;
    let path = path
        .to_str()
        .filter(|_| path.is_absolute())
        .ok_or_else(|| failure(EXPORT_BAD))?;
    let bytes =
        serde_json::to_vec(&ExportChoice { path: path.into() }).map_err(|_| failure(SAVE))?;
    private::write_private(home, slug, EXPORT, &bytes).map_err(|_| failure(SAVE))?;
    export_status(home, slug)?.ok_or_else(|| failure(EXPORT_GONE))
}
pub fn claudeai_export_status_in(
    home: &Path,
    request: SlugRequest,
) -> Result<Option<ExportStatus>> {
    readable(home, &request.slug)?;
    export_status(home, &request.slug)
}
pub fn list_claudeai_chats_in(home: &Path, request: SlugRequest) -> Result<ChatList> {
    let (member, _lock) = locked(home, &request.slug)?;
    let export = export(home, &request.slug)?;
    let existing = private::read_private_optional(home, &request.slug, REVIEW)
        .map_err(|_| failure(EXPORT_BAD))?
        .map(|bytes| claudeai::read_review(REVIEW, &bytes).map_err(|_| failure(EXPORT_BAD)))
        .transpose()?
        .unwrap_or_default();
    let filters = Filters::compile(&member.filters).map_err(|_| failure(EXPORT_BAD))?;
    let titles =
        claudeai::titles(&filters, &export, &existing, None).map_err(|_| failure(EXPORT_BAD))?;
    private::write_private(
        home,
        &request.slug,
        REVIEW,
        claudeai::review_text(&titles.rows).as_bytes(),
    )
    .map_err(|_| failure(SAVE))?;
    Ok(chat_list(titles.rows))
}
pub fn mark_claudeai_chats_in(home: &Path, request: MarkChatsRequest) -> Result<ChatList> {
    readable(home, &request.slug)?;
    if request.uuids.is_empty() && !request.unmarked {
        return Err(failure(NO_CHATS));
    }
    let (_member, _lock) = locked(home, &request.slug)?;
    let mut rows = review(home, &request.slug, EXPORT_GONE, SAVE)?;
    claudeai::mark(
        &mut rows,
        &claudeai::Selector {
            uuids: request.uuids,
            unmarked: request.unmarked,
            ..Default::default()
        },
        if request.mine { "y" } else { "n" },
    );
    private::write_private(
        home,
        &request.slug,
        REVIEW,
        claudeai::review_text(&rows).as_bytes(),
    )
    .map_err(|_| failure(SAVE))?;
    Ok(chat_list(rows))
}
pub fn read_claudeai_in(
    home: &Path,
    request: SlugRequest,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<ClaudeaiCounts> {
    let (member, _lock) = locked(home, &request.slug)?;
    let export = export(home, &request.slug)?;
    let rows = review(home, &request.slug, NO_CHATS, EXPORT_BAD)?;
    let filters = Filters::compile(&member.filters).map_err(|_| failure(EXPORT_BAD))?;
    let mined = claudeai::mine_with_progress(
        &filters,
        &export,
        &rows,
        &Split::Holdout(mine::DEFAULT_HOLDOUT),
        &mut progress_callback(progress),
    )
    .map_err(|_| failure(EXPORT_BAD))?;
    private::write_private(
        home,
        &request.slug,
        "decisions-claudeai.jsonl",
        mine::to_jsonl(&mined.records).as_bytes(),
    )
    .map_err(|_| failure(SAVE))?;
    Ok(ClaudeaiCounts {
        chats: count(mined.read),
        messages: count(mined.records.len()),
        private_left_out: count(mined.personal.saturating_add(mined.secret)),
    })
}
pub fn merge_histories_in(home: &Path, request: SlugRequest) -> Result<MergeCounts> {
    let (member, _lock) = locked(home, &request.slug)?;
    let names = private::list_private(home, &request.slug).map_err(|_| failure(MERGE))?;
    let discovered = corpus::discover(&names, "decisions.jsonl");
    if discovered.is_empty() {
        return Err(failure(NOTHING));
    }
    let bytes = discovered
        .iter()
        .map(|(name, _)| {
            private::read_private(home, &request.slug, name).map_err(|_| failure(MERGE))
        })
        .collect::<Result<Vec<_>>>()?;
    let inputs: Vec<_> = discovered
        .iter()
        .zip(&bytes)
        .map(|((name, mode), bytes)| corpus::Input {
            name,
            mode: *mode,
            bytes,
        })
        .collect();
    let filters = Filters::compile(&member.filters).map_err(|_| failure(MERGE))?;
    let merged = corpus::merge(&filters, &inputs, &Split::Holdout(mine::DEFAULT_HOLDOUT))
        .map_err(|_| failure(MERGE))?;
    private::write_private(
        home,
        &request.slug,
        "decisions.jsonl",
        mine::to_jsonl(&merged.records).as_bytes(),
    )
    .map_err(|_| failure(SAVE))?;
    Ok(MergeCounts {
        questions: count(merged.questions),
        held_out: count(merged.held),
        messages: count(merged.messages),
        duplicates: count(merged.duplicates),
        private_left_out: count(merged.personal.saturating_add(merged.secret)),
    })
}
fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| failure(FIND))
}
pub fn can_read(request: SlugRequest) -> Result<()> {
    can_read_in(&home()?, request)
}
pub fn claude_code_folders(request: SlugRequest) -> Result<ClaudeCodeFolders> {
    claude_code_folders_in(&home()?, request)
}
pub fn read_claude_code(
    request: ReadRequest,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<ReadCounts> {
    read_claude_code_in(&home()?, request, progress)
}
pub fn use_claudeai_export(slug: &str, path: &Path) -> Result<ExportStatus> {
    use_claudeai_export_in(&home()?, slug, path)
}
pub fn claudeai_export_status(request: SlugRequest) -> Result<Option<ExportStatus>> {
    claudeai_export_status_in(&home()?, request)
}
pub fn list_claudeai_chats(request: SlugRequest) -> Result<ChatList> {
    list_claudeai_chats_in(&home()?, request)
}
pub fn mark_claudeai_chats(request: MarkChatsRequest) -> Result<ChatList> {
    mark_claudeai_chats_in(&home()?, request)
}
pub fn read_claudeai(
    request: SlugRequest,
    progress: &mut dyn FnMut(ReadProgress),
) -> Result<ClaudeaiCounts> {
    read_claudeai_in(&home()?, request, progress)
}
pub fn merge_histories(request: SlugRequest) -> Result<MergeCounts> {
    merge_histories_in(&home()?, request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Home(PathBuf);

    impl Home {
        fn new() -> Self {
            let home = Self(std::env::temp_dir().join(format!(
                "pb-history-lock-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )));
            fs::create_dir(&home.0).unwrap();
            crate::new_assistant_in(
                &home.0,
                NewAssistantRequest {
                    name: "Ari".into(),
                    pronouns: "they/them".into(),
                    principal: "Robin".into(),
                    principal_pronouns: "they/them".into(),
                },
            )
            .unwrap();
            crate::agree_consent_in(&home.0, SlugRequest { slug: "ari".into() }).unwrap();
            home
        }

        fn state(&self) -> PathBuf {
            self.0.join(".claude/state/ari")
        }

        fn files(&self) -> Vec<std::ffi::OsString> {
            let mut names: Vec<_> = fs::read_dir(self.state())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn a_forget_between_the_check_and_the_lock_is_refused_and_recreates_nothing() {
        let home = Home::new();
        let result = locked_after(&home.0, "ari", &mut || {
            let preview =
                crate::forget_preview_in(&home.0, SlugRequest { slug: "ari".into() }).unwrap();
            crate::forget_in(
                &home.0,
                ForgetRequest {
                    slug: "ari".into(),
                    files: preview.files,
                },
            )
            .unwrap();
        });
        assert_eq!(result.unwrap_err().message, PERMISSION);
        assert_eq!(
            fs::symlink_metadata(home.state()).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }

    #[test]
    fn permission_withdrawn_between_the_check_and_the_lock_is_refused() {
        let home = Home::new();
        let mut expected = home.files();
        expected.retain(|name| name != "consent.json");
        let result = locked_after(&home.0, "ari", &mut || {
            fs::remove_file(home.state().join("consent.json")).unwrap();
        });
        assert_eq!(result.unwrap_err().message, PERMISSION);
        assert_eq!(home.files(), expected);
    }

    #[test]
    fn current_permission_returns_the_member_and_holds_the_state_lock() {
        let home = Home::new();
        let (member, _lock) = locked(&home.0, "ari").unwrap();
        assert_eq!(member.slug, "ari");
        assert!(matches!(
            private::lock_state(&home.0, "ari"),
            Err(Error::Invalid(text)) if text == private::BUSY
        ));
    }
}
